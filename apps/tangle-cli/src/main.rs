//! Headless command-line interface for Tangle.
//!
//! `run` loads a JSON5 scenario, advances the kernel for a fixed number of
//! ticks, and writes the canonical trace. The trace hash printed to stderr is
//! the value a golden test or manifest compares. When `--run-dir` is given it
//! also writes the immutable per-run directory holding `manifest.json`,
//! `summary.json`, the gzip-compressed canonical event stream, and the Parquet
//! sampled trajectories; a completed run directory is never rewritten, so that
//! destination fails rather than mutating finished artifacts. The trajectories
//! are sampled at the stride and cap the manifest declares, and
//! `--full-trajectories` opts into keeping every tick instead.
//!
//! `validate` loads the same source through the same loader but stops there:
//! it reports whether the schema and the semantic invariants hold, exits
//! non-zero on any invalid input, and advances no tick or run artifact. It is
//! the check a CI job or a pre-batch step runs.
//!
//! `batch` runs one scenario once per seed, each seed into its own immutable run
//! directory under an output root, and writes the batch manifest `batch.json`
//! linking every run in ascending seed order. It is resumable — a run directory
//! that already holds a completed run is skipped, and one without the completion
//! marker is re-run — and `--jobs` runs whole independent single-threaded runs
//! concurrently without changing any per-run trace hash.
//!
//! `replay` reproduces a completed run directory from its manifest: it re-loads
//! the recorded scenario source, checks the recorded content hash, and re-runs
//! the kernel with the recorded seed, step, and tick count, emitting the
//! reproduced canonical event stream to stdout. `--verify` additionally checks
//! the reproduction against the recorded `events.jsonl.gz` and its trace hash.
//! Replay writes no artifact and never mutates a run directory.
//!
//! `baseline` captures the deterministic Phase 1 baseline manifest at every
//! fidelity preset and optionally a non-normative wall-clock report.

use std::fmt::Display;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use tangle_cli::{
    BATCH_MANIFEST_FILE, BatchRequest, CaptureRequest, RunDirectoryRequest, SamplingPolicy,
    ScenarioProvenance, canonical_run, canonical_run_captured, capture, load_scenario_hashed,
    render_validation_failure, replay_run_directory, run_batch, validate_scenario,
    write_run_directory,
};
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
    /// Run a scenario once per seed into an output root and write a batch manifest.
    #[command(long_about = BATCH_LONG_ABOUT)]
    Batch(BatchArgs),
    /// Reproduce a completed run directory's canonical event stream.
    #[command(long_about = REPLAY_LONG_ABOUT)]
    Replay(ReplayArgs),
    /// Check a scenario source and report diagnostics without running it.
    #[command(long_about = VALIDATE_LONG_ABOUT)]
    Validate(ValidateArgs),
    /// Capture the deterministic Phase 1 baseline and a performance report.
    Baseline(BaselineArgs),
}

/// The `batch` contract shown by `--help`: inputs, resume, parallelism, exits.
const BATCH_LONG_ABOUT: &str = "\
Run one scenario once per seed, each seed into its own immutable run directory
under the output root, and write the batch manifest batch.json listing every run
in ascending seed order.

Usage:
  tangle-cli batch <SCENARIO> --seeds <LIST> --out-root <DIR> [--ticks <N>] [--jobs <N>] [--full-trajectories]

Inputs:
  <SCENARIO>  Path to a JSON5 scenario source document.
  --seeds     Comma-separated seeds, e.g. `--seeds 0,1,2`; sorted and deduplicated.
  --out-root  Directory holding batch.json and one seed-<n> run directory per seed.
  --jobs      Whole runs to execute at once; every run stays single-threaded.

Output:
  batch.json names the specification, the seed list, and each run's directory,
  trace hash, and manifest hash. Each seed directory is a run directory: the
  same manifest.json, summary.json, compressed event stream, and sampled
  trajectories `run --run-dir` writes.

Resume:
  A seed whose run directory already holds manifest.json is skipped and never
  touched; a directory without it is cleared and re-run. Re-invoking a completed
  batch rewrites batch.json and changes no completed artifact.

Exit codes:
  0  every run in the batch is complete
  1  a run failed, a run directory did not match the batch, or an artifact could
     not be written
  2  command-line usage error";

/// The `replay` contract shown by `--help`: usage, inputs, output, exit codes.
const REPLAY_LONG_ABOUT: &str = "\
Reproduce a completed run directory's canonical event stream. The manifest
records the scenario source and content hash, the seed, the fixed step, and the
tick count, and replay re-runs the kernel with exactly those, writing the
reproduced stream to stdout. Replay reads the run directory and the recorded
scenario source only: it writes no artifact and mutates nothing.

Usage:
  tangle-cli replay <RUN_DIR> [--verify]

Inputs:
  <RUN_DIR>  A completed run directory holding manifest.json and events.jsonl.gz.
  --verify   Also check the reproduction against the recorded stream and hash.

Output:
  The reproduced canonical event stream on stdout, decompressed to canonical
  record order, and the reproduced trace hash on stderr.

Exit codes:
  0  the run was reproduced, and verified when --verify was given
  1  the run directory is missing or incomplete, the scenario source no longer
     matches the recorded content hash, a recorded artifact cannot be read, or
     the reproduction disagrees with the recorded run
  2  command-line usage error";

/// The `validate` contract shown by `--help`: usage, inputs, and exit codes.
const VALIDATE_LONG_ABOUT: &str = "\
Check a scenario source against the scenario schema and the semantic
invariants the loader enforces, then stop. No tick is advanced and no run
artifact is written.

Usage:
  tangle-cli validate <SCENARIO>

Inputs:
  <SCENARIO>  Path to a JSON5 scenario source document.

Output:
  One report line on stdout when the loader accepts the source; every
  diagnostic, one per line, on stderr when it does not.

Exit codes:
  0  the loader accepts the source
  1  the source cannot be read, is not valid JSON5 for the source schema, or
     fails semantic validation
  2  command-line usage error";

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

    /// Also write an immutable run directory here, holding manifest.json,
    /// summary.json, the gzip-compressed canonical event stream, and the
    /// sampled trajectories the declared policy retains as Parquet. The
    /// directory must not hold a completed run, which is never rewritten.
    #[arg(long)]
    run_dir: Option<PathBuf>,

    /// Sample nothing: keep every tick's trajectory in the run directory.
    ///
    /// Full trajectories are opt-in and are never truncated; without this flag
    /// the run directory holds the bounded sample the default policy declares.
    /// Requires `--run-dir`.
    #[arg(long, requires = "run_dir")]
    full_trajectories: bool,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run(args) => run(args),
        Command::Batch(args) => batch(args),
        Command::Replay(args) => replay(args),
        Command::Validate(args) => validate(args),
        Command::Baseline(args) => baseline(args),
    }
}

fn run(args: RunArgs) -> ExitCode {
    let (scenario, content_sha256) = match load_scenario_hashed(&args.scenario) {
        Ok(loaded) => loaded,
        Err(error) => return fail(error),
    };

    let provenance = ScenarioProvenance {
        id: scenario.id().to_owned(),
        source_path: args.scenario.to_string_lossy().into_owned(),
        schema_version: scenario.schema_version(),
        content_sha256,
    };

    let config = RunConfig::new(args.seed);
    let sampling = sampling_policy(args.full_trajectories);
    // Without a run directory nothing consumes the trajectories or the metric
    // capture, so the run takes the plain path and pays neither cost.
    let (trace, summary, trajectories, metrics) = if args.run_dir.is_some() {
        match canonical_run_captured(scenario, config, args.ticks, &sampling.trajectories) {
            Ok((trace, summary, trajectories, metrics)) => {
                (trace, summary, trajectories, Some(metrics))
            }
            Err(error) => return fail(error),
        }
    } else {
        match canonical_run(scenario, config, args.ticks) {
            Ok((trace, summary)) => (trace, summary, Vec::new(), None),
            Err(error) => return fail(error),
        }
    };

    // The run directory is written before the trace output: a completed run
    // directory rejects the rerun, and that rejection must not also emit a
    // trace to the caller's destination.
    if let Some(directory) = &args.run_dir {
        let metrics = metrics
            .as_ref()
            .expect("a run directory always captures metrics");
        if let Err(error) = write_run_directory(
            directory,
            RunDirectoryRequest {
                scenario: &provenance,
                seed: args.seed,
                step_s: config.step().as_secs(),
                sampling,
                trace: &trace,
                trajectories: &trajectories,
                summary: &summary,
                metrics,
            },
        ) {
            return fail(error);
        }
    }

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

/// The sampling policy a command declares and applies.
///
/// The applied policy is what the manifest records, so a consumer reading only
/// the run directory can tell a bounded sample from a full trajectory.
fn sampling_policy(full_trajectories: bool) -> SamplingPolicy {
    if full_trajectories {
        SamplingPolicy::full_trajectories()
    } else {
        SamplingPolicy::default()
    }
}

/// Arguments for `batch`.
#[derive(Args)]
struct BatchArgs {
    /// Path to the JSON5 scenario.
    scenario: PathBuf,

    /// Seeds to run, comma-separated and sorted ascending, e.g. `0,1,2`.
    #[arg(long, value_delimiter = ',', required = true)]
    seeds: Vec<u64>,

    /// Number of fixed steps every run advances.
    #[arg(long, default_value_t = DEFAULT_TICKS)]
    ticks: u64,

    /// Output root for the batch manifest and one run directory per seed.
    #[arg(long)]
    out_root: PathBuf,

    /// Whole runs to execute at once; every run stays single-threaded.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    jobs: u64,

    /// Keep every tick's trajectory in each run directory.
    #[arg(long)]
    full_trajectories: bool,
}

fn batch(args: BatchArgs) -> ExitCode {
    let (scenario, content_sha256) = match load_scenario_hashed(&args.scenario) {
        Ok(loaded) => loaded,
        Err(error) => return fail(error),
    };

    let provenance = ScenarioProvenance {
        id: scenario.id().to_owned(),
        source_path: args.scenario.to_string_lossy().into_owned(),
        schema_version: scenario.schema_version(),
        content_sha256,
    };
    let out_root = args.out_root;
    let manifest = match run_batch(BatchRequest {
        root: out_root.clone(),
        scenario,
        provenance,
        ticks: args.ticks,
        step_s: RunConfig::new(0).step().as_secs(),
        sampling: sampling_policy(args.full_trajectories),
        seeds: args.seeds,
        jobs: args.jobs,
    }) {
        Ok(manifest) => manifest,
        Err(error) => return fail(error),
    };

    eprintln!(
        "batch manifest: {} ({} runs)",
        out_root.join(BATCH_MANIFEST_FILE).display(),
        manifest.runs.len()
    );
    ExitCode::SUCCESS
}

/// Arguments for `replay`.
#[derive(Args)]
struct ReplayArgs {
    /// Completed run directory holding manifest.json and events.jsonl.gz.
    run_dir: PathBuf,

    /// Also verify the reproduction against the recorded stream and trace hash.
    #[arg(long)]
    verify: bool,
}

/// Reproduce a completed run directory and write its canonical event stream to
/// stdout.
///
/// Reproduction and verification both succeed before any stream is written, so
/// a verified replay's stdout is exactly the bytes a `run` with the same
/// parameters produced, and a failed verification emits no stream at all.
fn replay(args: ReplayArgs) -> ExitCode {
    let trace = match replay_run_directory(&args.run_dir, args.verify) {
        Ok(trace) => trace,
        Err(error) => return fail(error),
    };

    if let Err(error) = io::stdout().write_all(trace.bytes()) {
        return fail(format_args!(
            "cannot write replay stream to stdout: {error}"
        ));
    }
    eprintln!("replay trace hash: {}", trace.hash());
    if args.verify {
        eprintln!("replay verified: {}", args.run_dir.display());
    }
    ExitCode::SUCCESS
}

/// Arguments for `validate`.
#[derive(Args)]
struct ValidateArgs {
    /// Path to the JSON5 scenario.
    scenario: PathBuf,
}

fn validate(args: ValidateArgs) -> ExitCode {
    match validate_scenario(&args.scenario) {
        Ok(summary) => {
            println!(
                "{}: valid scenario '{}' (schema version {})",
                summary.source_path.display(),
                summary.scenario_id,
                summary.schema_version
            );
            ExitCode::SUCCESS
        }
        Err(error) => fail(render_validation_failure(&error)),
    }
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
