//! Headless command-line interface for Tangle.
//!
//! `run` loads a JSON5 scenario, advances the kernel for a fixed number of
//! ticks, and writes the canonical trace. The trace hash printed to stderr is
//! the value a golden test or manifest compares.
//!
//! `baseline` captures the deterministic Phase 1 baseline manifest at every
//! fidelity preset and optionally a non-normative wall-clock report.

use std::fmt::Display;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use tangle_cli::{CaptureRequest, canonical_trace, capture, load_scenario, load_scenario_hashed};
use tangle_sim::RunConfig;

/// Fixed steps run by `run` when `--ticks` is omitted.
///
/// The walking-skeleton scenario clears its guide path (120 m at 12 m/s) in
/// about 200 ticks, so 250 shows the whole run.
const DEFAULT_TICKS: u64 = 250;

/// Simulated seconds the `baseline` command covers at every fidelity preset.
///
/// 12.5 s is the Standard run of 250 ticks at the 50 ms step, so the Standard
/// baseline hash matches the checked-in golden trace.
const DEFAULT_BASELINE_DURATION_S: f64 = 12.5;

#[derive(Parser)]
#[command(
    name = "tangle-cli",
    version,
    about = "Headless command-line interface for Tangle"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a scenario and emit its canonical trace and hash.
    Run(RunArgs),
    /// Capture the deterministic Phase 1 baseline and a performance report.
    Baseline(BaselineArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Path to the JSON5 scenario.
    scenario: PathBuf,

    /// Root seed recorded for the run.
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Number of fixed steps to advance.
    #[arg(long, default_value_t = DEFAULT_TICKS)]
    ticks: u64,

    /// Destination for the canonical trace; `-` writes it to stdout.
    #[arg(long, short, default_value = "-")]
    output: PathBuf,

    /// Also write the lowercase hexadecimal SHA-256 of the trace to this path.
    #[arg(long)]
    hash_file: Option<PathBuf>,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run(args) => run(args),
        Command::Baseline(args) => baseline(args),
    }
}

fn run(args: RunArgs) -> ExitCode {
    let scenario = match load_scenario(&args.scenario) {
        Ok(scenario) => scenario,
        Err(error) => return fail(error),
    };

    let trace = match canonical_trace(scenario, RunConfig::new(args.seed), args.ticks) {
        Ok(trace) => trace,
        Err(error) => return fail(error),
    };

    if args.output.as_os_str() == "-" {
        if let Err(error) = io::stdout().write_all(trace.bytes()) {
            return fail(format_args!("cannot write trace to stdout: {error}"));
        }
    } else if let Err(error) = fs::write(&args.output, trace.bytes()) {
        return fail(format_args!(
            "cannot write trace to '{}': {error}",
            args.output.display()
        ));
    }

    if let Some(path) = &args.hash_file
        && let Err(error) = fs::write(path, format!("{}\n", trace.hash()))
    {
        return fail(format_args!(
            "cannot write hash to '{}': {error}",
            path.display()
        ));
    }

    eprintln!("trace hash: {}", trace.hash());
    ExitCode::SUCCESS
}

/// Arguments for `baseline`.
#[derive(Args)]
struct BaselineArgs {
    /// Path to the JSON5 scenario.
    scenario: PathBuf,

    /// Root seed recorded for every preset.
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Simulated seconds to cover in every preset.
    #[arg(long, default_value_t = DEFAULT_BASELINE_DURATION_S)]
    duration_s: f64,

    /// Destination for the baseline manifest; `-` writes it to stdout.
    #[arg(long, short, default_value = "-")]
    output: PathBuf,

    /// Also write the non-normative wall-clock performance report here.
    #[arg(long)]
    performance: Option<PathBuf>,
}

fn baseline(args: BaselineArgs) -> ExitCode {
    let (scenario, content_sha256) = match load_scenario_hashed(&args.scenario) {
        Ok(loaded) => loaded,
        Err(error) => return fail(error),
    };

    let source_path = args.scenario.to_string_lossy().into_owned();
    let (manifest, performance) = match capture(CaptureRequest {
        scenario: &scenario,
        source_path: &source_path,
        content_sha256: &content_sha256,
        seed: args.seed,
        duration_s: args.duration_s,
    }) {
        Ok(captured) => captured,
        Err(error) => return fail(error),
    };

    if let Err(error) = write_json(&args.output, &manifest) {
        return fail(format_args!(
            "cannot write baseline manifest to '{}': {error}",
            args.output.display()
        ));
    }
    if let Some(path) = &args.performance
        && let Err(error) = write_json(path, &performance)
    {
        return fail(format_args!(
            "cannot write performance report to '{}': {error}",
            path.display()
        ));
    }

    if let Some(standard) = manifest
        .presets
        .iter()
        .find(|preset| preset.preset == "standard")
    {
        eprintln!("baseline standard trace hash: {}", standard.trace_sha256);
    }
    ExitCode::SUCCESS
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let mut json = serde_json::to_string_pretty(value).expect("baseline manifests serialize");
    json.push('\n');
    if path.as_os_str() == "-" {
        io::stdout().write_all(json.as_bytes())
    } else {
        fs::write(path, json)
    }
}

fn fail(error: impl Display) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::FAILURE
}
