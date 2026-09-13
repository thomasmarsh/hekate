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
//! concurrently without changing any per-run trace hash. `--seed-bank <FILE>`
//! takes the seed list from a seed bank instead of `--seeds` and records the
//! bank's path and content hash in the manifest, so two batches can prove they
//! ran the same common-random-number seeds in the same order.
//!
//! `seed-bank` writes that bank: a versioned JSON artifact holding an ordered
//! seed list, which is byte-reproducible because the writer reads no clock and
//! no randomness. It is the input two variants of one experiment share so their
//! per-seed runs pair up.
//!
//! `aggregate` reads a completed batch and writes the batch-level experiment
//! record `aggregation.json`: for every metric the runs report, the across-seed
//! count, mean, spread, and 95% Student-t confidence interval, disaggregated by
//! mode pair and by movement, with every number linked to the manifests it came
//! from. It reports not-applicable and not-observed values as statuses and
//! counts them per seed rather than reading them as zero, and it mutates no run
//! artifact.
//!
//! `compare` reads two completed batches (`--a` and `--b`) and the seed bank
//! both ran, proves both sides used that one bank in that one order, pairs the
//! runs by seed, and writes `comparison.json`: the per-seed paired difference of
//! every reported metric with its mean and a paired 95% Student-t confidence
//! interval, disaggregated by mode pair and by movement and linked to both run
//! manifests. Pairing by seed cancels the between-seed variance both sides
//! share, which the unpaired per-side interval cannot. It refuses unpaired or
//! mismatched inputs and mutates no run artifact.
//!
//! `converge` runs one scenario at the Fast, Standard, and Fine fidelity presets
//! over one seed bank — one batch per fidelity, each covering the same simulated
//! duration — and writes `convergence.json`: for every metric the across-seed
//! value at each fidelity, the paired refinement change from Fast to Standard
//! and from Standard to Fine, and a convergence verdict against a declared 5%
//! relative tolerance, disaggregated by mode pair and by movement. A metric
//! whose standard-to-fine change exceeds the tolerance is reported as materially
//! sensitive rather than hidden. It writes the three batches through the batch
//! machinery's resume path, so a completed run directory is never rewritten.
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
    AGGREGATION_FILE, BATCH_MANIFEST_FILE, BatchRequest, COMPARISON_FILE, CONVERGENCE_FILE,
    CONVERGENCE_TOLERANCE, CaptureRequest, PRESETS, RunDirectoryRequest, SamplingPolicy,
    ScenarioProvenance, SeedBank, SeedBankReference, aggregate_batch, canonical_run,
    canonical_run_captured, capture, compare_batches, converge_batches, fidelity_ticks,
    load_scenario_hashed, read_seed_bank, render_validation_failure, replay_run_directory,
    run_batch, validate_scenario, write_run_directory,
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
    #[command(long_about = RUN_LONG_ABOUT)]
    Run(RunArgs),
    /// Run a scenario once per seed into an output root and write a batch manifest.
    #[command(long_about = BATCH_LONG_ABOUT)]
    Batch(BatchArgs),
    /// Write a versioned, ordered common-random-number seed bank.
    #[command(long_about = SEED_BANK_LONG_ABOUT)]
    SeedBank(SeedBankArgs),
    /// Aggregate a completed batch into a machine-readable metric distribution file.
    #[command(long_about = AGGREGATE_LONG_ABOUT)]
    Aggregate(AggregateArgs),
    /// Compare two batches run from one seed bank, pairing runs by seed.
    #[command(long_about = COMPARE_LONG_ABOUT)]
    Compare(CompareArgs),
    /// Run one scenario at the Fast, Standard, and Fine fidelities over one seed bank.
    #[command(long_about = CONVERGE_LONG_ABOUT)]
    Converge(ConvergeArgs),
    /// Reproduce a completed run directory's canonical event stream.
    #[command(long_about = REPLAY_LONG_ABOUT)]
    Replay(ReplayArgs),
    /// Check a scenario source and report diagnostics without running it.
    #[command(long_about = VALIDATE_LONG_ABOUT)]
    Validate(ValidateArgs),
    /// Capture the deterministic Phase 1 baseline and a performance report.
    Baseline(BaselineArgs),
}

/// The `run` contract shown by `--help`: usage, inputs, outputs, exit codes.
const RUN_LONG_ABOUT: &str = "\
Run a scenario and emit its canonical trace and hash. The trace goes to stdout
by default or to --output, and the SHA-256 of its exact bytes is printed to
stderr and optionally written to --hash-file. With --run-dir the run also writes
an immutable run directory holding manifest.json, summary.json, the
gzip-compressed canonical event stream, and the sampled trajectories; a
completed run directory is never rewritten.

Usage:
  tangle-cli run <SCENARIO> [--seed <N>] [--ticks <N>] [--output <FILE>] [--hash-file <FILE>] [--run-dir <DIR>] [--full-trajectories]

Inputs:
  <SCENARIO>           Path to a JSON5 scenario source document.
  --seed <N>           Root seed recorded for the run (default 0).
  --ticks <N>          Fixed steps to advance (default 250).
  --output <FILE>      Destination for the canonical trace; `-` is stdout.
  --hash-file <FILE>   Also write the trace's lowercase SHA-256 to this path.
  --run-dir <DIR>      Also write an immutable run directory here, holding
                       manifest.json, summary.json, the gzip-compressed
                       canonical event stream, and the sampled trajectories.
  --full-trajectories  Sample nothing: keep every tick's trajectory in the run
                       directory. Requires --run-dir.

Output:
  The canonical trace on stdout when --output is `-`, otherwise in the given
  file; the trace hash on stderr.

Exit codes:
  0  the run completed and its artifacts were written
  1  the scenario cannot be read or is invalid, a destination cannot be
     written, or the run directory already holds a completed run or foreign
     content
  2  command-line usage error";

/// The `batch` contract shown by `--help`: inputs, resume, parallelism, exits.
const BATCH_LONG_ABOUT: &str = "\
Run one scenario once per seed, each seed into its own immutable run directory
under the output root, and write the batch manifest batch.json listing every run
in ascending seed order.

Usage:
  tangle-cli batch <SCENARIO> (--seeds <LIST> | --seed-bank <FILE>) --out-root <DIR> [--ticks <N>] [--jobs <N>] [--full-trajectories]

Inputs:
  <SCENARIO>   Path to a JSON5 scenario source document.
  --seeds      Comma-separated seeds, e.g. `--seeds 0,1,2`; sorted and deduplicated.
  --seed-bank  A seed bank file whose ordered seeds the batch runs, e.g. one bank
               shared by two variants of an experiment. Malformed, duplicated,
               empty, and non-ascending banks are refused. Exactly one of
               --seeds and --seed-bank is required.
  --out-root   Directory holding batch.json and one seed-<n> run directory per seed.
  --jobs       Whole runs to execute at once; every run stays single-threaded.

Output:
  batch.json names the specification, the seed list, and each run's directory,
  trace hash, and manifest hash. A batch run from a seed bank also names the
  bank's path and content hash, so a comparison can prove both sides ran one
  bank; a --seeds batch omits that field. Each seed directory is a run
  directory: the same manifest.json, summary.json, compressed event stream, and
  sampled trajectories `run --run-dir` writes.

Resume:
  A seed whose run directory already holds manifest.json is skipped and never
  touched; a directory without it is cleared and re-run. Re-invoking a completed
  batch rewrites batch.json and changes no completed artifact.

Exit codes:
  0  every run in the batch is complete
  1  a run failed, a run directory did not match the batch, a seed bank could
     not be read or is invalid, or an artifact could not be written
  2  command-line usage error";

/// The `seed-bank` contract shown by `--help`: inputs, ordering, exits.
const SEED_BANK_LONG_ABOUT: &str = "\
Write a versioned common-random-number seed bank: the ordered seed list two
variants of one scenario share so their per-seed runs are paired and a paired
A/B comparison cancels the between-seed variance both sides carry.

Usage:
  tangle-cli seed-bank --seeds <LIST> [--output <PATH>]
  tangle-cli seed-bank --start <N> --count <N> [--stride <N>] [--output <PATH>]

Inputs:
  --seeds   Comma-separated seeds, e.g. `--seeds 0,1,2`.
  --start   First seed of an arithmetic bank.
  --count   How many seeds that bank holds; must be positive.
  --stride  Step between consecutive seeds; defaults to 1, must be positive.

Output:
  A seed bank document with seed_bank_version 1 and an ordered seeds list,
  written to --output, or to stdout for `-` (the default).

Ordering:
  The seeds are written strictly ascending and unique. The order is canonical,
  not the order the request listed, so equal seed sets produce byte-identical
  banks and a bank's content hash identifies the pairing. A repeated seed is
  refused rather than collapsed. The writer reads no clock and no randomness, so
  the same request always produces the same bytes.

Exit codes:
  0  the seed bank was written
  1  the request is empty or repeats a seed, the arithmetic sequence overflows,
     or the output could not be written
  2  command-line usage error";

/// The `aggregate` contract shown by `--help`: inputs, output, method, exits.
const AGGREGATE_LONG_ABOUT: &str = "\
Aggregate a completed batch into one machine-readable aggregation.json: for every
metric the batch's runs report, the across-seed count, mean, spread, and two-sided
95% Student-t confidence interval, disaggregated by mode pair and by movement,
with every number linked to its run manifest and to metric_definition_version 1.

Usage:
  tangle-cli aggregate <BATCH_ROOT> [--output <PATH>]

Inputs:
  <BATCH_ROOT>  A completed batch root holding batch.json and one run directory
                per seed, each with its manifest.json and metrics.json.
  --output      Destination for the aggregation; `-` writes it to stdout.
                Defaults to aggregation.json inside the batch root.

Output:
  aggregation.json names the batch manifest, the seeds read, the interval
  method, and one distribution per metric, mode-pair slice, and movement slice.
  A value the runs call not applicable or not observed is counted as a seed
  behind that status, never averaged in as zero. The output orders metrics,
  slices, and seeds deterministically.

Exit codes:
  0  every run's metrics were read, checked, and aggregated
  1  the batch root holds no readable batch.json, a run's artifacts are missing,
     unreadable, or disagree with what the batch recorded, or the aggregation
     could not be written
  2  command-line usage error";

/// The `compare` contract shown by `--help`: inputs, output, method, exits.
const COMPARE_LONG_ABOUT: &str = "\
Compare two completed batches that ran one common-random-number seed bank. The
comparison proves both sides used that bank in that one seed order, pairs the
runs by seed, and for every reported metric computes the per-seed difference
d_i = A(seed_i) - B(seed_i) with its mean and a paired two-sided 95% Student-t
confidence interval, disaggregated by mode pair and by movement and linked to
both run manifests and to metric_definition_version 1.

Usage:
  tangle-cli compare --a <BATCH_ROOT> --b <BATCH_ROOT> --seed-bank <FILE> [--output <PATH>]

Inputs:
  --a          Batch root of side A: its values are the minuend of every
               difference, so a positive mean difference means A is greater.
  --b          Batch root of side B: its values are the subtrahend.
  --seed-bank  The bank both batches ran; it must hash to the bank content hash
               each batch manifest recorded.
  --output     Destination for the comparison; `-` writes it to stdout.
               Defaults to comparison.json in the current directory.

Method:
  The interval is the two-sided Student-t interval on the mean of the per-seed
differences, mean_difference +/- t(0.975, n - 1) * s_d / sqrt(n), with the n - 1
sample standard deviation of the differences. Pairing by seed removes the
between-seed variance both variants share, so the paired interval is narrower
than either side's unpaired across-seed interval; the unpaired interval contains
the seed-to-seed spread that cancels in the paired difference. A pair with a
not-applicable or not-observed value on either side is excluded from the
statistic and counted with both sides' statuses.

Output:
  comparison.json names the seed bank and both batch manifests, the paired runs,
the documented method, and one distribution per metric, mode-pair slice, and
movement slice. Metrics, slices, and seeds are ordered deterministically. No run
artifact is created, changed, or removed.

Exit codes:
  0  the two batches were proven to share one bank and compared
  1  a batch names no bank, the batches name different banks or orders, the bank
     file disagrees with what they recorded, a seed is missing, duplicated, or
     unpaired, a run disagrees with what its batch recorded, a metric is
     reported by one side only, or the comparison could not be written
  2  command-line usage error";

/// The `converge` contract shown by `--help`: inputs, method, output, exits.
const CONVERGE_LONG_ABOUT: &str = "\
Run one scenario at the Fast, Standard, and Fine fidelities over one
common-random-number seed bank and write convergence.json: for every metric the
across-seed value at each fidelity, the paired refinement change from Fast to
Standard and from Standard to Fine, and a convergence verdict against the
declared tolerance, disaggregated by mode pair and by movement.

Usage:
  tangle-cli converge <SCENARIO> --seed-bank <FILE> --out-root <DIR> [--ticks <N>] [--output <PATH>]

Inputs:
  <SCENARIO>   Path to a JSON5 scenario source document.
  --seed-bank  The bank every fidelity runs; each fidelity's batch records it, so
               the report can prove all three ran one bank in one seed order.
  --out-root   Directory holding one batch root per fidelity — fast/,
               standard/, and fine/ — each the batch.json and seed-<n> run
               directories `batch` writes.
  --ticks      Fixed steps the Standard fidelity advances; the Fast and Fine
               fidelities advance the whole number of steps that covers the same
               simulated duration, because the presets differ in step size.
  --output     Destination for the report; `-` writes it to stdout. Defaults to
               convergence.json in the current directory.

Method:
  A metric's value at a fidelity is the across-seed distribution the aggregation
reports: count, mean, spread, interval, and the seeds behind each reporting
status. A refinement step's change is the paired mean difference of that one
bank's runs at the two fidelities, with its paired 95% Student-t interval. The
declared tolerance is 5% of the coarser fidelity's across-seed value, measured
as the relative change |fine - coarse| / |coarse|: a metric whose
standard-to-fine relative change exceeds it is materially sensitive, and the
report names it. A metric no seed reports at both steps is inconclusive; a
not-applicable or not-observed value is a status, never a zero; and a zero
coarser value has no relative scale, so a zero change is converged and any other
change is materially sensitive.

Output:
  convergence.json names the scenario, the seed bank, the three fidelity batches
with their ticks and artifact hashes, the declared tolerance, and one record per
metric, mode-pair slice, and movement slice. Fidelities and refinement steps are
in the declared Fast, Standard, Fine order, metrics and slices are ordered
deterministically, and no run artifact is mutated: a completed run directory is
never rewritten, so re-invoking the command resumes the batches and changes
nothing but the report.

Exit codes:
  0  the three fidelities ran and the report was written
  1  the scenario or seed bank could not be read, a fidelity's batch is missing
     or unreadable, a batch does not run its fidelity's declared step or duration,
     the fidelities name two scenarios or two banks, a run disagrees with what its
     batch recorded, or the report could not be written
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
        Command::SeedBank(args) => seed_bank(args),
        Command::Aggregate(args) => aggregate(args),
        Command::Compare(args) => compare(args),
        Command::Converge(args) => converge(args),
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
#[command(group = clap::ArgGroup::new("seed_source").required(true).args(["seeds", "seed_bank"]))]
struct BatchArgs {
    /// Path to the JSON5 scenario.
    scenario: PathBuf,

    /// Seeds to run, comma-separated and sorted ascending, e.g. `0,1,2`.
    #[arg(long, value_delimiter = ',')]
    seeds: Vec<u64>,

    /// Run the ordered seeds of a seed bank, e.g. one bank shared by two
    /// variants. The bank's path and content hash are recorded in the manifest.
    #[arg(long)]
    seed_bank: Option<PathBuf>,

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

    // Exactly one of `--seeds` and `--seed-bank` is present. A bank is read and
    // validated before any run starts, and its identity is recorded at the path
    // the command named it by rather than a canonicalized one.
    let (seeds, seed_bank) = match &args.seed_bank {
        Some(path) => match read_seed_bank(path) {
            Ok(loaded) => (
                loaded.bank.seeds,
                Some(SeedBankReference {
                    path: path.display().to_string(),
                    content_sha256: loaded.content_sha256,
                }),
            ),
            Err(error) => return fail(error),
        },
        None => (args.seeds, None),
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
        seeds,
        seed_bank,
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

/// Arguments for `seed-bank`.
#[derive(Args)]
#[command(group = clap::ArgGroup::new("request").required(true).args(["seeds", "start"]))]
struct SeedBankArgs {
    /// Seeds to write, comma-separated; written ascending, duplicates refused.
    #[arg(long, value_delimiter = ',')]
    seeds: Vec<u64>,

    /// First seed of an arithmetic bank.
    #[arg(long, requires = "count")]
    start: Option<u64>,

    /// How many seeds an arithmetic bank holds.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..), requires = "start")]
    count: Option<u64>,

    /// Step between consecutive seeds of an arithmetic bank; defaults to 1.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..), requires = "start")]
    stride: Option<u64>,

    /// Destination for the seed bank; `-` writes it to stdout.
    #[arg(long, short, default_value = "-")]
    output: PathBuf,
}

/// Write a versioned, ordered common-random-number seed bank.
///
/// The bank is a pure function of the request — no clock and no randomness — so
/// repeating the command reproduces the same bytes, and two batches that run
/// the same bank artifact can prove it by the bank's content hash.
fn seed_bank(args: SeedBankArgs) -> ExitCode {
    let bank = if args.seeds.is_empty() {
        let start = args.start.expect("clap requires --start or --seeds");
        let count = args.count.expect("clap requires --count with --start");
        SeedBank::from_range(start, count, args.stride.unwrap_or(1))
    } else {
        SeedBank::from_seeds(&args.seeds)
    };
    let bank = match bank {
        Ok(bank) => bank,
        Err(error) => return fail(error),
    };

    if let Err(error) = write_json(&args.output, &bank) {
        return fail(format_args!(
            "cannot write seed bank to '{}': {error}",
            args.output.display()
        ));
    }

    eprintln!(
        "seed bank: {} ({} seeds)",
        args.output.display(),
        bank.seeds.len()
    );
    ExitCode::SUCCESS
}

/// Arguments for `aggregate`.
#[derive(Args)]
struct AggregateArgs {
    /// Completed batch root holding batch.json and one run directory per seed.
    batch_root: PathBuf,

    /// Destination for the aggregation; `-` writes it to stdout.
    ///
    /// Defaults to `aggregation.json` inside the batch root. The aggregation is
    /// derived and deterministic, so writing it again is a no-op in content;
    /// no run artifact is ever written.
    #[arg(long)]
    output: Option<PathBuf>,
}

/// Aggregate a completed batch's per-seed metrics into one machine-readable
/// file of across-seed distributions and confidence intervals.
fn aggregate(args: AggregateArgs) -> ExitCode {
    let aggregation = match aggregate_batch(&args.batch_root) {
        Ok(aggregation) => aggregation,
        Err(error) => return fail(error),
    };

    let output = args
        .output
        .unwrap_or_else(|| args.batch_root.join(AGGREGATION_FILE));
    if let Err(error) = write_json(&output, &aggregation) {
        return fail(format_args!(
            "cannot write aggregation to '{}': {error}",
            output.display()
        ));
    }

    eprintln!(
        "aggregation: {} ({} metrics over {} seeds)",
        output.display(),
        aggregation.metrics.len(),
        aggregation.seeds.len()
    );
    ExitCode::SUCCESS
}

/// Arguments for `compare`.
#[derive(Args)]
struct CompareArgs {
    /// Batch root of side A, whose values are the minuend of every difference.
    #[arg(long)]
    a: PathBuf,

    /// Batch root of side B, whose values are the subtrahend.
    #[arg(long)]
    b: PathBuf,

    /// The seed bank both batches ran; it must hash to what they recorded.
    #[arg(long)]
    seed_bank: PathBuf,

    /// Destination for the comparison; `-` writes it to stdout.
    ///
    /// Defaults to `comparison.json` in the current directory. The comparison is
    /// derived and deterministic, so writing it again is a no-op in content; no
    /// run artifact is ever written.
    #[arg(long)]
    output: Option<PathBuf>,
}

/// Compare two batches run from one seed bank into one machine-readable file of
/// paired per-metric differences and confidence intervals.
fn compare(args: CompareArgs) -> ExitCode {
    let comparison = match compare_batches(&args.a, &args.b, &args.seed_bank) {
        Ok(comparison) => comparison,
        Err(error) => return fail(error),
    };

    let output = args
        .output
        .unwrap_or_else(|| PathBuf::from(COMPARISON_FILE));
    if let Err(error) = write_json(&output, &comparison) {
        return fail(format_args!(
            "cannot write comparison to '{}': {error}",
            output.display()
        ));
    }

    eprintln!(
        "comparison: {} ({} metrics over {} paired seeds)",
        output.display(),
        comparison.metrics.len(),
        comparison.pairs.len()
    );
    ExitCode::SUCCESS
}

/// Arguments for `converge`.
#[derive(Args)]
struct ConvergeArgs {
    /// Path to the JSON5 scenario.
    scenario: PathBuf,

    /// The seed bank every fidelity runs, e.g. one bank shared by three runs of
    /// one experiment.
    #[arg(long)]
    seed_bank: PathBuf,

    /// Output root holding one batch root per fidelity.
    #[arg(long)]
    out_root: PathBuf,

    /// Fixed steps the Standard fidelity advances.
    ///
    /// The Fast and Fine fidelities advance the whole number of steps that
    /// covers the same simulated duration, because the presets differ in step
    /// size and a fixed tick count would compare different horizons.
    #[arg(long, default_value_t = DEFAULT_TICKS, value_parser = clap::value_parser!(u64).range(1..))]
    ticks: u64,

    /// Destination for the report; `-` writes it to stdout.
    ///
    /// Defaults to `convergence.json` in the current directory. The report is
    /// derived and deterministic, so writing it again is a no-op in content; no
    /// run artifact is ever written or rewritten beyond the batch machinery's
    /// own resume path.
    #[arg(long, default_value = CONVERGENCE_FILE)]
    output: PathBuf,
}

/// Run one scenario at the three fidelity presets over one seed bank and write
/// the sensitivity report.
///
/// The three batches go through `run_batch`, so a completed run directory is
/// skipped rather than rewritten and the report reads the artifacts actually on
/// disk. `PRESETS` is the declared Fast, Standard, Fine order the report records
/// and reads back.
fn converge(args: ConvergeArgs) -> ExitCode {
    let (scenario, content_sha256) = match load_scenario_hashed(&args.scenario) {
        Ok(loaded) => loaded,
        Err(error) => return fail(error),
    };
    let bank = match read_seed_bank(&args.seed_bank) {
        Ok(loaded) => loaded,
        Err(error) => return fail(error),
    };
    let seed_bank = SeedBankReference {
        path: args.seed_bank.display().to_string(),
        content_sha256: bank.content_sha256,
    };
    let provenance = ScenarioProvenance {
        id: scenario.id().to_owned(),
        source_path: args.scenario.to_string_lossy().into_owned(),
        schema_version: scenario.schema_version(),
        content_sha256,
    };

    let mut roots: Vec<PathBuf> = Vec::with_capacity(PRESETS.len());
    for preset in PRESETS {
        let root = args.out_root.join(preset.name);
        let request = BatchRequest {
            root: root.clone(),
            scenario: scenario.clone(),
            provenance: provenance.clone(),
            ticks: fidelity_ticks(preset.step_s, args.ticks),
            step_s: preset.step_s,
            sampling: SamplingPolicy::default(),
            seeds: bank.bank.seeds.clone(),
            seed_bank: Some(seed_bank.clone()),
            jobs: 1,
        };
        if let Err(error) = run_batch(request) {
            return fail(error);
        }
        roots.push(root);
    }

    let report = match converge_batches(
        &roots[0],
        &roots[1],
        &roots[2],
        &args.seed_bank,
        CONVERGENCE_TOLERANCE,
    ) {
        Ok(report) => report,
        Err(error) => return fail(error),
    };

    if let Err(error) = write_json(&args.output, &report) {
        return fail(format_args!(
            "cannot write convergence report to '{}': {error}",
            args.output.display()
        ));
    }

    eprintln!(
        "convergence: {} ({} metrics over {} fidelities)",
        args.output.display(),
        report.metrics.len(),
        report.fidelities.len()
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
    let mut json = serde_json::to_string_pretty(value).expect("JSON artifacts serialize");
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
