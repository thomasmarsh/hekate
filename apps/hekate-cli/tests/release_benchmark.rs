//! Release-mode end-to-end benchmark harness (Phase 1 Increment 5, slice E).
//!
//! The harness measures the same kernel the viewer and the headless commands
//! run, but with the clock turned into evidence: for each declared scenario it
//! reports simulated seconds per wall second, agent steps per second, and output
//! bytes per simulated hour, together with the machine, toolchain, and method.
//!
//! It is ignored by default and writes the checked-in artifact
//! `perf/release-bench.json`. Run it through `scripts/bench-release.sh`, which
//! builds the release profile first:
//!
//! ```sh
//! scripts/bench-release.sh
//! cargo test --release -p hekate-cli --test release_benchmark -- --ignored --nocapture
//! ```
//!
//! Wall-clock time is evidence about one machine at one time, never a gate: no
//! test asserts a timing number, and the five gates never run this ignored test.
//! Its only assertions are structural — the release profile is in use and the
//! runs completed — so a regression in the harness itself is visible without
//! turning host speed into a failure.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use hekate_cli::{
    RunDirectoryRequest, SamplingPolicy, canonical_run_captured, load_scenario_provenance,
    write_run_directory,
};
use hekate_sim::{RunConfig, Simulation};
use serde::Serialize;

/// Version of the release benchmark artifact format.
const ARTIFACT_VERSION: u32 = 1;

/// Simulated seconds every scenario covers; the artifact reports per-hour units.
///
/// One simulated hour at the Standard 0.05 s step is 72 000 fixed steps, long
/// enough that process start-up and scenario load are outside the measurement
/// and short enough that the sweep stays bounded.
const SIMULATED_SECONDS: f64 = 3600.0;

/// Measured passes per scenario. Every pass steps the same kernel from a fresh
/// `Simulation`, so the passes are independent samples of the same work.
const REPEATS: usize = 3;

/// Steps advanced before a measured pass, so demand admission and the announced
/// initial population are resident before the clock starts.
const WARMUP_TICKS: u64 = 2_000;

/// Root seed every scenario runs.
const SEED: u64 = 0;

/// Checked-in artifact this harness writes, relative to the repository root.
const ARTIFACT_PATH: &str = "perf/release-bench.json";

/// One scenario in the sweep and why it is in it.
struct BenchScenario {
    /// Repository-relative scenario source path.
    path: &'static str,
    /// What this scenario is the representative case for.
    role: &'static str,
}

/// The declared sweep: the walking skeleton, one scenario per interaction
/// family, the mixed gate fixture the interaction-metrics pass is measured on,
/// and the Increment 2 representative mixed-mode profile. Adding a scenario
/// here adds a row to the next capture.
const SCENARIOS: [BenchScenario; 7] = [
    BenchScenario {
        path: "scenarios/walking/walking_guide_v1.json5",
        role: "Phase 1 walking skeleton: a static constant-speed population that leaves the 120 m path and is never replaced, so the row is the empty-tick floor (6 agents, 704 agent steps over the hour)",
    },
    BenchScenario {
        path: "scenarios/benchmarks/car_following_v1.json5",
        role: "single corridor at high demand: IDM car following is the only interaction",
    },
    BenchScenario {
        path: "scenarios/benchmarks/red_light_compliance_v1.json5",
        role: "signalized approach: compliance decisions, queueing, and red-light running",
    },
    BenchScenario {
        path: "scenarios/benchmarks/pedestrian_crossing_v1.json5",
        role: "one crosswalk: pedestrian compliance and vehicle yielding across modes",
    },
    BenchScenario {
        path: "scenarios/benchmarks/four_leg_signal_v1.json5",
        role: "four-leg signalized intersection with two pedestrian crossings",
    },
    BenchScenario {
        path: "scenarios/benchmarks/mixed_interaction_v1.json5",
        role: "two crosswalks, four pedestrian routes: the Increment 3 mixed gate fixture and the scenario the interaction-metrics pass is measured on",
    },
    BenchScenario {
        path: "scenarios/phase2/inc2/mixed_mode_profile_v2.json5",
        role: "Increment 2 representative mixed-mode profile: passenger cars, bicycles, and scooters sharing one two-way corridor, so the lateral tactics (pass/overtake) and the opposing traversal run together at a declared density (480 arrivals/hour on the corridor, ~9 live bodies) (TAS-110)",
    },
];

#[derive(Serialize)]
struct Machine {
    os: String,
    os_version: String,
    arch: String,
    cpu: String,
    logical_cpus: usize,
    rustc: String,
    cargo: String,
}

#[derive(Serialize)]
struct Method {
    harness: &'static str,
    command: &'static str,
    profile: String,
    clock: &'static str,
    aggregate: &'static str,
    simulated_seconds_per_scenario: f64,
    repeats: usize,
    warmup_ticks: u64,
    agent_steps: &'static str,
    output_bytes: &'static str,
    run_directory_policy: &'static str,
    notes: [&'static str; 5],
}

#[derive(Serialize)]
struct ArtifactFile {
    name: String,
    bytes: u64,
}

#[derive(Serialize)]
struct ScenarioBenchmark {
    /// Scenario id as the compiled scenario declares it.
    scenario: String,
    /// Repository-relative source path.
    source_path: &'static str,
    /// Why the scenario is in the sweep.
    role: &'static str,
    step_s: f64,
    ticks: u64,
    simulated_seconds: f64,
    /// Wall seconds of each measured pass, in pass order.
    wall_seconds: Vec<f64>,
    /// Fastest pass, the least noise-affected sample.
    wall_seconds_min: f64,
    /// Median pass, the headline number the ratios are derived from.
    wall_seconds_median: f64,
    simulated_seconds_per_wall_second: f64,
    microseconds_per_tick: f64,
    agent_steps: u64,
    agent_steps_per_second: f64,
    spawned: u64,
    despawned: u64,
    remaining: usize,
    /// Canonical JSON Lines trace bytes for the whole simulated hour.
    canonical_trace_bytes: u64,
    canonical_trace_sha256: String,
    /// The same trace expressed per simulated hour.
    canonical_trace_bytes_per_simulated_hour: f64,
    /// Immutable run-directory bytes for the whole simulated hour: manifest,
    /// summary, gzip event stream, sampled trajectories, and metrics.
    run_directory_bytes: u64,
    run_directory_bytes_per_simulated_hour: f64,
    /// The run directory's files with their sizes.
    run_directory_files: Vec<ArtifactFile>,
}

#[derive(Serialize)]
struct BenchmarkArtifact {
    artifact_version: u32,
    kind: &'static str,
    generated_unix_s: u64,
    build_profile: String,
    method: Method,
    machine: Machine,
    scenarios: Vec<ScenarioBenchmark>,
}

/// The harness itself, as the artifact records it.
#[test]
#[ignore = "release-mode wall-clock benchmark: run scripts/bench-release.sh"]
fn release_benchmark_writes_the_checked_in_artifact() {
    if cfg!(debug_assertions) {
        panic!(
            "the release benchmark must be built with --release; scripts/bench-release.sh does that"
        );
    }

    let machine = machine();
    let mut scenarios = Vec::with_capacity(SCENARIOS.len());
    for scenario in &SCENARIOS {
        let measured = measure(scenario);
        println!(
            "{:<34} {:>10.3} s sim/s wall   {:>9.3} s wall   {:>8.3} us/tick   {:>12.0} traces bytes/hour",
            measured.scenario,
            measured.simulated_seconds_per_wall_second,
            measured.wall_seconds_median,
            measured.microseconds_per_tick,
            measured.canonical_trace_bytes_per_simulated_hour,
        );
        scenarios.push(measured);
    }

    let artifact = BenchmarkArtifact {
        artifact_version: ARTIFACT_VERSION,
        kind: "release-end-to-end-benchmark",
        generated_unix_s: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0),
        build_profile: if cfg!(debug_assertions) {
            "debug".to_owned()
        } else {
            "release".to_owned()
        },
        method: Method {
            harness: "apps/hekate-cli/tests/release_benchmark.rs",
            command: "scripts/bench-release.sh",
            profile: "release (no LTO, debug = false), the pinned toolchain in rust-toolchain.toml"
                .to_owned(),
            clock: "std::time::Instant around the fixed-step loop only: scenario load, warm-up, artifact writing, and process start-up are outside the measured interval",
            aggregate: "three independent passes per scenario; the artifact reports every pass, the fastest, and the median, and derives the ratios from the median",
            simulated_seconds_per_scenario: SIMULATED_SECONDS,
            repeats: REPEATS,
            warmup_ticks: WARMUP_TICKS,
            agent_steps: "live agents counted before each step and summed over the run, the same quantity baselines/phase1/performance.json reports",
            output_bytes: "the canonical JSON Lines trace and the immutable run directory the default sampling policy writes, each extrapolated to one simulated hour",
            run_directory_policy: "SamplingPolicy::default(): every event, trajectories sampled at stride 10 ticks up to 100 000 rows",
            notes: [
                "Wall clock is evidence about one machine at one time, not a gate: no test asserts these numbers.",
                "Single-threaded: one simulation per pass, no parallelism, so the numbers are per-core.",
                "The host was otherwise idle; runs are not pinned to a core and not repeated under load.",
                "Agent counts are small by design (tens of live bodies), so per-tick cost is dominated by the fixed per-tick passes, not by agent count.",
                "Extrapolating bytes to one simulated hour is a ratio of the measured hour, not a model of a longer run.",
            ],
        },
        machine,
        scenarios,
    };

    let path = repo_path(ARTIFACT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the artifact directory is creatable");
    }
    let mut json = serde_json::to_string_pretty(&artifact).expect("the artifact serializes");
    json.push('\n');
    fs::write(&path, json).expect("the artifact is writable");
    println!("release benchmark artifact: {}", path.display());
}

/// Measure one scenario: three timed passes plus one artifact pass.
fn measure(scenario: &BenchScenario) -> ScenarioBenchmark {
    let path = repo_path(scenario.path);
    let (compiled, provenance) = load_scenario_provenance(&path)
        .unwrap_or_else(|error| panic!("{} loads: {error}", scenario.path));
    let config = RunConfig::new(SEED);
    let step_s = config.step().as_secs();
    let ticks = (SIMULATED_SECONDS / step_s).round() as u64;

    // Warm-up: the same kernel from a fresh simulation, so the measured passes
    // never pay first-touch costs for code and pages.
    let mut warm = Simulation::new(compiled.clone(), config).expect("the scenario builds");
    for _ in 0..WARMUP_TICKS.min(ticks) {
        warm.step();
    }
    drop(warm);

    let mut wall_seconds = Vec::with_capacity(REPEATS);
    let mut agent_steps = 0u64;
    for _ in 0..REPEATS {
        let mut sim = Simulation::new(compiled.clone(), config).expect("the scenario builds");
        let mut steps = 0u64;
        let start = Instant::now();
        for _ in 0..ticks {
            steps += sim.agent_count() as u64;
            sim.step();
        }
        wall_seconds.push(start.elapsed().as_secs_f64());
        agent_steps = steps;
    }

    // The artifact pass runs the recording path the `run` command uses, so the
    // trace and the run directory are the artifacts a run directory holds and
    // not a second implementation of them.
    let sampling = SamplingPolicy::default();
    let (trace, summary, trajectories, metrics) =
        canonical_run_captured(compiled, config, ticks, &sampling.trajectories)
            .expect("the artifact pass runs");
    let directory = std::env::temp_dir().join(format!("hekate-release-bench-{}", provenance.id));
    let _ = fs::remove_dir_all(&directory);
    write_run_directory(
        &directory,
        RunDirectoryRequest {
            scenario: &provenance,
            seed: SEED,
            step_s,
            sampling,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )
    .expect("the run directory is writable");
    let run_directory_files = directory_files(&directory);
    let run_directory_bytes: u64 = run_directory_files.iter().map(|file| file.bytes).sum();
    let _ = fs::remove_dir_all(&directory);

    let mut sorted = wall_seconds.clone();
    sorted.sort_by(f64::total_cmp);
    let wall_seconds_min = sorted[0];
    let wall_seconds_median = median(&sorted);
    let canonical_trace_bytes = trace.bytes().len() as u64;
    let seconds_per_hour = SIMULATED_SECONDS / 3600.0;

    ScenarioBenchmark {
        scenario: provenance.id.clone(),
        source_path: scenario.path,
        role: scenario.role,
        step_s,
        ticks,
        simulated_seconds: ticks as f64 * step_s,
        wall_seconds,
        wall_seconds_min,
        wall_seconds_median,
        simulated_seconds_per_wall_second: ticks as f64 * step_s / wall_seconds_median,
        microseconds_per_tick: wall_seconds_median * 1e6 / ticks as f64,
        agent_steps,
        agent_steps_per_second: agent_steps as f64 / wall_seconds_median,
        spawned: summary.spawned(),
        despawned: summary.despawned(),
        remaining: summary.remaining(),
        canonical_trace_bytes,
        canonical_trace_sha256: trace.hash().to_owned(),
        canonical_trace_bytes_per_simulated_hour: canonical_trace_bytes as f64 / seconds_per_hour,
        run_directory_bytes,
        run_directory_bytes_per_simulated_hour: run_directory_bytes as f64 / seconds_per_hour,
        run_directory_files,
    }
}

/// File sizes of one run directory, sorted by name so a diff is readable.
fn directory_files(directory: &Path) -> Vec<ArtifactFile> {
    let mut files: Vec<ArtifactFile> = fs::read_dir(directory)
        .expect("the run directory is readable")
        .map(|entry| entry.expect("the run directory entry is readable"))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then(|| ArtifactFile {
                name: entry.file_name().to_string_lossy().into_owned(),
                bytes: metadata.len(),
            })
        })
        .collect();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    files
}

fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

/// Machine identity: the OS, CPU, and toolchain the numbers belong to.
///
/// Every lookup degrades to `unknown` rather than failing, because the artifact
/// must be writable on a host whose identity commands differ or are absent.
fn machine() -> Machine {
    Machine {
        os: std::env::consts::OS.to_owned(),
        os_version: command_stdout("sw_vers", &["-productVersion"])
            .or_else(|| command_stdout("uname", &["-r"]))
            .unwrap_or_else(|| "unknown".to_owned()),
        arch: std::env::consts::ARCH.to_owned(),
        cpu: command_stdout("sysctl", &["-n", "machdep.cpu.brand_string"])
            .unwrap_or_else(|| "unknown".to_owned()),
        logical_cpus: std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1),
        rustc: command_stdout("rustc", &["-V"]).unwrap_or_else(|| "unknown".to_owned()),
        cargo: command_stdout("cargo", &["-V"]).unwrap_or_else(|| "unknown".to_owned()),
    }
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
