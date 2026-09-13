//! Headless command-line interface for Tangle.
//!
//! `run` loads a JSON5 scenario, advances the kernel for a fixed number of
//! ticks, and writes the canonical trace. The trace hash printed to stderr is
//! the value a golden test or manifest compares. When `--run-dir` is given it
//! also writes the immutable per-run directory holding `manifest.json`,
//! `summary.json`, and the gzip-compressed canonical event stream; a completed
//! run directory is never rewritten, so that destination fails rather than
//! mutating finished artifacts.
//!
//! `validate` loads the same source through the same loader but stops there:
//! it reports whether the schema and the semantic invariants hold, exits
//! non-zero on any invalid input, and advances no tick or run artifact. It is
//! the check a CI job or a pre-batch step runs.
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
    CaptureRequest, RunDirectoryRequest, ScenarioProvenance, canonical_run, capture,
    load_scenario_hashed, render_validation_failure, validate_scenario, write_run_directory,
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
    /// Check a scenario source and report diagnostics without running it.
    #[command(long_about = VALIDATE_LONG_ABOUT)]
    Validate(ValidateArgs),
    /// Capture the deterministic Phase 1 baseline and a performance report.
    Baseline(BaselineArgs),
}

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
    /// summary.json, and the gzip-compressed canonical event stream. The
    /// directory must not hold a completed run, which is never rewritten.
    #[arg(long)]
    run_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run(args) => run(args),
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
    let (trace, summary) = match canonical_run(scenario, config, args.ticks) {
        Ok(run) => run,
        Err(error) => return fail(error),
    };

    // The run directory is written before the trace output: a completed run
    // directory rejects the rerun, and that rejection must not also emit a
    // trace to the caller's destination.
    if let Some(directory) = &args.run_dir
        && let Err(error) = write_run_directory(
            directory,
            RunDirectoryRequest {
                scenario: &provenance,
                seed: args.seed,
                step_s: config.step().as_secs(),
                trace: &trace,
                summary: &summary,
            },
        )
    {
        return fail(error);
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
