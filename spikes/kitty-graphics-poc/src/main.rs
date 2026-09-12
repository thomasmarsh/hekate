//! Kitty graphics go/no-go spike (Braintree node TAS-010).
//!
//! Commands:
//!   probe   report whether the current terminal answers the protocol probe
//!   run     transmit, place, animate, and delete synthetic frames; measure cost
//!   info    print terminal size and capability hints
//!
//! The spike is disposable and detached from the Tangle workspace.

mod kitty;
mod scene;
mod term;

use std::io;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use term::Terminal;

fn main() {
    let options = match parse_args(std::env::args().skip(1).collect()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("{}", USAGE);
            std::process::exit(2);
        }
    };
    let code = match options.command.as_str() {
        "probe" => command_probe(&options),
        "run" => command_run(&options),
        "info" => command_info(&options),
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("{}", USAGE);
            2
        }
    };
    std::process::exit(code);
}

const USAGE: &str = "\
usage: kitty-graphics-poc COMMAND [options]

commands:
  probe                 probe terminal support for the Kitty graphics protocol
  run                   render and transmit frames, then measure
  info                  print terminal geometry and capability hints

options:
  --scene walking|dense   scene to render (default walking)
  --transport raw|zlib|file|shm   payload medium (default zlib)
  --frames N              frames to transmit (default 300)
  --fps N                 cap frames per second, 0 for unlimited (default 0)
  --mux auto|none|tmux|screen     passthrough wrapper (default auto)
  --fence                 round-trip a query after each frame to bound parsing
  --force                 transmit even when the probe says unsupported
  --no-alt                do not use the alternate screen
  --timeout-ms N          probe/fence reply timeout in ms (default 500)
  --report PATH           write the JSON report to PATH
";

#[derive(Debug, Clone)]
struct Options {
    command: String,
    scene: String,
    transport: String,
    frames: u32,
    fps: f64,
    mux: String,
    fence: bool,
    force: bool,
    no_alt: bool,
    timeout_ms: u64,
    report: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            command: String::new(),
            scene: "walking".to_string(),
            transport: "zlib".to_string(),
            frames: 300,
            fps: 0.0,
            mux: "auto".to_string(),
            fence: false,
            force: false,
            no_alt: false,
            timeout_ms: 500,
            report: None,
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    if args.is_empty() {
        return Err("missing command".to_string());
    }
    options.command = args[0].clone();
    index += 1;
    while index < args.len() {
        let flag = args[index].clone();
        let mut value = || -> Result<String, String> {
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        match flag.as_str() {
            "--scene" => options.scene = value()?,
            "--transport" => options.transport = value()?,
            "--frames" => options.frames = value()?.parse().map_err(|_| "bad --frames")?,
            "--fps" => options.fps = value()?.parse().map_err(|_| "bad --fps")?,
            "--mux" => options.mux = value()?,
            "--timeout-ms" => {
                options.timeout_ms = value()?.parse().map_err(|_| "bad --timeout-ms")?
            }
            "--report" => options.report = Some(PathBuf::from(value()?)),
            "--fence" => options.fence = true,
            "--force" => options.force = true,
            "--no-alt" => options.no_alt = true,
            other => return Err(format!("unknown option: {other}")),
        }
        index += 1;
    }
    Ok(options)
}

// ---------------------------------------------------------------------------
// probe
// ---------------------------------------------------------------------------

/// What the support probe observed.
struct ProbeOutcome {
    supported: bool,
    verdict: &'static str,
    graphics_reply: Option<String>,
    da1_reply: Option<String>,
    raw_bytes: usize,
    elapsed_ms: f64,
}

fn classify(buf: &[u8], elapsed_ms: f64) -> ProbeOutcome {
    let parsed = kitty::parse_replies(buf);
    let graphics_reply = parsed.graphics.last().cloned();
    let da1_reply = parsed.device_attributes.last().cloned();
    let supported = parsed.graphics_ok();
    let verdict = if supported {
        "supported"
    } else if graphics_reply.is_some() {
        "graphics-error"
    } else if parsed.saw_device_attributes() {
        "unsupported"
    } else {
        "no-reply"
    };
    ProbeOutcome {
        supported,
        verdict,
        graphics_reply,
        da1_reply,
        raw_bytes: buf.len(),
        elapsed_ms,
    }
}

fn send_probe(term: &mut Terminal) -> io::Result<()> {
    term.write(&kitty::query())?;
    term.write(kitty::da1())?;
    term.flush()
}

/// Read replies until device attributes arrive (plus a short grace for the
/// graphics reply that must precede it), or until the timeout.
fn collect(rx: &Receiver<Vec<u8>>, timeout: Duration) -> Vec<u8> {
    let mut buf = Vec::new();
    let deadline = Instant::now() + timeout;
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(chunk) => {
                buf.extend_from_slice(&chunk);
                if kitty::saw_da1(&buf) {
                    if let Ok(extra) = rx.recv_timeout(Duration::from_millis(40)) {
                        buf.extend_from_slice(&extra);
                    }
                    break;
                }
            }
            Err(_) => break,
        }
    }
    buf
}

fn command_probe(options: &Options) -> i32 {
    let mut term = match Terminal::open() {
        Ok(term) => term,
        Err(error) => {
            eprintln!("cannot open terminal: {error}");
            return 4;
        }
    };
    if !term.is_tty {
        println!("probe: no controlling terminal (is_tty=false) — nothing to probe");
        return 4;
    }
    let timeout = Duration::from_millis(options.timeout_ms);
    let outcome = match probe_once(&mut term, timeout) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("probe failed: {error}");
            return 5;
        }
    };
    let hints: Vec<(String, String)> = term::environment_hints()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    println!("supported: {}", outcome.supported);
    println!("verdict: {}", outcome.verdict);
    println!(
        "graphics_reply: {}",
        outcome.graphics_reply.as_deref().unwrap_or("(none)")
    );
    println!(
        "da1_reply: {}",
        outcome.da1_reply.as_deref().unwrap_or("(none)")
    );
    println!("probe_ms: {:.2}", outcome.elapsed_ms);
    for (key, value) in &hints {
        println!("hint[{key}]: {value}");
    }
    if let Some(path) = &options.report {
        let report = json!({
            "spike": "kitty-graphics-poc",
            "command": "probe",
            "is_tty": term.is_tty,
            "supported": outcome.supported,
            "verdict": outcome.verdict,
            "graphics_reply": outcome.graphics_reply,
            "da1_reply": outcome.da1_reply,
            "probe_ms": outcome.elapsed_ms,
            "reply_bytes": outcome.raw_bytes,
            "mux_detected": term::detect_mux(),
            "hints": hints.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        });
        write_report(path, &report);
    }
    if outcome.supported { 0 } else { 3 }
}

fn probe_once(term: &mut Terminal, timeout: Duration) -> io::Result<ProbeOutcome> {
    term.enter_raw()?;
    let rx = term.reader()?;
    let start = Instant::now();
    send_probe(term)?;
    let buf = collect(&rx, timeout);
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    term.leave_raw();
    Ok(classify(&buf, elapsed_ms))
}

fn command_info(options: &Options) -> i32 {
    let term = match Terminal::open() {
        Ok(term) => term,
        Err(error) => {
            eprintln!("cannot open terminal: {error}");
            return 4;
        }
    };
    let size = term.size();
    println!("is_tty: {}", term.is_tty);
    println!("cells: {}x{}", size.cols, size.rows);
    println!("pixels: {}x{}", size.xpixel, size.ypixel);
    println!("mux_detected: {}", term::detect_mux());
    for (key, value) in term::environment_hints() {
        println!("hint[{key}]: {value}");
    }
    if let Some(path) = &options.report {
        let report = json!({
            "spike": "kitty-graphics-poc",
            "command": "info",
            "is_tty": term.is_tty,
            "cells": {"cols": size.cols, "rows": size.rows},
            "pixels": {"width": size.xpixel, "height": size.ypixel},
            "mux_detected": term::detect_mux(),
            "hints": term::environment_hints().into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        });
        write_report(path, &report);
    }
    0
}
// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Per-frame byte accounting for one transport.
#[derive(Default, Clone, Copy)]
struct ByteAccount {
    raw: u64,
    data: u64,
    base64: u64,
    escaped: u64,
    pty: u64,
    sequences: u64,
    file_write_ms: f64,
}

fn command_run(options: &Options) -> i32 {
    let mut term = match Terminal::open() {
        Ok(term) => term,
        Err(error) => {
            eprintln!("cannot open terminal: {error}");
            return 4;
        }
    };
    if options.frames == 0 {
        eprintln!("--frames must be greater than zero");
        return 2;
    }
    let timeout = Duration::from_millis(options.timeout_ms);
    let mut hints: Vec<(String, String)> = term::environment_hints()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    hints.sort();

    if let Err(error) = term.enter_raw() {
        eprintln!("cannot enter raw mode: {error}");
        return 5;
    }
    let rx = match term.reader() {
        Ok(rx) => rx,
        Err(error) => {
            eprintln!("cannot read terminal: {error}");
            return 5;
        }
    };

    // Probe before transmitting anything.
    let probe = if term.is_tty {
        let start = Instant::now();
        if let Err(error) = send_probe(&mut term) {
            eprintln!("probe write failed: {error}");
            return 5;
        }
        let buf = collect(&rx, timeout);
        classify(&buf, start.elapsed().as_secs_f64() * 1000.0)
    } else {
        ProbeOutcome {
            supported: false,
            verdict: "no-tty",
            graphics_reply: None,
            da1_reply: None,
            raw_bytes: 0,
            elapsed_ms: 0.0,
        }
    };

    if !probe.supported && !options.force {
        println!(
            "terminal does not answer the graphics probe ({}); refusing to transmit.",
            probe.verdict
        );
        println!("re-run with --force to measure the encode/write path anyway.");
        term.leave_raw();
        return 3;
    }

    let mux = if options.mux == "auto" {
        term::detect_mux().to_string()
    } else {
        options.mux.clone()
    };
    if mux == "tmux" {
        // A tmux pane can still receive replies from the outer terminal only
        // when tmux forwards them; report the probe result as-is.
    }

    let mut spec = scene::spec(&options.scene);
    let use_alt = term.is_tty && !options.no_alt;
    if use_alt {
        let _ = term.write(b"\x1b[?1049h\x1b[?25l");
    }
    let cleanup = term::Cleanup::new(term.fd(), use_alt);

    let mut rasters = Vec::new();
    let mut encodes = Vec::new();
    let mut writes = Vec::new();
    let mut totals = Vec::new();
    let mut fences = Vec::new();
    let mut accounts = Vec::new();
    let mut resize_events = 0u32;
    let mut fence_enabled = probe.supported && options.fence;
    let mut fenced_frames = 0u32;
    let mut medium_paths: Vec<String> = Vec::new();
    let mut last_size = term.size();
    let mut notes: Vec<String> = Vec::new();

    for frame in 0..options.frames {
        let size = term.size();
        if size != last_size {
            resize_events += 1;
            last_size = size;
            if let Some((width, height)) = resize_target(&spec, &size) {
                spec.width = width;
                spec.height = height;
            }
            // The image id is reused, so the next transmit replaces the old
            // placement at the new size without needing an explicit delete.
        }

        let frame_start = Instant::now();
        let rgba = scene::render(&spec, frame);
        let raster_done = Instant::now();

        let (sequences, account) = match encode_transport(options, &spec, &rgba, &mut medium_paths)
        {
            Ok(result) => result,
            Err(error) => {
                eprintln!("encode failed: {error}");
                drop(cleanup);
                term.leave_raw();
                return 5;
            }
        };
        let encode_done = Instant::now();

        let wrapped: Vec<Vec<u8>> = match mux.as_str() {
            "tmux" => sequences.iter().map(|seq| kitty::tmux_wrap(seq)).collect(),
            "screen" => sequences
                .iter()
                .map(|seq| kitty::screen_wrap(seq))
                .collect(),
            _ => sequences,
        };
        let mut outbound = Vec::with_capacity(wrapped.iter().map(Vec::len).sum::<usize>() + 8);
        outbound.extend_from_slice(b"\x1b[1;1H");
        for seq in &wrapped {
            outbound.extend_from_slice(seq);
        }
        if let Err(error) = term.write(&outbound) {
            eprintln!("terminal write failed: {error}");
            break;
        }
        let _ = term.flush();
        let write_done = Instant::now();

        if fence_enabled {
            let fence_start = Instant::now();
            if term.write(&kitty::query()).is_err() {
                fence_enabled = false;
            } else {
                let _ = term.flush();
                let buf = collect(&rx, timeout);
                let fence_ms = fence_start.elapsed().as_secs_f64() * 1000.0;
                if kitty::parse_replies(&buf).graphics_ok() {
                    fenced_frames += 1;
                    fences.push(fence_ms);
                } else {
                    fence_enabled = false;
                    notes.push(format!(
                        "fence disabled at frame {frame}: no graphics reply within {} ms",
                        options.timeout_ms
                    ));
                }
            }
        }

        let total_done = Instant::now();
        rasters.push(ms(raster_done - frame_start));
        encodes.push(ms(encode_done - raster_done));
        writes.push(ms(write_done - encode_done));
        totals.push(ms(total_done - frame_start));
        let account = ByteAccount {
            pty: outbound.len() as u64,
            ..account
        };
        accounts.push(account);

        if options.fps > 0.0 {
            let target = Duration::from_secs_f64(1.0 / options.fps);
            let elapsed = total_done - frame_start;
            if elapsed < target {
                std::thread::sleep(target - elapsed);
            }
        }
    }

    // Cleanup: delete images, leave the alternate screen, restore termios.
    drop(cleanup);
    term.leave_raw();
    for path in &medium_paths {
        let _ = std::fs::remove_file(path);
    }
    for name in &medium_paths {
        if name.starts_with("/tangle-kitty-poc") {
            unlink_shm(name);
        }
    }

    let size = term.size();
    let frames = accounts.len();
    let total_ms: f64 = totals.iter().sum();
    let byte_totals: ByteAccount = accounts.iter().fold(ByteAccount::default(), |mut acc, a| {
        acc.raw += a.raw;
        acc.data += a.data;
        acc.base64 += a.base64;
        acc.escaped += a.escaped;
        acc.pty += a.pty;
        acc.sequences += a.sequences;
        acc.file_write_ms += a.file_write_ms;
        acc
    });
    let raw_total = byte_totals.raw.max(1);
    let report = json!({
        "spike": "kitty-graphics-poc",
        "command": "run",
        "scene": spec.name,
        "transport": options.transport,
        "mux": mux,
        "frames": frames,
        "is_tty": term.is_tty,
        "terminal": {
            "cells": {"cols": size.cols, "rows": size.rows},
            "pixels": {"width": size.xpixel, "height": size.ypixel},
            "cell_width_px": size.xpixel.checked_div(size.cols).unwrap_or(0),
            "cell_height_px": size.ypixel.checked_div(size.rows).unwrap_or(0),
            "hints": hints.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        },
        "probe": {
            "supported": probe.supported,
            "verdict": probe.verdict,
            "graphics_reply": probe.graphics_reply,
            "da1_reply": probe.da1_reply,
            "probe_ms": probe.elapsed_ms,
        },
        "geometry": {
            "width": spec.width,
            "height": spec.height,
            "rgba_bytes": u64::from(spec.width) * u64::from(spec.height) * 4,
        },
        "bytes": {
            "raw_total": byte_totals.raw,
            "data_total": byte_totals.data,
            "base64_total": byte_totals.base64,
            "escaped_total": byte_totals.escaped,
            "pty_total": byte_totals.pty,
            "png_equivalent_note": "raw RGBA before any transport encoding",
            "data_per_frame": byte_totals.data as f64 / frames.max(1) as f64,
            "pty_per_frame": byte_totals.pty as f64 / frames.max(1) as f64,
            "sequences_per_frame": byte_totals.sequences as f64 / frames.max(1) as f64,
            "compression_ratio": byte_totals.data as f64 / raw_total as f64,
            "medium_write_ms_total": byte_totals.file_write_ms,
        },
        "frame_stats_ms": {
            "raster": stats(&mut rasters.clone()),
            "encode": stats(&mut encodes.clone()),
            "write": stats(&mut writes.clone()),
            "total": stats(&mut totals.clone()),
        },
        "rates": {
            "write_only_fps": if total_ms > 0.0 { frames as f64 * 1000.0 / total_ms } else { 0.0 },
            "fenced_fps": if fenced_frames > 0 {
                fenced_frames as f64 * 1000.0 / fences.iter().sum::<f64>()
            } else {
                0.0
            },
            "fenced_frames": fenced_frames,
            "total_ms": total_ms,
        },
        "fence_stats_ms": stats(&mut fences),
        "resize_events": resize_events,
        "notes": notes,
    });

    if let Some(path) = &options.report {
        write_report(path, &report);
    }
    print_summary(&report);
    0
}

/// Reasonable resize target from the terminal's pixel size, if it reports one.
fn resize_target(spec: &scene::SceneSpec, size: &term::Size) -> Option<(u32, u32)> {
    if size.xpixel == 0 || size.ypixel == 0 {
        return None;
    }
    let width = (size.xpixel as u32).min(1920);
    let height = (size.ypixel as u32).min(1200);
    if width == spec.width && height == spec.height {
        None
    } else {
        Some((width, height))
    }
}

/// Build the escape sequences for one frame and account for their bytes.
fn encode_transport(
    options: &Options,
    spec: &scene::SceneSpec,
    rgba: &[u8],
    media: &mut Vec<String>,
) -> io::Result<(Vec<Vec<u8>>, ByteAccount)> {
    let account = ByteAccount {
        raw: rgba.len() as u64,
        ..ByteAccount::default()
    };
    let width = spec.width;
    let height = spec.height;
    match options.transport.as_str() {
        "raw" => {
            let control = format!("a=T,f=32,s={width},v={height},i=1");
            let frame = kitty::encode(&control, rgba);
            let sequences = frame.sequences.len() as u64;
            Ok((
                frame.sequences,
                ByteAccount {
                    data: frame.data_bytes as u64,
                    base64: frame.base64_bytes as u64,
                    escaped: frame.escaped_bytes as u64,
                    sequences,
                    ..account
                },
            ))
        }
        "zlib" => {
            let compressed = kitty::zlib(rgba);
            let control = format!("a=T,f=32,s={width},v={height},i=1,o=z");
            let frame = kitty::encode(&control, &compressed);
            let sequences = frame.sequences.len() as u64;
            Ok((
                frame.sequences,
                ByteAccount {
                    data: frame.data_bytes as u64,
                    base64: frame.base64_bytes as u64,
                    escaped: frame.escaped_bytes as u64,
                    sequences,
                    ..account
                },
            ))
        }
        "file" => {
            let path = std::env::temp_dir().join(format!(
                "tangle-kitty-poc-{}-{}.rgba",
                std::process::id(),
                media.len()
            ));
            let start = Instant::now();
            std::fs::write(&path, rgba)?;
            let write_ms = ms(start.elapsed());
            let name = path.to_string_lossy().into_owned();
            media.push(name.clone());
            let control = format!("a=T,f=32,s={width},v={height},i=1,t=f");
            let seq = kitty::apc(&control, &kitty::medium_payload(&name));
            let escaped = seq.len() as u64;
            Ok((
                vec![seq],
                ByteAccount {
                    data: rgba.len() as u64,
                    escaped,
                    sequences: 1,
                    file_write_ms: write_ms,
                    ..account
                },
            ))
        }
        "shm" => {
            let name = format!("/tangle-kitty-poc-{}-{}", std::process::id(), media.len());
            let start = Instant::now();
            write_shm(&name, rgba)?;
            let write_ms = ms(start.elapsed());
            media.push(name.clone());
            let control = format!("a=T,f=32,s={width},v={height},i=1,t=s");
            let seq = kitty::apc(&control, &kitty::medium_payload(&name));
            let escaped = seq.len() as u64;
            Ok((
                vec![seq],
                ByteAccount {
                    data: rgba.len() as u64,
                    escaped,
                    sequences: 1,
                    file_write_ms: write_ms,
                    ..account
                },
            ))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown transport: {other}"),
        )),
    }
}

fn print_summary(report: &Value) {
    let frames = report["frames"].as_u64().unwrap_or(0);
    let scene = report["scene"].as_str().unwrap_or("?");
    let transport = report["transport"].as_str().unwrap_or("?");
    let mux = report["mux"].as_str().unwrap_or("?");
    let probe = &report["probe"];
    println!(
        "kitty spike: scene={scene} transport={transport} mux={mux} frames={frames} \
         probe={} ({})",
        probe["supported"],
        probe["verdict"].as_str().unwrap_or("?")
    );
    println!(
        "  bytes/frame: data={:.0} pty={:.0} sequences={:.1} compression={:.3}",
        report["bytes"]["data_per_frame"].as_f64().unwrap_or(0.0),
        report["bytes"]["pty_per_frame"].as_f64().unwrap_or(0.0),
        report["bytes"]["sequences_per_frame"]
            .as_f64()
            .unwrap_or(0.0),
        report["bytes"]["compression_ratio"].as_f64().unwrap_or(0.0),
    );
    for key in ["raster", "encode", "write", "total"] {
        let stat = &report["frame_stats_ms"][key];
        println!(
            "  {key:<7} ms mean={:.3} p50={:.3} p95={:.3}",
            stat["mean"].as_f64().unwrap_or(0.0),
            stat["p50"].as_f64().unwrap_or(0.0),
            stat["p95"].as_f64().unwrap_or(0.0),
        );
    }
    println!(
        "  write_only_fps={:.1} fenced_fps={:.1} ({} fenced frames)",
        report["rates"]["write_only_fps"].as_f64().unwrap_or(0.0),
        report["rates"]["fenced_fps"].as_f64().unwrap_or(0.0),
        report["rates"]["fenced_frames"].as_u64().unwrap_or(0),
    );
    println!(
        "  resize_events={}",
        report["resize_events"].as_u64().unwrap_or(0)
    );
    if let Some(notes) = report["notes"].as_array() {
        for note in notes {
            println!("  note: {}", note.as_str().unwrap_or(""));
        }
    }
}

fn write_report(path: &PathBuf, report: &Value) {
    match serde_json::to_string_pretty(report) {
        Ok(text) => {
            if let Err(error) = std::fs::write(path, text) {
                eprintln!("cannot write report {}: {error}", path.display());
            }
        }
        Err(error) => eprintln!("cannot serialize report: {error}"),
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn stats(values: &mut [f64]) -> Value {
    if values.is_empty() {
        return json!({"count": 0, "mean": 0.0, "p50": 0.0, "p95": 0.0, "min": 0.0, "max": 0.0});
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let count = values.len();
    let sum: f64 = values.iter().sum();
    let pick = |fraction: f64| values[((count as f64 - 1.0) * fraction).round() as usize];
    json!({
        "count": count,
        "mean": sum / count as f64,
        "p50": pick(0.50),
        "p95": pick(0.95),
        "min": values[0],
        "max": values[count - 1],
    })
}

// ---------------------------------------------------------------------------
// shared memory (POSIX), used by the shm transport
// ---------------------------------------------------------------------------
#[cfg(unix)]
fn write_shm(name: &str, data: &[u8]) -> io::Result<()> {
    let c_name = std::ffi::CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "shm name has NUL"))?;
    let fd = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let length = data.len();
    if unsafe { libc::ftruncate(fd, length as libc::off_t) } != 0 {
        let error = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(error);
    }
    let pointer = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            length,
            libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    if pointer == libc::MAP_FAILED {
        let error = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(error);
    }
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), pointer.cast::<u8>(), length);
        libc::munmap(pointer, length);
        libc::close(fd);
    }
    Ok(())
}

#[cfg(unix)]
fn unlink_shm(name: &str) {
    if let Ok(c_name) = std::ffi::CString::new(name) {
        unsafe { libc::shm_unlink(c_name.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_supported_when_graphics_ok_precedes_device_attributes() {
        let outcome = classify(b"\x1b_Gi=31;OK\x1b\\\x1b[?62;4c", 3.0);
        assert!(outcome.supported);
        assert_eq!(outcome.verdict, "supported");
        assert_eq!(outcome.graphics_reply.as_deref(), Some("i=31;OK"));
        assert_eq!(outcome.da1_reply.as_deref(), Some("?62;4"));
    }

    #[test]
    fn classify_unsupported_when_only_device_attributes_answer() {
        let outcome = classify(b"\x1b[?1;2c", 3.0);
        assert!(!outcome.supported);
        assert_eq!(outcome.verdict, "unsupported");
    }

    #[test]
    fn classify_graphics_error_is_not_support() {
        let outcome = classify(b"\x1b_Gi=31;ENOTSUP:unsupported\x1b\\\x1b[?62c", 3.0);
        assert!(!outcome.supported);
        assert_eq!(outcome.verdict, "graphics-error");
    }

    #[test]
    fn classify_no_reply_is_not_support() {
        let outcome = classify(b"", 500.0);
        assert!(!outcome.supported);
        assert_eq!(outcome.verdict, "no-reply");
    }
}
