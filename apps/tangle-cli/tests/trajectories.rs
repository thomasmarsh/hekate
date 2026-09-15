//! Contract tests for the run directory's Parquet sampled trajectories.
//!
//! The trajectory artifact is the per-agent motion record a consumer plots or
//! aggregates, so these tests pin what it holds and what bounds it: the rows
//! round-trip in canonical order through Parquet, the declared policy bounds
//! the row count, full trajectories are opt-in and never truncated, the
//! manifest links the file back to its run, and a completed run directory stays
//! immutable.
//!
//! The bounds are checked against a reference frame set replayed straight
//! through the kernel, so a sampling rule that dropped, duplicated, or reordered
//! a row fails here rather than passing because the test reimplemented the
//! writer.

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use tangle_cli::{
    DEFAULT_MAX_TRAJECTORY_SAMPLES, DEFAULT_TRAJECTORY_STRIDE_TICKS, EVENT_STREAM_FILE,
    MANIFEST_FILE, METRICS_FILE, RunDirectoryError, RunDirectoryRequest, RunManifest, RunMetrics,
    SUMMARY_FILE, SamplingPolicy, ScenarioProvenance, TRAJECTORY_FILE, TRAJECTORY_FORMAT,
    TRAJECTORY_FORMAT_VERSION, TrajectoryRetention, TrajectorySample, TrajectorySampling,
    canonical_run_captured, load_scenario_provenance, read_trajectories, write_run_directory,
    write_trajectories,
};
use tangle_model::{BodyKind, CompiledScenario, MovementDirection, PermissionEffect};
use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
/// A Phase 1 mixed-mode fixture: one road movement and four pedestrian routes.
const MIXED: &str = "scenarios/benchmarks/mixed_interaction_v1.json5";
const GOLDEN_SEED: u64 = 0;
const GOLDEN_TICKS: u64 = 250;

/// A version-2 fixture whose rider enters at the reference end and travels the
/// facility's reverse traversal, against its authored forward nominal direction.
/// A `nominal_direction` `permit` statement binds the pair, so every row the
/// rider contributes carries the wrong-way rule state.
const OPPOSING_V2: &str = r#"
{
  schema_version: 2,
  id: 'opposing_state_v2',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  boundaries: [ { id: 'world', points: [
    { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
    { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
  ] } ],
  regions: [ { id: 'band', points: [
    { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
    { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
  ] } ],
  facilities: [
    { id: 'bikeway', region: 'band', reference_path: 'guide',
      width_m: 3.0, nominal_direction: 'forward',
      access: { modes: [ 'rider' ] }, lateral_use: 'shared',
      speed_policy: { limit_mps: null } },
  ],
  movements: [
    { id: 'against', from: 'exit', to: 'entry', path: 'guide', priority: 0,
      direction: 'reverse' },
  ],
  mode_templates: [
    {
      id: 'rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 6.0, max: 6.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
    },
  ],
  permissions: [
    { id: 'contraflow_bikeway', kind: 'nominal_direction', holder: 'rider',
      target: 'bikeway', effect: 'permit' },
  ],
  demand: [
    { id: 'rider_inflow', mode: 'rider',
      spawn: { rate: {
        portal: 'exit',
        rate_per_hour: 900.0,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'against', weight: 1.0 } ] },
      } } },
  ],
}
"#;

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "tangle-cli-trajectories-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Load the checked-in walking scenario and the provenance of its bytes.
fn walking() -> (CompiledScenario, ScenarioProvenance) {
    load_scenario_provenance(&repo_path(WALKING)).expect("walking scenario loads")
}

/// Run the golden walking run under `policy` and capture its kept rows.
fn golden_run(
    policy: &TrajectorySampling,
) -> (
    ScenarioProvenance,
    tangle_cli::Trace,
    tangle_sim::RunSummary,
    Vec<TrajectorySample>,
    RunMetrics,
) {
    let (scenario, provenance) = walking();
    let (trace, summary, trajectories, metrics) =
        canonical_run_captured(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS, policy)
            .expect("run completes");
    (provenance, trace, summary, trajectories, metrics)
}

/// Write the golden walking run into `directory` under `policy`.
fn write_golden_run(directory: &Path, policy: SamplingPolicy) {
    let (scenario, trace, summary, trajectories, metrics) = golden_run(&policy.trajectories);
    write_run_directory(
        directory,
        RunDirectoryRequest {
            scenario: &scenario,
            seed: GOLDEN_SEED,
            step_s: RunConfig::new(GOLDEN_SEED).step().as_secs(),
            sampling: policy,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )
    .expect("run directory is written");
}

/// Every live agent frame the golden run produces, in canonical order: one
/// entry per live agent per completed tick, replayed straight through the
/// kernel rather than through the recorder under test.
fn every_agent_frame() -> Vec<(u64, u32)> {
    let (scenario, _) = walking();
    let mut sim = Simulation::new(scenario, RunConfig::new(GOLDEN_SEED)).expect("builds");
    let mut frames = Vec::new();
    for _ in 0..GOLDEN_TICKS {
        sim.step();
        let tick = sim.time().tick();
        for agent in sim.snapshot(SnapshotDetail::Full).agents() {
            frames.push((tick, agent.id.get()));
        }
    }
    frames
}

/// The `(tick, agent)` identity of each sample, which is what order and
/// coverage are asserted on.
fn identities(samples: &[TrajectorySample]) -> Vec<(u64, u32)> {
    samples
        .iter()
        .map(|sample| (sample.tick, sample.agent))
        .collect()
}

/// Sorted file names a directory holds.
fn entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", directory.display()))
        .map(|entry| {
            entry
                .expect("directory entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// SHA-256 of a file's exact bytes.
fn file_sha256(path: &Path) -> String {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()));
    format!("{:x}", Sha256::digest(&bytes))
}

/// The content hash of every file a directory holds, sorted by file name.
fn content_hashes(directory: &Path) -> Vec<(String, String)> {
    entries(directory)
        .into_iter()
        .map(|name| {
            let hash = file_sha256(&directory.join(&name));
            (name, hash)
        })
        .collect()
}

fn read_manifest(directory: &Path) -> RunManifest {
    let json = std::fs::read_to_string(directory.join(MANIFEST_FILE)).expect("manifest is written");
    serde_json::from_str(&json).expect("manifest is JSON")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The sampled rows round-trip through Parquet in canonical order, and they are
/// exactly the frames the default policy's stride selects.
#[test]
fn sampled_trajectories_round_trip_in_canonical_order() {
    let scratch = Scratch::new("round-trip");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir, SamplingPolicy::default());
    let samples = read_trajectories(&run_dir).expect("trajectories read back");

    let expected: Vec<(u64, u32)> = every_agent_frame()
        .into_iter()
        .filter(|(tick, _)| tick.is_multiple_of(DEFAULT_TRAJECTORY_STRIDE_TICKS))
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(
        identities(&samples),
        expected,
        "the artifact must hold exactly the sampled frames in canonical order"
    );
    assert!(
        samples
            .windows(2)
            .all(|pair| pair[0].tick < pair[1].tick || pair[0].agent < pair[1].agent),
        "rows must ascend by tick, then by agent"
    );

    // Field values are the kernel's own: every walking-scenario agent is a
    // vehicle, alive, and moving east along the guide path.
    for sample in &samples {
        assert_eq!(sample.mode, "vehicle");
        assert!(sample.speed_mps > 0.0, "{sample:?}");
        assert_eq!(sample.y_m, 0.0, "{sample:?}");
    }
    let first = &samples[0];
    let last = &samples[samples.len() - 1];
    assert_eq!(first.tick, DEFAULT_TRAJECTORY_STRIDE_TICKS);
    // The position agrees with the simulated time the tick and speed imply.
    let elapsed_s = RunConfig::new(GOLDEN_SEED).step().as_secs() * first.tick as f64;
    let expected_x_m = first.speed_mps * elapsed_s;
    assert!(
        (first.x_m - expected_x_m).abs() < 1e-9,
        "first sample sits at {} m, not the {expected_x_m} m its speed and tick imply",
        first.x_m
    );
    assert!(last.x_m > first.x_m, "the sampled run must make progress");
}

/// A Phase 1 run's trajectory artifact reports every body's envelope kind: a
/// vehicle is a box and a pedestrian a circle, each a single envelope with no
/// ordered segments. The rows round-trip through Parquet carrying those fields.
#[test]
fn the_artifact_reports_each_phase1_body_kind_with_no_segments() {
    let scratch = Scratch::new("body-kind");
    let policy = SamplingPolicy::full_trajectories();
    let (scenario, _) = load_scenario_provenance(&repo_path(MIXED)).expect("scenario loads");
    let (_, _, trajectories, _) = canonical_run_captured(
        scenario,
        RunConfig::new(GOLDEN_SEED),
        400,
        &policy.trajectories,
    )
    .expect("run completes");
    write_trajectories(&scratch.dir, &trajectories).expect("trajectories write");
    let samples = read_trajectories(&scratch.dir).expect("trajectories read back");

    let vehicle = samples
        .iter()
        .find(|sample| sample.mode == "vehicle")
        .expect("the mixed fixture spawns a vehicle");
    assert_eq!(vehicle.body_kind, BodyKind::Box);
    assert!(vehicle.segments.is_empty());
    let pedestrian = samples
        .iter()
        .find(|sample| sample.mode == "pedestrian")
        .expect("the mixed fixture spawns a pedestrian");
    assert_eq!(pedestrian.body_kind, BodyKind::Circle);
    assert!(pedestrian.segments.is_empty());
}

/// The declared default policy bounds the artifact: a sampled artifact holds
/// fewer rows than the run has agent frames, and never more than the declared
/// cap.
#[test]
fn the_default_policy_bounds_the_written_sample_count() {
    let scratch = Scratch::new("bounded");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir, SamplingPolicy::default());
    let samples = read_trajectories(&run_dir).expect("trajectories read back");
    let frames = every_agent_frame();

    assert!(samples.len() as u64 <= DEFAULT_MAX_TRAJECTORY_SAMPLES);
    assert!(
        samples.len() < frames.len(),
        "the default policy must sample a subset: {} rows for {} agent frames",
        samples.len(),
        frames.len()
    );
}

/// A cap smaller than the sampled frame set is the hard bound: the artifact
/// holds exactly the first `max_samples` rows in canonical order and nothing
/// past them.
#[test]
fn a_bounded_cap_writes_exactly_the_declared_maximum() {
    let scratch = Scratch::new("cap");
    let run_dir = scratch.path("run");
    let policy = SamplingPolicy {
        trajectories: TrajectorySampling {
            retention: TrajectoryRetention::Sampled,
            stride_ticks: 1,
            max_samples: 5,
        },
        ..SamplingPolicy::default()
    };

    write_golden_run(&run_dir, policy);
    let samples = read_trajectories(&run_dir).expect("trajectories read back");
    let frames = every_agent_frame();

    assert_eq!(samples.len(), 5, "the cap must bound the row count");
    assert_eq!(
        identities(&samples),
        frames[..5].to_vec(),
        "the artifact must hold the first rows in canonical order"
    );
    assert_eq!(entries(&run_dir).len(), 5);
}

/// Full trajectories are opt-in through the command line, and a policy that
/// keeps every tick is not truncated: the artifact holds every agent frame of
/// the run.
#[test]
fn full_trajectories_are_opt_in_and_never_truncated() {
    let scratch = Scratch::new("full");
    let sampled_dir = scratch.path("sampled");
    let full_dir = scratch.path("full");
    let scenario = repo_path(WALKING);

    write_golden_run(&sampled_dir, SamplingPolicy::default());
    let sampled = read_trajectories(&sampled_dir).expect("sampled trajectories read back");

    let output = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args(["--seed", "0", "--ticks", "250"])
        .args(["--output", "trace.jsonl"])
        .args(["--run-dir", "full"])
        .arg("--full-trajectories")
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let full = read_trajectories(&full_dir).expect("full trajectories read back");
    let frames = every_agent_frame();
    assert_eq!(
        identities(&full),
        frames,
        "a full trajectory keeps every tick, with no truncation"
    );
    assert!(full.len() > sampled.len());

    let manifest = read_manifest(&full_dir);
    let applied = manifest.sampling.trajectories;
    assert_eq!(applied.retention, TrajectoryRetention::Full);
    assert_eq!(applied.stride_ticks, 1);
    assert_eq!(applied.max_samples, u64::MAX);
    assert_eq!(
        manifest.trajectories.expect("artifact").rows,
        full.len() as u64
    );
    assert_eq!(
        read_manifest(&sampled_dir).sampling.trajectories.retention,
        TrajectoryRetention::Sampled
    );
}

/// `--full-trajectories` only means something for a run directory, so the
/// command rejects it rather than silently ignoring it.
#[test]
fn full_trajectories_require_a_run_directory() {
    let scratch = Scratch::new("usage");
    let scenario = repo_path(WALKING);

    let output = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args(["--output", "trace.jsonl"])
        .arg("--full-trajectories")
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("run-dir"), "{}", stderr(&output));
    assert!(
        !scratch.path("trace.jsonl").exists(),
        "a usage error wrote a trace"
    );
}

/// The manifest links the artifact to its run: the descriptor names the file
/// inside the run directory, its hash is the file's own hash, its row count is
/// the row count a reader sees, and the manifest carries the policy and run
/// identity that produced both.
#[test]
fn the_manifest_links_the_trajectory_artifact_to_its_run() {
    let scratch = Scratch::new("link");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir, SamplingPolicy::default());

    let manifest = read_manifest(&run_dir);
    let artifact = manifest.trajectories.expect("the artifact descriptor");
    assert_eq!(artifact.path, TRAJECTORY_FILE);
    assert_eq!(artifact.format, TRAJECTORY_FORMAT);
    assert_eq!(
        artifact.sha256,
        file_sha256(&run_dir.join(TRAJECTORY_FILE)),
        "the descriptor must name the file's exact bytes"
    );
    let samples = read_trajectories(&run_dir).expect("trajectories read back");
    assert_eq!(artifact.rows, samples.len() as u64);
    assert_eq!(manifest.scenario.id, "walking_guide_v1");
    assert_eq!(manifest.seed, GOLDEN_SEED);
    assert_eq!(manifest.ticks, GOLDEN_TICKS);
    assert_eq!(
        manifest.sampling.trajectories.retention,
        TrajectoryRetention::Sampled
    );
    // The manifest names the artifact, so the run directory reproduces the rows.
    let (_, _, _, expected, _) = golden_run(&manifest.sampling.trajectories);
    assert_eq!(samples, expected);
}

/// A completed run directory with a trajectory artifact is immutable: a repeat
/// run under a different policy fails and leaves every artifact, including the
/// Parquet file, byte-identical.
#[test]
fn a_completed_run_with_trajectories_is_never_rewritten() {
    let scratch = Scratch::new("immutable");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir, SamplingPolicy::default());
    let before = content_hashes(&run_dir);
    let rows_before = read_trajectories(&run_dir)
        .expect("trajectories read back")
        .len();

    let error = {
        let (scenario, trace, summary, trajectories, metrics) =
            golden_run(&SamplingPolicy::full_trajectories().trajectories);
        write_run_directory(
            &run_dir,
            RunDirectoryRequest {
                scenario: &scenario,
                seed: GOLDEN_SEED,
                step_s: RunConfig::new(GOLDEN_SEED).step().as_secs(),
                sampling: SamplingPolicy::full_trajectories(),
                trace: &trace,
                trajectories: &trajectories,
                summary: &summary,
                metrics: &metrics,
            },
        )
        .expect_err("a completed run directory rejects a rerun")
    };

    assert!(
        matches!(&error, RunDirectoryError::Completed { path } if path == &run_dir),
        "unexpected error: {error}"
    );
    assert_eq!(
        content_hashes(&run_dir),
        before,
        "the rejected rerun mutated a completed artifact"
    );
    assert_eq!(
        read_trajectories(&run_dir)
            .expect("trajectories read back")
            .len(),
        rows_before,
        "the rejected rerun replaced the sampled artifact"
    );
}

/// The ordinary single-envelope rows the walking run's artifact holds: they
/// carry no route state at all, so the rule-state columns stay null rather than
/// defaulting to a direction or a rule.
#[test]
fn a_phase1_run_carries_no_rule_state_in_any_row() {
    let scratch = Scratch::new("no-rule-state");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir, SamplingPolicy::default());
    let samples = read_trajectories(&run_dir).expect("trajectories read back");

    assert!(!samples.is_empty());
    for sample in &samples {
        assert_eq!(sample.route_s_m, None, "{sample:?}");
        assert_eq!(sample.perceived_rule, None, "{sample:?}");
        assert_eq!(sample.opposing_direction, None, "{sample:?}");
    }
}

/// The artifact carries a real run's wrong-way rule state and a completed run
/// directory that holds it is immutable: the rule-state rows round-trip through
/// Parquet with the direction the body travels and the rule it perceived, the
/// manifest names the file's exact bytes under the unchanged artifact version,
/// and a rerun under another policy is refused with every byte untouched.
#[test]
fn the_artifact_records_rule_state_and_a_completed_run_stays_immutable() {
    let scratch = Scratch::new("rule-state");
    let scenario_path = scratch.path("opposing.json5");
    std::fs::write(&scenario_path, OPPOSING_V2).expect("the fixture is written");
    let (scenario, provenance) =
        load_scenario_provenance(&scenario_path).expect("the fixture loads");
    let policy = SamplingPolicy::full_trajectories();
    let (trace, summary, trajectories, metrics) = canonical_run_captured(
        scenario,
        RunConfig::new(GOLDEN_SEED),
        400,
        &policy.trajectories,
    )
    .expect("the run completes");
    let run_dir = scratch.path("run");
    write_run_directory(
        &run_dir,
        RunDirectoryRequest {
            scenario: &provenance,
            seed: GOLDEN_SEED,
            step_s: RunConfig::new(GOLDEN_SEED).step().as_secs(),
            sampling: policy,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )
    .expect("run directory is written");

    let samples = read_trajectories(&run_dir).expect("trajectories read back");
    let opposing: Vec<&TrajectorySample> = samples
        .iter()
        .filter(|sample| sample.opposing_direction.is_some())
        .collect();
    assert!(
        !opposing.is_empty(),
        "an opposing traversal must write the rule state"
    );
    for sample in &opposing {
        assert_eq!(sample.opposing_direction, Some(MovementDirection::Reverse));
        assert_eq!(sample.perceived_rule, Some(PermissionEffect::Permit));
        assert!(sample.route_s_m.is_some(), "the state rides route state");
    }

    // The artifact is still version 3 and the descriptor names its exact bytes.
    let manifest = read_manifest(&run_dir);
    let artifact = manifest.trajectories.expect("the artifact descriptor");
    assert_eq!(TRAJECTORY_FORMAT_VERSION, 3);
    assert_eq!(artifact.format_version, TRAJECTORY_FORMAT_VERSION);
    assert_eq!(
        artifact.sha256,
        file_sha256(&run_dir.join(TRAJECTORY_FILE)),
        "the descriptor must name the file's exact bytes"
    );
    assert_eq!(artifact.rows, samples.len() as u64);

    // A completed run directory is immutable, rule-state rows included.
    let before = content_hashes(&run_dir);
    let error = {
        let (scenario, _) =
            load_scenario_provenance(&scenario_path).expect("the fixture loads again");
        let (trace, summary, trajectories, metrics) = canonical_run_captured(
            scenario,
            RunConfig::new(GOLDEN_SEED),
            400,
            &SamplingPolicy::default().trajectories,
        )
        .expect("the shorter-policy run completes");
        write_run_directory(
            &run_dir,
            RunDirectoryRequest {
                scenario: &provenance,
                seed: GOLDEN_SEED,
                step_s: RunConfig::new(GOLDEN_SEED).step().as_secs(),
                sampling: SamplingPolicy::default(),
                trace: &trace,
                trajectories: &trajectories,
                summary: &summary,
                metrics: &metrics,
            },
        )
        .expect_err("a completed run directory rejects a rerun")
    };
    assert!(
        matches!(&error, RunDirectoryError::Completed { path } if path == &run_dir),
        "unexpected error: {error}"
    );
    assert_eq!(
        content_hashes(&run_dir),
        before,
        "the rejected rerun mutated a completed artifact"
    );
    assert_eq!(
        read_trajectories(&run_dir).expect("trajectories read back"),
        samples,
        "the rejected rerun replaced the rule-state rows"
    );
}

/// A policy that retains no trajectory state keeps the event-stream-only
/// directory: no Parquet file, no descriptor.
#[test]
fn a_policy_that_retains_no_trajectories_writes_no_artifact() {
    let scratch = Scratch::new("no-trajectories");
    let run_dir = scratch.path("run");
    let policy = SamplingPolicy {
        trajectories: TrajectorySampling::off(),
        ..SamplingPolicy::default()
    };

    write_golden_run(&run_dir, policy);

    assert_eq!(
        entries(&run_dir),
        vec![
            EVENT_STREAM_FILE.to_owned(),
            MANIFEST_FILE.to_owned(),
            METRICS_FILE.to_owned(),
            SUMMARY_FILE.to_owned()
        ]
    );
    let manifest = read_manifest(&run_dir);
    assert_eq!(manifest.trajectories, None);
    assert_eq!(
        manifest.sampling.trajectories.retention,
        TrajectoryRetention::Off
    );
}

/// A run too short to reach the policy's stride still writes a readable,
/// empty artifact, so the manifest's descriptor always names a real file.
#[test]
fn a_run_shorter_than_the_stride_writes_an_empty_artifact() {
    let scratch = Scratch::new("short");
    let run_dir = scratch.path("run");
    let (scenario, provenance) = walking();
    let policy = SamplingPolicy::default();
    let (trace, summary, trajectories, metrics) = canonical_run_captured(
        scenario,
        RunConfig::new(GOLDEN_SEED),
        5,
        &policy.trajectories,
    )
    .expect("run completes");
    assert!(trajectories.is_empty(), "five ticks never reach the stride");

    write_run_directory(
        &run_dir,
        RunDirectoryRequest {
            scenario: &provenance,
            seed: GOLDEN_SEED,
            step_s: RunConfig::new(GOLDEN_SEED).step().as_secs(),
            sampling: policy,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )
    .expect("run directory is written");

    assert_eq!(
        read_manifest(&run_dir).trajectories.expect("artifact").rows,
        0
    );
    assert_eq!(
        read_trajectories(&run_dir).expect("trajectories read back"),
        Vec::new()
    );
}
