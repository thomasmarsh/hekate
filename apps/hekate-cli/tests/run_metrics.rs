//! Contract tests for the run directory's versioned `metrics.json`.
//!
//! The metrics artifact is what makes a run directory self-describing for
//! multi-seed aggregation, so these tests pin every value it reports against
//! the in-process `Simulation::interaction_metrics()` accessors and against the
//! decompressed event stream — not against a reimplementation of the writer.
//! They cover the run-level minima, the per-`ModePair` separation slice, the
//! per-movement slice keyed by each pair's two movement keys, the event counts
//! with their mode and movement slices, the explicit no-value statuses, and the
//! immersion of the artifact into `run --run-dir`, `batch`, and resume.
//!
//! `serde_json` writes a float as its shortest round-trippable decimal, but its
//! parser rounds to within one ULP of the true value, so a value read back
//! through `serde_json` can differ from the in-process value by at most one ULP.
//! The comparisons below allow exactly that much and nothing more.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::read::GzDecoder;
use hekate_cli::{
    EVENT_STREAM_FILE, MANIFEST_FILE, METRIC_DEFINITION_VERSION, METRICS_FILE, MetricStatus,
    MetricValue, OperationalValues, RunDirectoryRequest, RunMetrics, RunMetricsArtifact,
    RunMetricsRecorder, SUMMARY_FILE, SamplingPolicy, ScenarioProvenance, WrongWayValues,
    canonical_run_captured, load_scenario_provenance, replay_run_directory, write_run_directory,
};
use hekate_model::{
    CompiledScenario, FacilityId, MovementDirection, MovementId, NominalDirection,
    PermissionEffect, TacticKind, parse_scenario_source_v2,
};
use hekate_sim::{
    AgentId, AgentMode, DespawnReason, Event, ManeuverEdge, ManeuverState, MetricMinimum, ModePair,
    MovementKey, OperationValues, OvertakeObservation, PostEncroachment, RunConfig, Simulation,
    SnapshotDetail, WrongWayReason, wrong_way::OpposingTraversalObservation,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

const MIXED: &str = "scenarios/benchmarks/mixed_interaction_v1.json5";
const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
/// Long enough that the mixed benchmark records a reported time to collision,
/// a cross-mode pair, and several movement buckets; still under a second.
const MIXED_TICKS: u64 = 600;
/// Long enough that the mixed benchmark also serves agents, which the
/// operational families need: its shortest route takes about 33 simulated
/// seconds, so 30 seconds of run serves none.
const MIXED_OPERATIONAL_TICKS: u64 = 3000;
const GOLDEN_TICKS: u64 = 250;
/// Fast ticks for the CLI/batch wiring tests.
const WIRING_TICKS: &str = "60";

/// Every family metric definition v1 counts.
const FAMILIES: [&str; 10] = [
    "collisions",
    "near_misses",
    "violations",
    "region_entries",
    "region_exits",
    "queue_events",
    "control_transitions",
    "yields",
    "spawns",
    "despawns",
];

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-cli-run-metrics-{name}-{}",
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

fn load(relative: &str) -> (CompiledScenario, ScenarioProvenance) {
    load_scenario_provenance(&repo_path(relative)).expect("scenario loads")
}

/// Run one scenario through the capturing loop and write its run directory.
fn write_run(directory: &Path, relative: &str, seed: u64, ticks: u64) -> RunMetrics {
    let (scenario, provenance) = load(relative);
    let sampling = SamplingPolicy::default();
    let (trace, summary, trajectories, metrics) = canonical_run_captured(
        scenario,
        RunConfig::new(seed),
        ticks,
        &sampling.trajectories,
    )
    .expect("run completes");
    write_run_directory(
        directory,
        RunDirectoryRequest {
            scenario: &provenance,
            seed,
            step_s: RunConfig::new(seed).step().as_secs(),
            sampling,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )
    .expect("run directory is written");
    metrics
}

fn read_artifact(directory: &Path) -> RunMetricsArtifact {
    let json = std::fs::read_to_string(directory.join(METRICS_FILE)).expect("metrics is written");
    serde_json::from_str(&json).expect("metrics is JSON")
}

/// The canonical event records, decompressed from the run directory.
fn event_records(directory: &Path) -> Vec<Value> {
    let compressed = std::fs::read(directory.join(EVENT_STREAM_FILE)).expect("stream is written");
    let mut decoded = Vec::new();
    GzDecoder::new(&compressed[..])
        .read_to_end(&mut decoded)
        .expect("event stream decompresses");
    String::from_utf8(decoded)
        .expect("the stream is UTF-8")
        .lines()
        .filter(|line| line.contains(r#""kind":"event""#))
        .map(|line| serde_json::from_str(line).expect("each record is JSON"))
        .collect()
}

/// The in-process metric accessors of a fresh, independent run of the same
/// scenario and seed.
struct Reference {
    minimum_ttc_s: Option<MetricMinimum>,
    minimum_separation_m: Option<MetricMinimum>,
    minimum_post_encroachment_s: Option<PostEncroachment>,
    mode_pair: BTreeMap<String, Option<MetricMinimum>>,
    pair_separation: BTreeMap<(u32, u32), f64>,
    pair_ttc: BTreeMap<(u32, u32), f64>,
    movement_key: BTreeMap<u32, String>,
    pets: Vec<PostEncroachment>,
}

fn reference(relative: &str, seed: u64, ticks: u64) -> Reference {
    let (scenario, _) = load(relative);
    let mut sim = Simulation::new(scenario, RunConfig::new(seed)).expect("simulation builds");
    let mut spawned = BTreeSet::new();
    for _ in 0..ticks {
        let output = sim.step();
        for event in output.events() {
            if let Event::Spawned { agent, .. } = event {
                spawned.insert(agent.get());
            }
        }
    }

    let interaction = sim.interaction_metrics();
    let ids: Vec<u32> = spawned.into_iter().collect();
    let mut pair_separation = BTreeMap::new();
    let mut pair_ttc = BTreeMap::new();
    let mut movement_key = BTreeMap::new();
    for (position, &first) in ids.iter().enumerate() {
        let first_id = AgentId::from_index(first as usize);
        if let Some(key) = reference_movement_key(&sim, first_id) {
            movement_key.insert(first, key);
        }
        for &second in &ids[position + 1..] {
            let second_id = AgentId::from_index(second as usize);
            if let Some(value) = interaction.pair_minimum_separation_m(first_id, second_id) {
                pair_separation.insert((first, second), value);
            }
            if let Some(value) = interaction.pair_minimum_ttc_s(first_id, second_id) {
                pair_ttc.insert((first, second), value);
            }
        }
    }

    let mode_pair = [
        ModePair::VehicleVehicle,
        ModePair::VehiclePedestrian,
        ModePair::PedestrianPedestrian,
    ]
    .into_iter()
    .map(|pair| {
        (
            pair.label().to_owned(),
            interaction.mode_pair_minimum_separation_m(pair),
        )
    })
    .collect();

    Reference {
        minimum_ttc_s: interaction.minimum_ttc_s(),
        minimum_separation_m: interaction.minimum_separation_m(),
        minimum_post_encroachment_s: interaction.minimum_post_encroachment_s(),
        mode_pair,
        pair_separation,
        pair_ttc,
        movement_key,
        pets: interaction.post_encroachments().to_vec(),
    }
}

/// The movement-key spelling metric definition v1 fixes: `MovementId` for
/// vehicles and `PedestrianRouteId` for pedestrians, tagged.
fn reference_movement_key(sim: &Simulation, agent: AgentId) -> Option<String> {
    let key = match sim.agent_mode(agent)? {
        AgentMode::Vehicle => sim.agent_route(agent).map(MovementKey::Vehicle),
        AgentMode::Pedestrian => sim
            .agent_pedestrian_route(agent)
            .map(MovementKey::Pedestrian),
    };
    reference_operational_key(sim, key?)
}

/// The same spelling for one movement identity on its own, which is what an
/// operational bucket is keyed by.
fn reference_operational_key(sim: &Simulation, key: MovementKey) -> Option<String> {
    match key {
        MovementKey::Vehicle(movement) => sim
            .scenario()
            .movement(movement)
            .map(|movement| format!("movement:{}", movement.name())),
        MovementKey::Pedestrian(route) => sim
            .scenario()
            .pedestrian_route(route)
            .map(|route| format!("pedestrian_route:{}", route.name())),
    }
}

/// The operational metric of one artifact bucket, by field name, so a test can
/// walk the block without a second spelling of each metric.
fn operational_field<'a>(values: &'a OperationalValues, metric: &str) -> &'a MetricValue {
    match metric {
        "throughput_agents_per_s" => &values.throughput_agents_per_s,
        "mean_travel_time_s" => &values.mean_travel_time_s,
        "total_travel_time_s" => &values.total_travel_time_s,
        "mean_stopped_delay_s" => &values.mean_stopped_delay_s,
        "total_stopped_delay_s" => &values.total_stopped_delay_s,
        "mean_control_delay_s" => &values.mean_control_delay_s,
        "total_control_delay_s" => &values.total_control_delay_s,
        "maximum_queue_length_agents" => &values.maximum_queue_length_agents,
        "maximum_queue_duration_s" => &values.maximum_queue_duration_s,
        "mean_queue_duration_s" => &values.mean_queue_duration_s,
        other => panic!("'{other}' is not an operational metric"),
    }
}

/// The status and value metric definition v2 fixes for one absent metric: the
/// predicate that would define a rate or a length is the bucket's own
/// observation, so an observed bucket with no value is not applicable, while a
/// total, mean, or duration that has no value has no observation behind it.
fn expected_status(value: Option<f64>, applicable: bool) -> (MetricStatus, Option<f64>) {
    match value {
        Some(value) => (MetricStatus::Reported, Some(value)),
        None if applicable => (MetricStatus::NotApplicable, None),
        None => (MetricStatus::NotObserved, None),
    }
}

/// Every operational metric of one bucket with the status and value the v2
/// definition fixes, restated from the kernel's own observation.
fn expected_operational_values(
    values: &OperationValues,
) -> Vec<(&'static str, (MetricStatus, Option<f64>))> {
    let unobserved = values.observed_agents == 0;
    let served = values.served_agents > 0;
    vec![
        (
            "throughput_agents_per_s",
            expected_status(values.throughput_agents_per_s, !unobserved),
        ),
        (
            "mean_travel_time_s",
            expected_status(values.mean_travel_time_s, false),
        ),
        (
            "total_travel_time_s",
            expected_status(served.then_some(values.total_travel_time_s), false),
        ),
        (
            "mean_stopped_delay_s",
            expected_status(values.mean_stopped_delay_s, false),
        ),
        (
            "total_stopped_delay_s",
            expected_status(served.then_some(values.total_stopped_delay_s), false),
        ),
        (
            "mean_control_delay_s",
            expected_status(values.mean_control_delay_s, false),
        ),
        (
            "total_control_delay_s",
            expected_status(served.then_some(values.total_control_delay_s), false),
        ),
        (
            "maximum_queue_length_agents",
            expected_status(
                values
                    .maximum_queue_length
                    .map(|length| length.agents as f64),
                !unobserved,
            ),
        ),
        (
            "maximum_queue_duration_s",
            expected_status(
                values
                    .maximum_queue_duration
                    .map(|duration| duration.seconds),
                false,
            ),
        ),
        (
            "mean_queue_duration_s",
            expected_status(values.mean_queue_duration_s, false),
        ),
    ]
}

/// Assert one artifact metric carries the status and value the definition
/// fixes.
fn assert_operational(metric: &str, actual: &MetricValue, expected: (MetricStatus, Option<f64>)) {
    assert_eq!(actual.status, expected.0, "'{metric}' reports a status");
    match expected.1 {
        Some(value) => assert_close(actual.value, value),
        None => assert!(
            actual.value.is_none(),
            "'{metric}' must carry no value when it has none"
        ),
    }
}

/// One movement bucket recomputed from the per-pair accessors, independently of
/// the writer.
#[derive(Default)]
struct ExpectedBucket {
    separation: Option<(f64, u32, u32)>,
    ttc: Option<(f64, u32, u32)>,
    pet: Option<(f64, u32, u32)>,
}

fn keep_least(slot: &mut Option<(f64, u32, u32)>, value: f64, agent: u32, other: u32) {
    if slot.is_none_or(|current| value < current.0) {
        *slot = Some((value, agent, other));
    }
}

fn bucket_key(reference: &Reference, first: u32, second: u32) -> Option<String> {
    let first = reference.movement_key.get(&first)?;
    let second = reference.movement_key.get(&second)?;
    Some(if first <= second {
        format!("{first}|{second}")
    } else {
        format!("{second}|{first}")
    })
}

fn expected_buckets(reference: &Reference) -> BTreeMap<String, ExpectedBucket> {
    let mut buckets: BTreeMap<String, ExpectedBucket> = BTreeMap::new();
    for (&(first, second), &value) in &reference.pair_separation {
        let Some(key) = bucket_key(reference, first, second) else {
            continue;
        };
        keep_least(
            &mut buckets.entry(key).or_default().separation,
            value,
            first,
            second,
        );
    }
    for (&(first, second), &value) in &reference.pair_ttc {
        let Some(key) = bucket_key(reference, first, second) else {
            continue;
        };
        keep_least(
            &mut buckets.entry(key).or_default().ttc,
            value,
            first,
            second,
        );
    }
    for pet in &reference.pets {
        let Some(key) = bucket_key(reference, pet.preceding.get(), pet.following.get()) else {
            continue;
        };
        keep_least(
            &mut buckets.entry(key).or_default().pet,
            pet.seconds,
            pet.preceding.get(),
            pet.following.get(),
        );
    }
    buckets
}

/// A reported value read back through `serde_json` is within one ULP of the
/// in-process value; assert exactly that much and nothing more.
fn assert_close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("a reported value");
    assert!(
        (actual - expected).abs() <= f64::EPSILON * expected.abs().max(1.0),
        "value {actual} differs from the in-process {expected}"
    );
}

fn assert_reported(actual: &MetricValue, expected: MetricMinimum) {
    assert_eq!(actual.status, MetricStatus::Reported);
    assert_close(actual.value, expected.value);
    assert_eq!(actual.agent, Some(expected.agent.get()));
    assert_eq!(actual.other, Some(expected.other.get()));
    assert_eq!(
        actual.mode_pair.as_deref(),
        Some(expected.mode_pair.label())
    );
    assert_eq!(actual.tick, Some(expected.tick));
}

#[test]
fn metrics_json_reports_the_operational_families() {
    let scratch = Scratch::new("operational");
    let run_dir = scratch.path("run");

    write_run(&run_dir, MIXED, 0, MIXED_OPERATIONAL_TICKS);
    let artifact = read_artifact(&run_dir);

    // An independent run of the same scenario and seed, read through the same
    // accessors the writer reads.
    let (scenario, _) = load(MIXED);
    let config = RunConfig::new(0);
    let mut sim = Simulation::new(scenario, config).expect("simulation builds");
    let mut despawns = 0usize;
    for _ in 0..MIXED_OPERATIONAL_TICKS {
        let output = sim.step();
        despawns += output
            .events()
            .iter()
            .filter(|event| matches!(event, Event::Despawned { .. }))
            .count();
    }
    let operation = sim.interaction_metrics().operation();
    let expected = operation.values();
    let elapsed_s = MIXED_OPERATIONAL_TICKS as f64 * config.step().as_secs();

    // The mixed benchmark must actually serve an agent inside the run, or the
    // values below would be trivially absent and prove nothing.
    assert!(
        expected.served_agents > 0,
        "the mixed benchmark served no agent in {MIXED_TICKS} ticks"
    );

    // The run-level block carries exactly the status and value the definition
    // fixes for each metric, and an aggregate carries no pair or tick
    // provenance.
    let run = &artifact.operational.run;
    for (metric, expected) in expected_operational_values(&expected) {
        assert_operational(
            &format!("operational.run.{metric}"),
            operational_field(run, metric),
            expected,
        );
    }
    assert!(run.mean_travel_time_s.agent.is_none());
    assert!(run.mean_travel_time_s.tick.is_none());
    assert!(run.total_travel_time_s.other.is_none());

    // The two statistics that select one record carry it: the tick that held
    // the standing count, and the agent and tick of the longest stop.
    let length = expected
        .maximum_queue_length
        .expect("the mixed benchmark stands somewhere in the run");
    assert_eq!(run.maximum_queue_length_agents.tick, Some(length.tick));
    assert_close(run.maximum_queue_length_agents.value, length.agents as f64);
    let duration = expected
        .maximum_queue_duration
        .expect("the mixed benchmark closes a stop");
    assert_eq!(
        run.maximum_queue_duration_s.agent,
        Some(duration.agent.get())
    );
    assert_eq!(run.maximum_queue_duration_s.tick, Some(duration.tick));

    // The throughput is a property of the run and not of the observer alone:
    // the served count is exactly the stream's despawn records, and the rate is
    // that count over the run's elapsed time.
    assert_eq!(expected.served_agents as usize, despawns);
    assert_close(
        run.throughput_agents_per_s.value,
        despawns as f64 / elapsed_s,
    );
    assert_close(
        run.mean_travel_time_s.value,
        expected.total_travel_time_s / expected.served_agents as f64,
    );

    // The mode block reports both modes, each with its own bucket.
    assert_eq!(
        artifact.operational.by_mode.keys().collect::<Vec<_>>(),
        vec!["pedestrian", "vehicle"]
    );
    for (mode, values) in [AgentMode::Vehicle, AgentMode::Pedestrian]
        .into_iter()
        .map(|mode| (mode.label(), operation.mode_values(mode)))
    {
        for (metric, expected) in expected_operational_values(&values) {
            assert_operational(
                &format!("operational.by_mode.{mode}.{metric}"),
                operational_field(&artifact.operational.by_mode[mode], metric),
                expected,
            );
        }
    }

    // The movement block is keyed by the same movement spelling the v1 buckets
    // use, and holds the bucket the kernel observed for that movement.
    let mut observed: Vec<(String, OperationValues)> = operation
        .movements()
        .map(|(key, values)| {
            (
                reference_operational_key(&sim, key)
                    .expect("an observed movement is a compiled one"),
                values,
            )
        })
        .collect();
    observed.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(!observed.is_empty(), "the mixed benchmark runs movements");
    assert_eq!(
        artifact
            .operational
            .by_movement
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        observed
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>()
    );
    for (key, values) in &observed {
        let bucket = &artifact.operational.by_movement[key];
        for (metric, expected) in expected_operational_values(values) {
            assert_operational(
                &format!("operational.by_movement.{key}.{metric}"),
                operational_field(bucket, metric),
                expected,
            );
        }
    }

    // The artifact round-trips through JSON with the operational block intact.
    let json = std::fs::read_to_string(run_dir.join(METRICS_FILE)).expect("metrics is written");
    assert!(json.contains("\"operational\""));
    let decoded: RunMetricsArtifact = serde_json::from_str(&json).expect("metrics is JSON");
    assert_eq!(decoded, artifact);
}

#[test]
fn metrics_json_carries_the_versioned_run_minima_and_slices() {
    let scratch = Scratch::new("values");
    let run_dir = scratch.path("run");

    write_run(&run_dir, MIXED, 0, MIXED_TICKS);
    let artifact = read_artifact(&run_dir);
    let reference = reference(MIXED, 0, MIXED_TICKS);

    // The definition revision and the link back to the manifest.
    assert_eq!(
        artifact.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(
        artifact.manifest_sha256,
        file_sha256(&run_dir.join(MANIFEST_FILE))
    );
    let summary: hekate_cli::RunSummary = serde_json::from_str(
        &std::fs::read_to_string(run_dir.join(SUMMARY_FILE)).expect("summary is written"),
    )
    .expect("summary is JSON");
    assert_eq!(artifact.manifest_sha256, summary.manifest_sha256);
    assert_eq!(
        artifact.metric_definition_version, summary.metric_definition_version,
        "the summary must report the revision its metrics.json reports"
    );

    // The run-level minima are exactly the in-process accessors' values.
    let separation = reference
        .minimum_separation_m
        .expect("the mixed run observed a candidate pair");
    assert_reported(&artifact.minimum_separation_m, separation);
    let ttc = reference
        .minimum_ttc_s
        .expect("the mixed run observed a closing pair");
    assert_reported(&artifact.minimum_ttc_s, ttc);
    let pet = reference
        .minimum_post_encroachment_s
        .expect("the mixed run recorded a succession");
    assert_eq!(
        artifact.minimum_post_encroachment_s.status,
        MetricStatus::Reported
    );
    assert_close(artifact.minimum_post_encroachment_s.value, pet.seconds);
    assert_eq!(
        artifact.minimum_post_encroachment_s.agent,
        Some(pet.preceding.get())
    );
    assert_eq!(
        artifact.minimum_post_encroachment_s.other,
        Some(pet.following.get())
    );

    // The per-mode-pair separation slice.
    for pair in [
        ModePair::VehicleVehicle,
        ModePair::VehiclePedestrian,
        ModePair::PedestrianPedestrian,
    ] {
        let actual = &artifact.mode_pair_minimum_separation_m[pair.label()];
        match reference.mode_pair[pair.label()] {
            Some(minimum) => assert_reported(actual, minimum),
            None => {
                assert_eq!(actual.status, MetricStatus::NotObserved);
                assert!(actual.value.is_none());
            }
        }
    }

    // The per-movement slice: every bucket, its sorted movement keys, and its
    // least value over the pairs the bucket names.
    let expected = expected_buckets(&reference);
    let region_observed = event_records(&run_dir)
        .iter()
        .any(|record| record["event"] == "entry");
    assert_eq!(artifact.movement_minima.len(), expected.len());
    for (key, expected) in &expected {
        let actual = artifact
            .movement_minima
            .get(key)
            .unwrap_or_else(|| panic!("the movement bucket '{key}' is absent"));
        assert!(actual.movement_keys[0] <= actual.movement_keys[1]);
        assert_eq!(
            format!("{}|{}", actual.movement_keys[0], actual.movement_keys[1]),
            *key
        );
        match expected.separation {
            Some((value, agent, other)) => {
                assert_eq!(actual.minimum_separation_m.status, MetricStatus::Reported);
                assert_close(actual.minimum_separation_m.value, value);
                assert_eq!(actual.minimum_separation_m.agent, Some(agent));
                assert_eq!(actual.minimum_separation_m.other, Some(other));
            }
            None => assert_eq!(
                actual.minimum_separation_m.status,
                MetricStatus::NotObserved
            ),
        }
        match expected.ttc {
            Some((value, agent, other)) => {
                assert_eq!(actual.minimum_ttc_s.status, MetricStatus::Reported);
                assert_close(actual.minimum_ttc_s.value, value);
                assert_eq!(actual.minimum_ttc_s.agent, Some(agent));
                assert_eq!(actual.minimum_ttc_s.other, Some(other));
            }
            None if expected.separation.is_some() => {
                assert_eq!(actual.minimum_ttc_s.status, MetricStatus::NotApplicable);
                assert!(actual.minimum_ttc_s.value.is_none());
            }
            None => assert_eq!(actual.minimum_ttc_s.status, MetricStatus::NotObserved),
        }
        match expected.pet {
            Some((value, agent, other)) => {
                assert_eq!(
                    actual.minimum_post_encroachment_s.status,
                    MetricStatus::Reported
                );
                assert_close(actual.minimum_post_encroachment_s.value, value);
                assert_eq!(actual.minimum_post_encroachment_s.agent, Some(agent));
                assert_eq!(actual.minimum_post_encroachment_s.other, Some(other));
            }
            None if region_observed => {
                assert_eq!(
                    actual.minimum_post_encroachment_s.status,
                    MetricStatus::NotApplicable
                );
            }
            None => assert_eq!(
                actual.minimum_post_encroachment_s.status,
                MetricStatus::NotObserved
            ),
        }
    }

    // The event counts, independently counted from the canonical stream.
    assert_eq!(
        artifact.event_counts,
        expected_event_counts(&run_dir, &reference)
    );
}

/// Counts the event families straight from the canonical stream, with the mode
/// and movement slices, so the artifact is checked against its own record set.
fn expected_event_counts(directory: &Path, reference: &Reference) -> hekate_cli::EventCounts {
    let records = event_records(directory);
    let mut by_family: BTreeMap<String, u64> = FAMILIES
        .into_iter()
        .map(|family| (family.to_owned(), 0))
        .collect();
    let mut by_family_kind: BTreeMap<String, BTreeMap<String, u64>> = [
        ("violations", ["ran_red_light", "crossed_against_signal"]),
        ("control_transitions", ["signal_stop", "crossing_wait"]),
    ]
    .into_iter()
    .map(|(family, kinds)| {
        (
            family.to_owned(),
            kinds.into_iter().map(|kind| (kind.to_owned(), 0)).collect(),
        )
    })
    .collect();
    let mut by_family_mode: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut by_family_movement: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();

    let mut spawn_modes: BTreeMap<u32, String> = BTreeMap::new();
    for record in &records {
        if record["event"] == "spawned" {
            spawn_modes.insert(
                record["agent"].as_u64().expect("spawn carries an agent") as u32,
                record["mode"]
                    .as_str()
                    .expect("spawn carries a mode")
                    .to_owned(),
            );
        }
    }

    for record in &records {
        let Some((family, agent)) = counted_record(record) else {
            continue;
        };
        *by_family.get_mut(family).expect("every family is present") += 1;
        if let Some((kind_family, kind)) = counted_kind(record) {
            *by_family_kind
                .get_mut(kind_family)
                .expect("the kinded family is present")
                .get_mut(kind)
                .expect("every kind is present") += 1;
        }
        let mode = spawn_modes
            .get(&agent)
            .expect("every counted agent was spawned");
        *by_family_mode
            .entry(family.to_owned())
            .or_default()
            .entry(mode.clone())
            .or_insert(0) += 1;
        if let Some(key) = reference.movement_key.get(&agent) {
            *by_family_movement
                .entry(family.to_owned())
                .or_default()
                .entry(key.clone())
                .or_insert(0) += 1;
        }
    }

    let total = by_family.values().sum();
    hekate_cli::EventCounts {
        total,
        by_family,
        by_family_kind,
        by_family_mode,
        by_family_movement,
    }
}

/// The counted family and subject agent of one event record, or `None` when the
/// record's edge is not counted.
fn counted_record(record: &Value) -> Option<(&'static str, u32)> {
    let event = record["event"].as_str()?;
    let agent = record["agent"].as_u64()? as u32;
    let truthy = |field: &str| record[field].as_bool() == Some(true);
    let family = match event {
        "collision" if truthy("contacting") => "collisions",
        "near_miss" if truthy("entering") => "near_misses",
        "violation" => "violations",
        "entry" => "region_entries",
        "exit" => "region_exits",
        "queue" if truthy("joined") => "queue_events",
        "control_transition" => "control_transitions",
        "yielded" if truthy("yielding") => "yields",
        "spawned" => "spawns",
        "despawned" => "despawns",
        _ => return None,
    };
    Some((family, agent))
}

/// The family and variant kind of a kinded record.
fn counted_kind(record: &Value) -> Option<(&'static str, &str)> {
    match record["event"].as_str()? {
        "violation" => Some(("violations", record["violation"].as_str()?)),
        "control_transition" => Some(("control_transitions", record["control"].as_str()?)),
        _ => None,
    }
}

/// A run with no movements and no regions: the time to collision is not
/// applicable and the post-encroachment time is not observed, each an explicit
/// no-value status rather than `0`.
#[test]
fn a_not_applicable_metric_is_a_status_and_never_zero() {
    let scratch = Scratch::new("absent");
    let run_dir = scratch.path("run");

    write_run(&run_dir, WALKING, 0, GOLDEN_TICKS);
    let artifact = read_artifact(&run_dir);

    // Two vehicles follow at a constant spacing: no closing pair, so no time to
    // collision — and an explicit status, never a false `0`.
    assert_eq!(artifact.minimum_ttc_s.status, MetricStatus::NotApplicable);
    assert_eq!(artifact.minimum_ttc_s.value, None);

    // The separation is defined for the candidate pair and is positive.
    assert_eq!(artifact.minimum_separation_m.status, MetricStatus::Reported);
    assert!(artifact.minimum_separation_m.value.expect("a value") > 0.0);

    // No region was entered, so no post-encroachment observation was made.
    assert_eq!(
        artifact.minimum_post_encroachment_s.status,
        MetricStatus::NotObserved
    );
    assert_eq!(artifact.minimum_post_encroachment_s.value, None);

    // The mode slice reports the observed pair and marks the others unobserved.
    assert_eq!(
        artifact.mode_pair_minimum_separation_m["vehicle_vehicle"].status,
        MetricStatus::Reported
    );
    assert_eq!(
        artifact.mode_pair_minimum_separation_m["vehicle_pedestrian"].status,
        MetricStatus::NotObserved
    );
    assert_eq!(
        artifact.mode_pair_minimum_separation_m["pedestrian_pedestrian"].status,
        MetricStatus::NotObserved
    );

    // No body carries a movement, so the movement slice is empty.
    assert!(artifact.movement_minima.is_empty());
}

/// The event counts by family report `0` for an absent family, never omitting
/// it, so a consumer can tell a measured zero from a missing field.
#[test]
fn an_absent_event_family_is_reported_as_zero() {
    let scratch = Scratch::new("zero");
    let run_dir = scratch.path("run");

    write_run(&run_dir, WALKING, 0, GOLDEN_TICKS);
    let artifact = read_artifact(&run_dir);

    for family in FAMILIES {
        assert!(
            artifact.event_counts.by_family.contains_key(family),
            "family '{family}' must be present even at zero"
        );
    }
    assert_eq!(artifact.event_counts.by_family["collisions"], 0);
    assert!(artifact.event_counts.by_family["spawns"] > 0);
    assert_eq!(
        artifact.event_counts.total,
        artifact.event_counts.by_family.values().sum::<u64>()
    );
}

/// Run a batch command into `out_root` for `seeds`.
fn batch_command(scratch: &Scratch, out_root: &str, seeds: &str) -> Output {
    Command::new(CLI)
        .arg("batch")
        .arg(repo_path(WALKING))
        .args(["--seeds", seeds])
        .args(["--ticks", WIRING_TICKS])
        .args(["--out-root", out_root])
        .current_dir(&scratch.dir)
        .output()
        .expect("hekate-cli runs")
}

/// `run --run-dir` and `batch` both write `metrics.json`, and resume treats it
/// like every other run artifact: it is part of a partial run the batch may
/// re-run, and skipping a completed run leaves it untouched.
#[test]
fn run_and_batch_write_metrics_and_resume_treats_it_like_the_other_artifacts() {
    let scratch = Scratch::new("wiring");

    // `run --run-dir` writes the artifact, tied to the run's manifest.
    let run = Command::new(CLI)
        .arg("run")
        .arg(repo_path(WALKING))
        .args(["--seed", "0", "--ticks", WIRING_TICKS])
        .args(["--run-dir", "run"])
        .args(["--output", "trace.jsonl"])
        .current_dir(&scratch.dir)
        .output()
        .expect("hekate-cli runs");
    assert_eq!(
        run.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let run_dir = scratch.path("run");
    assert!(run_dir.join(METRICS_FILE).exists());
    let artifact = read_artifact(&run_dir);
    assert_eq!(
        artifact.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(
        artifact.manifest_sha256,
        file_sha256(&run_dir.join(MANIFEST_FILE))
    );

    // `batch` writes one complete run directory per seed, each with the
    // artifact.
    let first = batch_command(&scratch, "batch", "0,1");
    assert_eq!(
        first.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    for seed in ["seed-0", "seed-1"] {
        let directory = scratch.path("batch").join(seed);
        assert!(
            directory.join(METRICS_FILE).exists(),
            "{seed} lacks metrics.json"
        );
        assert_eq!(
            read_artifact(&directory).metric_definition_version,
            METRIC_DEFINITION_VERSION
        );
    }
    let seed1_before = content_hashes(&scratch.path("batch").join("seed-1"));

    // A directory holding only `metrics.json` is a partial run this batch may
    // re-run, not foreign content it must refuse.
    let seed2 = scratch.path("batch").join("seed-2");
    std::fs::create_dir_all(&seed2).expect("seed-2 is created");
    std::fs::write(seed2.join(METRICS_FILE), "{}\n").expect("a partial metrics file is written");
    let resumed = batch_command(&scratch, "batch", "0,1,2");
    assert_eq!(
        resumed.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert!(
        seed2.join(MANIFEST_FILE).exists(),
        "seed-2 was not completed"
    );
    assert_eq!(
        read_artifact(&seed2).metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(
        content_hashes(&scratch.path("batch").join("seed-1")),
        seed1_before,
        "the resume mutated a completed run"
    );

    // Deleting a run's completion marker leaves its `metrics.json` behind; the
    // batch re-runs that run from scratch and leaves the completed ones alone.
    std::fs::remove_file(scratch.path("batch").join("seed-0").join(MANIFEST_FILE))
        .expect("the completion marker is removed");
    let restored = batch_command(&scratch, "batch", "0,1,2");
    assert_eq!(
        restored.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        entries(&scratch.path("batch").join("seed-0")),
        vec![
            EVENT_STREAM_FILE.to_owned(),
            MANIFEST_FILE.to_owned(),
            METRICS_FILE.to_owned(),
            SUMMARY_FILE.to_owned(),
            "trajectories.parquet".to_owned(),
        ]
    );
    assert_eq!(
        content_hashes(&scratch.path("batch").join("seed-1")),
        seed1_before,
        "the resume mutated a completed run"
    );
}

/// Ticks long enough for demand to enter, catch up, and complete an overtake on
/// the close-pass fixture below: 900 ticks at the default 0.05 s step is 45 s.
const CLOSE_PASS_TICKS: u64 = 900;

/// Ticks long enough for the same demand to enter and travel but not to
/// complete an overtake, for the cases that must record none.
const NO_PASS_TICKS: u64 = 300;

/// The close-pass fixture: one shared continuous-width road that a motor mode
/// and a narrower, slower mode both ride, one demand source each, so the faster
/// motor catches a slower narrow user and passes it.
///
/// The shape mirrors `crates/hekate-sim/tests/close_pass.rs`, with the knobs
/// these cases need: the two speeds, whether the motor mode declares the
/// `overtake` tactic at all, and whether the facility is `shared` or `centered`.
fn passing_scenario(
    motor_speed_mps: f64,
    narrow_speed_mps: f64,
    overtakes: bool,
    lateral_use: &str,
) -> CompiledScenario {
    let source = parse_scenario_source_v2(&passing_scenario_source(
        motor_speed_mps,
        narrow_speed_mps,
        overtakes,
        lateral_use,
    ))
    .expect("the document is version 2");
    CompiledScenario::compile_v2(source).expect("the scenario compiles")
}

/// The authored source of the close-pass fixture, so a test can write it to a
/// file and have the run directory record a scenario `replay` can re-load.
fn passing_scenario_source(
    motor_speed_mps: f64,
    narrow_speed_mps: f64,
    overtakes: bool,
    lateral_use: &str,
) -> String {
    let tactics = match overtakes {
        true => "'follow', 'stop', 'yield', 'overtake'",
        false => "'follow', 'stop', 'yield'",
    };
    // A mode declares lateral maneuver parameters only when it carries the
    // tactic that selects one, and a centered facility offers no lateral target
    // for a passing side to apply to.
    let lateral = match overtakes {
        true => "lateral: { target_clearance_m: 0.75, horizon_s: 2.0 },",
        false => "",
    };
    let lateral_policy = match lateral_use {
        "centered" => "",
        _ => "lateral_policy: { passing_side: 'left' },",
    };
    // A wheeled box uses its steering, lateral-acceleration, and clearance
    // parameters exactly when it carries the lateral tactic that steers.
    let car_lateral_profile = match overtakes {
        true => {
            "steering_rate_max_rad_s: { min: 0.9, max: 0.9 },\n\
                 lateral_accel_max_mps2: { min: 2.0, max: 2.0 },\n\
                 lateral_clearance_m: { min: 0.3, max: 0.3 },"
        }
        false => "",
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'close_pass_metrics',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [ {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 500.0, y: 0.0 }} ] }} ],
  portals: [
    {{ id: 'entry', path: 'guide', end: 'start', width_m: 10.0 }},
    {{ id: 'exit', path: 'guide', end: 'end', width_m: 10.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -20.0, y: -20.0 }}, {{ x: 520.0, y: -20.0 }},
      {{ x: 520.0, y: 20.0 }}, {{ x: -20.0, y: 20.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band', points: [
      {{ x: 0.0, y: -5.0 }}, {{ x: 500.0, y: -5.0 }},
      {{ x: 500.0, y: 5.0 }}, {{ x: 0.0, y: 5.0 }},
    ] }},
  ],
  facilities: [
    {{ id: 'road', region: 'band', reference_path: 'guide',
      width_m: 10.0, nominal_direction: 'forward',
      access: {{ modes: [ 'passenger_car', 'bicycle' ] }}, lateral_use: '{lateral_use}',
      {lateral_policy}
      speed_policy: {{ limit_mps: null }} }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [
    {{
      id: 'passenger_car',
      body: {{ kind: 'box', length_m: {{ min: 4.5, max: 4.5 }},
        width_m: {{ min: 1.8, max: 1.8 }} }},
      motion: 'single_body_wheeled',
      tactics: [ {tactics} ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: {motor_speed_mps}, max: {motor_speed_mps} }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        {car_lateral_profile}
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
      {lateral}
    }},
    {{
      id: 'bicycle',
      body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }},
        radius_m: {{ min: 0.35, max: 0.35 }} }},
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: {narrow_speed_mps}, max: {narrow_speed_mps} }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
    }},
  ],
  clearance_bands: [
    {{ id: 'bicycle_close', threshold_m: 0.75, violation: true,
      applies_to_modes: [ 'bicycle' ] }},
    {{ id: 'bicycle_study', threshold_m: 1.5, violation: false,
      applies_to_modes: [ 'bicycle' ] }},
    {{ id: 'all_study', threshold_m: 3.0, violation: false }},
  ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'bicycle_inflow', mode: 'bicycle',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'car_inflow', mode: 'passenger_car',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}"#
    )
}

/// The four overtaking family counts of one run, counted from the event stream
/// so the recorder is checked against its own record set rather than against a
/// reimplementation of the writer.
#[derive(Debug, Default, PartialEq, Eq)]
struct OvertakingCounts {
    attempts: u64,
    commits: u64,
    completions: u64,
    aborts: u64,
}

/// One recorded close-pass run: the metrics the recorder captured, the
/// observations the tracker closed, and the overtaking counts.
struct Recording {
    metrics: RunMetrics,
    observations: Vec<OvertakeObservation>,
    overtaking: OvertakingCounts,
}

fn record_run(scenario: CompiledScenario, seed: u64, ticks: u64) -> Recording {
    let mut sim = Simulation::new(scenario, RunConfig::new(seed)).expect("simulation builds");
    let mut recorder = RunMetricsRecorder::new();
    let mut overtaking = OvertakingCounts::default();
    for _ in 0..ticks {
        let output = sim.step();
        for event in output.events() {
            let Event::Maneuver {
                kind,
                edge,
                from,
                to,
                ..
            } = event
            else {
                continue;
            };
            if *kind != TacticKind::Overtake {
                continue;
            }
            match (*edge, *from, *to) {
                (ManeuverEdge::Attempted, ..) => overtaking.attempts += 1,
                (ManeuverEdge::Committed, ..) => overtaking.commits += 1,
                (ManeuverEdge::Completed, ManeuverState::Committed, ManeuverState::Returning) => {
                    overtaking.completions += 1;
                }
                (ManeuverEdge::Aborted, ..) => overtaking.aborts += 1,
                // The return's own completion is not the pass's.
                (ManeuverEdge::Completed, ..) => {}
            }
        }
        recorder.record(&output);
    }
    // The run ends, so the still-open close-pass intervals close before the
    // capture reads the tracker: this hand-driven loop mirrors the run loop's
    // close boundary, and every reference it produces holds the run-end
    // closures the loop holds.
    sim.close_open_close_passes();
    let metrics = recorder.finish(&sim);
    Recording {
        metrics,
        observations: sim.close_pass_tracker().overtakes().to_vec(),
        overtaking,
    }
}

/// A reported countable family carries its value and the reported status.
fn assert_count(actual: &MetricValue, expected: u64) {
    assert_eq!(actual.status, MetricStatus::Reported);
    assert_eq!(actual.value, Some(expected as f64));
}

/// The band-duration series of one bucket, as `(band id, seconds)` in
/// declaration order.
fn band_durations(durations: &BTreeMap<u32, MetricValue>) -> Vec<(u32, Option<f64>)> {
    durations
        .iter()
        .map(|(band, value)| (*band, value.value))
        .collect()
}

/// The closed observations accumulate into every close-pass family, and each
/// bucket's applicability is explicit rather than a false `0`.
#[test]
fn close_pass_families_accumulate_from_the_closed_observations() {
    let recording = record_run(
        passing_scenario(9.0, 4.0, true, "shared"),
        7,
        CLOSE_PASS_TICKS,
    );
    let observations = &recording.observations;
    assert!(
        !observations.is_empty(),
        "the faster motor must overtake the slower narrow user at least once"
    );
    let pass = &recording.metrics.close_pass.run;

    // The overtaking families are the counted `Maneuver` edges of the run.
    assert_count(&pass.overtake_attempts, recording.overtaking.attempts);
    assert_count(&pass.overtake_commits, recording.overtaking.commits);
    assert_count(&pass.overtake_completions, recording.overtaking.completions);
    assert_count(&pass.overtake_aborts, recording.overtaking.aborts);
    assert!(
        recording.overtaking.completions > 0,
        "the run completes an overtake: {:?}",
        recording.overtaking
    );

    // The close-pass count is the tracker's own closed-observation slice, and
    // the violations are the observations that recorded a violating band.
    assert_count(&pass.close_passes, observations.len() as u64);
    assert_count(
        &pass.close_pass_violations,
        observations
            .iter()
            .filter(|observation| !observation.violating_bands.is_empty())
            .count() as u64,
    );

    // The minimum is the least observed clearance, with the time and relative
    // speed the observation recorded at it.
    let least = observations
        .iter()
        .map(|observation| observation.min_clearance_m)
        .fold(f64::INFINITY, f64::min);
    let observed = observations
        .iter()
        .find(|observation| observation.min_clearance_m == least)
        .expect("the least clearance belongs to an observation");
    let minimum = &pass.close_pass_minimum_clearance_m;
    assert_eq!(minimum.status, MetricStatus::Reported);
    assert_eq!(minimum.value, Some(least));
    assert_eq!(minimum.agent, Some(observed.agent.get()));
    assert_eq!(minimum.other, Some(observed.partner.get()));
    assert_eq!(minimum.mode_pair.as_deref(), Some("vehicle_vehicle"));
    assert_eq!(minimum.time_s, Some(observed.min_clearance_time_s));
    assert_eq!(
        minimum.relative_speed_mps,
        Some(observed.relative_speed_mps)
    );

    // One duration series entry per declared band, in declaration order, each
    // the sum of the observations that participated in it.
    assert_eq!(
        band_durations(&pass.clearance_band_durations_s)
            .iter()
            .map(|(band, _)| *band)
            .collect::<Vec<_>>(),
        vec![0, 1, 2],
        "the series is every declared band in declaration order"
    );
    for (band, value) in band_durations(&pass.clearance_band_durations_s) {
        let expected: f64 = observations
            .iter()
            .filter_map(|observation| {
                observation
                    .bands
                    .iter()
                    .find(|observed| observed.band.get() == band)
            })
            .map(|observed| observed.duration_s)
            .sum();
        assert_eq!(
            value,
            Some(expected),
            "band {band} sums its participating observations"
        );
        assert_eq!(
            pass.clearance_band_durations_s[&band].status,
            MetricStatus::Reported
        );
    }
    assert!(
        pass.clearance_band_durations_s[&2].value.expect("a value") > 0.0,
        "the every-pair band is crossed by a real pass"
    );

    // The mode-pair slice is the vehicle pair the fixture produces, and a pair
    // that could host a pass but recorded none is not_observed rather than a
    // reported zero; a class no template declares the tactic for is
    // not_applicable.
    let vehicle_pair = &recording.metrics.close_pass.by_mode_pair["vehicle_vehicle"];
    assert_count(&vehicle_pair.close_passes, observations.len() as u64);
    assert_eq!(
        vehicle_pair.close_pass_minimum_clearance_m.value,
        Some(least)
    );
    let cross_pair = &recording.metrics.close_pass.by_mode_pair["vehicle_pedestrian"];
    assert_eq!(
        cross_pair.close_pass_minimum_clearance_m.status,
        MetricStatus::NotObserved
    );
    assert_eq!(cross_pair.close_pass_minimum_clearance_m.value, None);
    let pedestrian_pair = &recording.metrics.close_pass.by_mode_pair["pedestrian_pedestrian"];
    assert_eq!(
        pedestrian_pair.close_pass_minimum_clearance_m.status,
        MetricStatus::NotApplicable
    );
    assert!(
        pedestrian_pair
            .clearance_band_durations_s
            .values()
            .all(|band| band.status == MetricStatus::NotApplicable && band.value.is_none()),
        "a mode pair that cannot host a pass reports no clearance value"
    );

    // The facility slice is the shared road both bodies ride, and the movement
    // slice is the pairwise key of the one movement the fixture declares.
    let facility = &recording.metrics.close_pass.by_facility["facility:road"];
    assert_count(
        &facility.close_passes,
        observations
            .iter()
            .filter(|observation| observation.facility.is_some())
            .count() as u64,
    );
    let movement = &recording.metrics.close_pass.by_movement["movement:through|movement:through"];
    assert_count(&movement.close_passes, observations.len() as u64);
    assert_eq!(
        recording.metrics.close_pass.by_movement.len(),
        1,
        "one movement pair is declared, so one movement bucket exists"
    );
}

/// A run that stops on the last tick of an overtaking interval reports that pass
/// with the evidence the completed interval carries: the run loop closes the
/// still-open interval at run end before the metric capture reads the tracker,
/// so a pass in progress at termination is counted rather than dropped.
#[test]
fn a_pass_still_open_at_the_final_tick_is_reported_with_its_evidence() {
    let seed = 7;
    let scenario = || passing_scenario(9.0, 4.0, true, "shared");

    // The full-horizon run names an interval that stayed alongside for several
    // ticks, so its last observed tick is a run end that leaves the same
    // interval open.
    let full = record_run(scenario(), seed, CLOSE_PASS_TICKS);
    let completed = full
        .observations
        .iter()
        .find(|observation| observation.end_tick > observation.start_tick)
        .expect("the fixture records an overtaking interval spanning several ticks");
    let final_tick = completed.end_tick;

    // The hand-driven reference closes the open interval explicitly, so it names
    // the observation the loop's own capture must hold.
    let reference = record_run(scenario(), seed, final_tick);
    let closed = reference
        .observations
        .iter()
        .find(|observation| observation.start_tick == completed.start_tick)
        .expect("the interval still alongside at the final tick closed at run end");
    assert_eq!(
        closed, completed,
        "the closed-at-run-end observation carries the completed interval's \
         minimum, time, relative speed, and band evidence"
    );
    assert_eq!(
        reference
            .observations
            .iter()
            .filter(|observation| observation.start_tick == completed.start_tick)
            .count(),
        1,
        "the interval yields one record for its pair"
    );

    // The run loop performs that same closure: its capture matches the reference
    // count for count, including the still-open interval.
    let sampling = SamplingPolicy::default();
    let (_, _, _, metrics) = canonical_run_captured(
        scenario(),
        RunConfig::new(seed),
        final_tick,
        &sampling.trajectories,
    )
    .expect("run completes");
    assert_count(
        &metrics.close_pass.run.close_passes,
        reference.observations.len() as u64,
    );
    assert_eq!(
        metrics, reference.metrics,
        "the run loop's capture is the explicitly closed reference, so the \
         still-open interval reaches every close-pass family"
    );
}

/// Nominal travel is not counted: an abreast convoy at one speed is no overtake,
/// so the countable families report a true zero and the value families report no
/// observation rather than a false `0`.
#[test]
fn nominal_travel_is_not_counted() {
    let recording = record_run(passing_scenario(4.0, 4.0, true, "shared"), 7, NO_PASS_TICKS);
    assert!(
        recording.observations.is_empty(),
        "an abreast convoy is not an overtake: {:?}",
        recording.observations
    );
    assert_eq!(recording.overtaking.completions, 0);

    let pass = &recording.metrics.close_pass.run;
    assert_count(&pass.close_passes, 0);
    assert_count(&pass.close_pass_violations, 0);
    assert_count(&pass.overtake_attempts, recording.overtaking.attempts);
    assert_eq!(
        pass.close_pass_minimum_clearance_m.status,
        MetricStatus::NotObserved
    );
    assert_eq!(pass.close_pass_minimum_clearance_m.value, None);
    assert!(
        pass.clearance_band_durations_s
            .values()
            .all(|band| band.status == MetricStatus::NotObserved && band.value.is_none()),
        "no observation is not a band duration of zero"
    );
}

/// An inapplicable mode and a facility that offers no lateral freedom report no
/// clearance value at all, while a bucket that could host a pass reports its
/// no-observation status instead.
#[test]
fn an_inapplicable_mode_reports_no_clearance_value() {
    // No mode declares the `overtake` or `pass` tactic, so no mode pair can host
    // a pass; the shared facility still could, and only recorded none.
    let recording = record_run(
        passing_scenario(9.0, 4.0, false, "shared"),
        7,
        NO_PASS_TICKS,
    );
    assert!(recording.observations.is_empty());
    let pass = &recording.metrics.close_pass.run;
    assert_eq!(
        pass.close_pass_minimum_clearance_m.status,
        MetricStatus::NotApplicable
    );
    assert_eq!(pass.close_pass_minimum_clearance_m.value, None);
    assert!(
        pass.clearance_band_durations_s
            .values()
            .all(|band| band.status == MetricStatus::NotApplicable && band.value.is_none()),
        "an inapplicable run reports no band duration"
    );
    for values in recording.metrics.close_pass.by_mode_pair.values() {
        assert_eq!(
            values.close_pass_minimum_clearance_m.status,
            MetricStatus::NotApplicable
        );
        assert_eq!(values.close_pass_minimum_clearance_m.value, None);
    }
    // The shared facility could host a pass, so it reports the no-observation
    // status instead of the inapplicable one.
    assert_eq!(
        recording.metrics.close_pass.by_facility["facility:road"]
            .close_pass_minimum_clearance_m
            .status,
        MetricStatus::NotObserved
    );
    assert_count(&pass.close_passes, 0);
}

/// Nominal travel on a centered facility is not counted: the facility offers no
/// lateral freedom, so its bucket is not applicable and the pair that could host
/// a pass on a shared facility still reports no observation here.
#[test]
fn a_centered_facility_reports_no_clearance_value() {
    let recording = record_run(
        passing_scenario(9.0, 4.0, true, "centered"),
        7,
        NO_PASS_TICKS,
    );
    assert!(
        recording.observations.is_empty(),
        "a centered facility admits no pass: {:?}",
        recording.observations
    );
    let facility = &recording.metrics.close_pass.by_facility["facility:road"];
    assert_count(&facility.close_passes, 0);
    assert_eq!(
        facility.close_pass_minimum_clearance_m.status,
        MetricStatus::NotApplicable
    );
    assert_eq!(facility.close_pass_minimum_clearance_m.value, None);
    assert!(
        facility
            .clearance_band_durations_s
            .values()
            .all(|band| band.status == MetricStatus::NotApplicable && band.value.is_none()),
        "a centered facility reports no band duration"
    );
    // The mode pair itself declares the overtaking tactic, so it is a bucket
    // that could host a pass and recorded none.
    assert_eq!(
        recording.metrics.close_pass.by_mode_pair["vehicle_vehicle"]
            .close_pass_minimum_clearance_m
            .status,
        MetricStatus::NotObserved
    );
}

/// The run-directory artifact serializes the close-pass families and their
/// dimensions: `metrics.json` carries the block the recorder captured, it parses
/// back to the same families, and the run it describes is the run the directory
/// replays.
#[test]
fn the_run_artifact_serializes_and_round_trips_the_close_pass_families() {
    let scratch = Scratch::new("close-pass-artifact");
    let seed = 7;
    // The fixture is written to a file so the manifest records a scenario source
    // `replay` can re-load and hash, exactly as `run` records a real one.
    let source_path = scratch.path("close_pass_metrics.json5");
    std::fs::write(
        &source_path,
        passing_scenario_source(9.0, 4.0, true, "shared"),
    )
    .expect("the fixture source is written");
    let (scenario, provenance) = load_scenario_provenance(&source_path).expect("the fixture loads");
    let sampling = SamplingPolicy::default();
    let (trace, summary, trajectories, metrics) = canonical_run_captured(
        scenario,
        RunConfig::new(seed),
        CLOSE_PASS_TICKS,
        &sampling.trajectories,
    )
    .expect("run completes");
    assert_eq!(
        metrics.close_pass.run.close_pass_minimum_clearance_m.status,
        MetricStatus::Reported,
        "the fixture must close a pass observation, or this proves nothing"
    );

    let run_dir = scratch.path("run");
    write_run_directory(
        &run_dir,
        RunDirectoryRequest {
            scenario: &provenance,
            seed,
            step_s: RunConfig::new(seed).step().as_secs(),
            sampling,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )
    .expect("run directory is written");

    // The artifact carries the block, and the dimensions metric definition v3
    // fixes are all serialized: every mode pair, the pairwise movement bucket,
    // the shared facility the pass happened on, and one entry per declared band.
    let json = std::fs::read_to_string(run_dir.join(METRICS_FILE)).expect("metrics is written");
    assert!(json.contains("\"close_pass\""));
    let artifact = read_artifact(&run_dir);
    assert_eq!(
        artifact.close_pass.by_mode_pair.keys().collect::<Vec<_>>(),
        vec![
            "pedestrian_pedestrian",
            "vehicle_pedestrian",
            "vehicle_vehicle"
        ]
    );
    assert!(
        artifact
            .close_pass
            .by_movement
            .contains_key("movement:through|movement:through"),
        "the pairwise movement bucket is serialized"
    );
    assert!(
        artifact
            .close_pass
            .by_facility
            .contains_key("facility:road"),
        "the facility the pass happened on is serialized"
    );
    assert_eq!(
        band_durations(&artifact.close_pass.run.clearance_band_durations_s)
            .iter()
            .map(|(band, _)| *band)
            .collect::<Vec<_>>(),
        vec![0, 1, 2],
        "the band series is serialized in declaration order"
    );

    // The block survives the artifact: it is the capture the recorder made, and
    // writing the parsed artifact again reproduces the recorded bytes exactly.
    assert_eq!(artifact.close_pass, metrics.close_pass);
    let mut reserialized =
        serde_json::to_string_pretty(&artifact).expect("the artifact serializes");
    reserialized.push('\n');
    assert_eq!(reserialized, json);

    // The directory replays: the recorded scenario source and the recorded
    // parameters reproduce the recorded event stream, so the artifact describes
    // a run the directory holds the evidence for.
    let replayed = replay_run_directory(&run_dir, true).expect("the recorded run reproduces");
    assert_eq!(replayed.bytes(), trace.bytes());
}

// ---------------------------------------------------------------------------
// The wrong-way families (TAS-124)
// ---------------------------------------------------------------------------

/// Steps a single-rider wrong-way run keeps recording after its entry request,
/// long enough for the traversal on `a` to close and its successor on `left` to
/// still be open at the run end.
const TICKS_AFTER_ENTRY: u32 = 200;

/// The wrong-way fixture: a forward-nominal facility `a` whose reverse
/// traversal is connected through `a_back_to_left` to the reverse continuance
/// `left`, so a reverse-capable rider that requests the entry travels `a`
/// against its rule direction and completes its route.
///
/// `policy` authors the `maneuver_policy.wrong_way` block, `permission` authors
/// the `permit` statement that makes the opposing traversal legal, `capable`
/// gives the mode the `reverse_direction` tactic, and `direction` is `a`'s
/// authored nominal direction, so a test can make the run inapplicable or the
/// facility `either`.
fn opposing_scenario_source(
    rate_per_hour: f64,
    policy: bool,
    permission: bool,
    capable: bool,
    direction: &str,
) -> String {
    let tactics = match capable {
        true => "'follow', 'stop', 'yield', 'reverse_direction'",
        false => "'follow', 'stop', 'yield'",
    };
    let wrong_way = match policy {
        true => {
            "  maneuver_policy: {\n    wrong_way: { min_time_saving_s: 0.0,\n      max_opposing_density_per_km: 100.0, urgency: 1.0 },\n  },\n"
        }
        false => "",
    };
    let permissions = match permission {
        true => {
            "  permissions: [ { id: 'contraflow_a', kind: 'nominal_direction', holder: 'rider', target: 'a', effect: 'permit' } ],\n"
        }
        false => "",
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'wrong_way_metrics',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 100.0, y: 0.0 }} ] }},
    {{ id: 'guide_left', points: [ {{ x: -100.0, y: 0.0 }}, {{ x: 0.0, y: 0.0 }} ] }},
  ],
  portals: [
    {{ id: 'a_entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'a_exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -110.0, y: -10.0 }}, {{ x: 110.0, y: -10.0 }},
      {{ x: 110.0, y: 10.0 }}, {{ x: -110.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 100.0, y: -1.5 }},
      {{ x: 100.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_left', points: [
      {{ x: -100.0, y: -1.5 }}, {{ x: 0.0, y: -1.5 }},
      {{ x: 0.0, y: 1.5 }}, {{ x: -100.0, y: 1.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: '{direction}',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'left', region: 'band_left', reference_path: 'guide_left',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_connectors: [
    {{ id: 'left_into_a',
      from: {{ facility: 'left', direction: 'forward' }},
      to: {{ facility: 'a', direction: 'forward' }} }},
    {{ id: 'a_back_to_left',
      from: {{ facility: 'a', direction: 'reverse' }},
      to: {{ facility: 'left', direction: 'reverse' }} }},
  ],
{permissions}  movements: [
    {{ id: 'through', from: 'a_entry', to: 'a_exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [
    {{
      id: 'rider',
      body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }},
        radius_m: {{ min: 0.35, max: 0.35 }} }},
      motion: 'single_body_wheeled',
      tactics: [ {tactics} ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: 6.0, max: 6.0 }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 0.0, max: 0.0 }},
      }},
    }},
  ],
{wrong_way}  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'a_entry',
        rate_per_hour: {rate_per_hour:?},
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

fn opposing_scenario(
    rate_per_hour: f64,
    policy: bool,
    permission: bool,
    capable: bool,
    direction: &str,
) -> CompiledScenario {
    let source = parse_scenario_source_v2(&opposing_scenario_source(
        rate_per_hour,
        policy,
        permission,
        capable,
        direction,
    ))
    .expect("the document is version 2");
    CompiledScenario::compile_v2(source).expect("the scenario compiles")
}

/// One live body on a compiled guide path: its stable id, arc-length position,
/// and body length, the fields a bumper gap needs.
#[derive(Debug, Clone, Copy)]
struct BodyPlacement {
    id: AgentId,
    s_m: f64,
    body_length_m: f64,
}

/// Every live body on `path`, ascending by arc length.
fn live_bodies(sim: &Simulation, path: usize) -> Vec<BodyPlacement> {
    let mut bodies: Vec<BodyPlacement> = sim
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            (motion.path.index() == path).then_some(BodyPlacement {
                id: sample.id,
                s_m: motion.path_distance_m,
                body_length_m: motion.body_length_m,
            })
        })
        .collect();
    bodies.sort_by(|first, second| first.s_m.total_cmp(&second.s_m));
    bodies
}

/// The bumper-to-bumper gap between two bodies on one path, in metres.
fn bumper_gap_m(first: BodyPlacement, second: BodyPlacement) -> f64 {
    (first.s_m - second.s_m).abs() - (first.body_length_m + second.body_length_m) * 0.5
}

/// Step until exactly two bodies ride `path`, the leading one lies inside
/// `lead_window`, and its bumper gap to the other lies inside `gap_window`;
/// return the leading body and the body it turns into.
///
/// The pair is deterministic for the fixture's fixed seed, and the helper
/// mirrors the one the sim seam is tested with.
fn leading_pair_on(
    sim: &mut Simulation,
    path: usize,
    lead_window: std::ops::RangeInclusive<f64>,
    gap_window: std::ops::RangeInclusive<f64>,
) -> (BodyPlacement, BodyPlacement) {
    for _ in 0..8000 {
        sim.step();
        let bodies = live_bodies(sim, path);
        if let [occupancy, lead] = bodies[..] {
            let gap_m = bumper_gap_m(lead, occupancy);
            if lead_window.contains(&lead.s_m) && gap_window.contains(&gap_m) {
                return (lead, occupancy);
            }
        }
    }
    panic!(
        "two riders must ride path {path} with the leading one in {lead_window:?} \
         and a bumper gap in {gap_window:?}"
    );
}

/// The agents the snapshot observes on an opposing traversal.
fn opposing_bodies(sim: &Simulation) -> Vec<AgentId> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter(|sample| {
            sample
                .motion
                .as_ref()
                .and_then(|motion| motion.route_state)
                .is_some_and(|route| route.opposing_direction.is_some())
        })
        .map(|sample| sample.id)
        .collect()
}

/// Whether a test lets the hand-driven run continue or stop after the step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drive {
    Continue,
    Stop,
}

/// One hand-driven wrong-way run: the metrics the recorder captured, the
/// simulation that produced them, and every step's events.
struct WrongWayRun {
    metrics: RunMetrics,
    sim: Simulation,
    /// One entry per completed step, in step order, so a test can restate a
    /// family from the stream without the writer's own bookkeeping.
    steps: Vec<Vec<Event>>,
    final_tick: u64,
}

/// Drive a wrong-way run by hand, exactly as the run loop does: record every
/// step's events, observe the same completed tick, and close the run's open
/// intervals before the capture.
///
/// `drive` runs after each step with the live simulation, so a test asks for
/// the entry once its subject reaches the window it needs; the kernel evaluates
/// the request at the start of the next step, exactly as it evaluates one a
/// tactical leaf records.
fn drive_wrong_way_run(
    mut sim: Simulation,
    max_ticks: u64,
    mut drive: impl FnMut(&mut Simulation, u64) -> Drive,
) -> WrongWayRun {
    let mut recorder = RunMetricsRecorder::new();
    let mut steps = Vec::new();
    let mut final_tick = 0;
    for tick in 0..max_ticks {
        let output = sim.step();
        recorder.record(&output);
        steps.push(output.events().to_vec());
        final_tick = output.time().tick();
        recorder.observe(&sim);
        if drive(&mut sim, tick) == Drive::Stop {
            break;
        }
    }
    sim.close_open_opposing_traversals();
    let metrics = recorder.finish(&sim);
    WrongWayRun {
        metrics,
        sim,
        steps,
        final_tick,
    }
}

/// The rule key one interval is reported under: its perceived rule's stable
/// label, or `none` when no statement binds it.
fn rule_bucket_key(rule: Option<PermissionEffect>) -> String {
    rule.map_or_else(|| "none".to_owned(), |rule| rule.label().to_owned())
}

/// The counted intervals one dimension's buckets report.
fn bucket_intervals(slices: &BTreeMap<String, WrongWayValues>) -> u64 {
    slices
        .values()
        .map(|values| values.wrong_way_intervals.value.unwrap_or(0.0) as u64)
        .sum()
}

/// Assert every bucket of one dimension reports exactly the intervals the
/// tracker recorded under that key, and that the buckets account for all of
/// them.
fn assert_bucket_counts(
    slices: &BTreeMap<String, WrongWayValues>,
    expected: &BTreeMap<String, u64>,
) {
    for (key, values) in slices {
        assert_count(
            &values.wrong_way_intervals,
            expected.get(key).copied().unwrap_or(0),
        );
    }
    assert_eq!(bucket_intervals(slices), expected.values().sum::<u64>());
}

/// The intervals the tracker closed, grouped by each dimension's own key,
/// restated from the observations rather than from the writer's buckets.
fn expected_bucket_counts(
    sim: &Simulation,
    intervals: &[OpposingTraversalObservation],
    key: impl Fn(&Simulation, &OpposingTraversalObservation) -> String,
) -> BTreeMap<String, u64> {
    let mut expected: BTreeMap<String, u64> = BTreeMap::new();
    for interval in intervals {
        *expected.entry(key(sim, interval)).or_default() += 1;
    }
    expected
}

/// Every conflict the raw stream records, restated from its own
/// [`Event::OpposingTraversal`] boundaries: a contacting collision or entering
/// near miss whose pair has at least one participant on an opposing traversal
/// on that step.
fn streamed_wrong_way_conflicts(steps: &[Vec<Event>]) -> Vec<(u32, u32)> {
    let mut open: BTreeSet<u32> = BTreeSet::new();
    let mut conflicts = Vec::new();
    for events in steps {
        for event in events {
            match event {
                Event::OpposingTraversal {
                    agent,
                    entering: true,
                    ..
                } => {
                    open.insert(agent.get());
                }
                Event::OpposingTraversal {
                    agent,
                    entering: false,
                    ..
                } => {
                    open.remove(&agent.get());
                }
                _ => {}
            }
        }
        for event in events {
            let (Event::Collision {
                agent,
                other,
                contacting: true,
                ..
            }
            | Event::NearMiss {
                agent,
                other,
                entering: true,
                ..
            }) = event
            else {
                continue;
            };
            if open.contains(&agent.get()) || open.contains(&other.get()) {
                conflicts.push((agent.get(), other.get()));
            }
        }
    }
    conflicts
}

/// Every family the wrong-way block reports is a count or an explicit status,
/// and each disaggregation dimension buckets the tracker's own intervals.
#[test]
fn wrong_way_families_accumulate_from_the_opposing_intervals() {
    let mut requested = false;
    let mut since_request = 0u32;
    let run = drive_wrong_way_run(
        Simulation::new(
            opposing_scenario(72.0, true, false, true, "forward"),
            RunConfig::new(0),
        )
        .expect("the simulation builds"),
        4000,
        |sim, _| {
            if !requested {
                if let [rider] = live_bodies(sim, 0)[..]
                    && (20.0..=30.0).contains(&rider.s_m)
                {
                    assert!(
                        sim.request_wrong_way_entry(rider.id),
                        "a reverse-capable rider on a two-way facility can request the entry"
                    );
                    requested = true;
                }
                return Drive::Continue;
            }
            since_request += 1;
            match since_request >= TICKS_AFTER_ENTRY {
                true => Drive::Stop,
                false => Drive::Continue,
            }
        },
    );
    assert!(requested, "the fixture's rider reaches the turn window");
    let intervals: Vec<OpposingTraversalObservation> =
        run.sim.opposing_traversal_tracker().intervals().to_vec();
    assert!(
        !intervals.is_empty(),
        "the turned rider records at least one opposing-traversal interval"
    );
    let total = intervals.len() as u64;

    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, total);

    // The duration is the tracker's own closed intervals, so the two sources
    // agree exactly rather than to a tolerance.
    let duration: f64 = intervals
        .iter()
        .map(OpposingTraversalObservation::duration_s)
        .sum();
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.status,
        MetricStatus::Reported
    );
    assert_eq!(wrong_way.run.wrong_way_duration_s.value, Some(duration));

    // The distance is the arc length travelled against the rule direction over
    // whole steps, so it is positive and cannot exceed the fixture's 6 m/s.
    assert_eq!(
        wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::Reported
    );
    let distance = wrong_way
        .run
        .wrong_way_distance_m
        .value
        .expect("the recorded interval carries a distance");
    assert!(
        distance > 0.0,
        "the turned rider travels its opposing traversal: {distance} m"
    );
    assert!(
        distance <= 6.0 * duration,
        "no whole step exceeds the fixture's 6 m/s: {distance} m over {duration} s"
    );

    // Each dimension buckets exactly the tracker's intervals: the mode pair of
    // each traversing agent, the traversed facility, the single subject's own
    // participant key, and the interval's perceived rule.
    let by_mode_pair = expected_bucket_counts(&run.sim, &intervals, |sim, interval| {
        let mode = sim
            .agent_mode(interval.agent)
            .expect("the traversing agent is live");
        ModePair::of(mode, mode).label().to_owned()
    });
    let by_facility = expected_bucket_counts(&run.sim, &intervals, |sim, interval| {
        let name = sim
            .scenario()
            .facility(interval.facility)
            .expect("the traversed facility is compiled")
            .name();
        format!("facility:{name}")
    });
    let by_pair = expected_bucket_counts(&run.sim, &intervals, |_, interval| {
        format!("agent:{}", interval.agent.get())
    });
    let by_rule = expected_bucket_counts(&run.sim, &intervals, |_, interval| {
        rule_bucket_key(interval.perceived_rule)
    });
    assert_bucket_counts(&wrong_way.by_mode_pair, &by_mode_pair);
    assert_bucket_counts(&wrong_way.by_facility, &by_facility);
    assert_bucket_counts(&wrong_way.by_pair, &by_pair);
    assert_bucket_counts(&wrong_way.by_rule, &by_rule);
    assert_eq!(
        wrong_way.by_facility.keys().collect::<Vec<_>>(),
        vec!["facility:a", "facility:left"],
        "every compiled facility is a bucket"
    );
    assert!(
        bucket_intervals(&wrong_way.by_movement) <= total,
        "a contribution names at most one movement bucket"
    );

    // One rider at this rate meets no one, so the value families are explicit:
    // the co-present exposure is no observation, and the countable families are
    // observed zeros.
    assert_eq!(
        wrong_way.run.wrong_way_exposure_agent_s.status,
        MetricStatus::NotObserved
    );
    assert_count(&wrong_way.run.wrong_way_encounters, 0);
    assert_count(&wrong_way.run.wrong_way_conflicts, 0);
}

/// A legal (permitted) opposing traversal is reported under its rule rather
/// than under `none`.
#[test]
fn a_permitted_opposing_traversal_is_reported_under_its_rule() {
    let mut requested = false;
    let mut since_request = 0u32;
    let run = drive_wrong_way_run(
        Simulation::new(
            opposing_scenario(72.0, true, true, true, "forward"),
            RunConfig::new(0),
        )
        .expect("the simulation builds"),
        4000,
        |sim, _| {
            if !requested {
                if let [rider] = live_bodies(sim, 0)[..]
                    && (5.0..=20.0).contains(&rider.s_m)
                {
                    assert!(sim.request_wrong_way_entry(rider.id));
                    requested = true;
                }
                return Drive::Continue;
            }
            since_request += 1;
            match since_request >= TICKS_AFTER_ENTRY {
                true => Drive::Stop,
                false => Drive::Continue,
            }
        },
    );
    let intervals: Vec<OpposingTraversalObservation> =
        run.sim.opposing_traversal_tracker().intervals().to_vec();
    let permitted = intervals
        .iter()
        .find(|interval| interval.perceived_rule.is_some())
        .expect("the permitted fixture records the rule on its interval");
    assert_eq!(permitted.perceived_rule, Some(PermissionEffect::Permit));
    assert!(!permitted.violating, "a permitted traversal is legal");

    let wrong_way = &run.metrics.wrong_way;
    let by_rule = expected_bucket_counts(&run.sim, &intervals, |_, interval| {
        rule_bucket_key(interval.perceived_rule)
    });
    assert!(
        by_rule.contains_key("permit"),
        "the fixture records a permitted interval"
    );
    assert_bucket_counts(&wrong_way.by_rule, &by_rule);
}

/// A scenario that cannot host an opposing traversal reports the explicit
/// inapplicable status rather than a zero: no `reverse_direction` capability
/// makes the whole run inapplicable, and an `either` facility makes its own
/// bucket inapplicable while the run reports no observation.
#[test]
fn an_inapplicable_wrong_way_scenario_reports_no_family() {
    // The mode carries no `reverse_direction` and the scenario authors no
    // wrong-way policy, so no entry can be requested and the value families are
    // inapplicable.
    let mut attempted = false;
    let run = drive_wrong_way_run(
        Simulation::new(
            opposing_scenario(72.0, false, false, false, "forward"),
            RunConfig::new(0),
        )
        .expect("the simulation builds"),
        4000,
        |sim, _| {
            if !attempted && let Some(rider) = live_bodies(sim, 0).first() {
                assert!(
                    !sim.request_wrong_way_entry(rider.id),
                    "no policy and no capability means no entry"
                );
                attempted = true;
                return Drive::Stop;
            }
            Drive::Continue
        },
    );
    assert!(attempted, "the fixture places a rider");
    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 0);
    for value in [
        &wrong_way.run.wrong_way_distance_m,
        &wrong_way.run.wrong_way_duration_s,
        &wrong_way.run.wrong_way_exposure_agent_s,
    ] {
        assert_eq!(value.status, MetricStatus::NotApplicable);
        assert_eq!(value.value, None);
    }
    for values in wrong_way.by_mode_pair.values() {
        assert_eq!(
            values.wrong_way_distance_m.status,
            MetricStatus::NotApplicable
        );
    }

    // An `either` facility has no rule direction to oppose, so the rider's
    // turn is recorded as no opposing traversal at all: the facility bucket is
    // inapplicable while the policy-authored run could still host one and
    // reports no observation.
    let mut requested = false;
    let mut since_request = 0u32;
    let run = drive_wrong_way_run(
        Simulation::new(
            opposing_scenario(72.0, true, false, true, "either"),
            RunConfig::new(0),
        )
        .expect("the simulation builds"),
        4000,
        |sim, _| {
            if !requested {
                if let Some(rider) = live_bodies(sim, 0).first() {
                    assert!(
                        sim.request_wrong_way_entry(rider.id),
                        "the connected opposing traversal is physically possible"
                    );
                    requested = true;
                }
                return Drive::Continue;
            }
            since_request += 1;
            match since_request >= 60 {
                true => Drive::Stop,
                false => Drive::Continue,
            }
        },
    );
    assert!(requested, "the fixture places a rider");
    assert!(
        run.sim.opposing_traversal_tracker().intervals().is_empty(),
        "an `either` facility has no rule direction to oppose"
    );
    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 0);
    assert_eq!(
        wrong_way.by_facility["facility:a"]
            .wrong_way_distance_m
            .status,
        MetricStatus::NotApplicable
    );
    assert_eq!(
        wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::NotObserved,
        "the policy could host a traversal, so the run reports no observation"
    );
}

/// A contacting collision and an entering near miss whose pair carries an
/// opposing participant are the conflict family, and the pairs that meet the
/// opposing rider are its encounters.
#[test]
fn wrong_way_conflicts_link_contacts_and_near_misses_to_the_traversal() {
    let mut sim = Simulation::new(
        opposing_scenario(720.0, true, false, true, "forward"),
        RunConfig::new(0),
    )
    .expect("the simulation builds");
    let (subject, occupancy) = leading_pair_on(&mut sim, 0, 30.0..=42.0, 8.0..=18.0);
    assert!(
        sim.request_wrong_way_entry(subject.id),
        "the entry is decided before any occupancy is read"
    );
    let run = drive_wrong_way_run(sim, 300, |_, _| Drive::Continue);

    let conflicts = streamed_wrong_way_conflicts(&run.steps);
    let contacts = run
        .steps
        .iter()
        .flatten()
        .filter(|event| {
            matches!(
                event,
                Event::Collision {
                    contacting: true,
                    ..
                }
            )
        })
        .count();
    let near_misses = run
        .steps
        .iter()
        .flatten()
        .filter(|event| matches!(event, Event::NearMiss { entering: true, .. }))
        .count();
    assert!(
        contacts >= 1,
        "the unavoidable encounter opens the collision scan's contact band"
    );
    assert!(
        near_misses >= 1,
        "the unavoidable encounter opens the collision scan's near-miss band"
    );
    assert!(
        !conflicts.is_empty(),
        "the encounter is linked to the opposing participant"
    );
    assert_count(
        &run.metrics.wrong_way.run.wrong_way_conflicts,
        conflicts.len() as u64,
    );

    // The opposing rider met the body it turned into, and the pair carries the
    // conflict the run attributed to it.
    let wrong_way = &run.metrics.wrong_way;
    let pair_key = format!(
        "agent:{}|agent:{}",
        subject.id.get().min(occupancy.id.get()),
        subject.id.get().max(occupancy.id.get())
    );
    assert!(
        wrong_way.by_pair.contains_key(&pair_key),
        "the encounter is keyed by the participant pair: {:?}",
        wrong_way.by_pair.keys().collect::<Vec<_>>()
    );
    assert!(
        wrong_way.run.wrong_way_encounters.value.expect("a count") >= 1.0,
        "the opposing rider's encounters include the body it met"
    );
    assert!(
        wrong_way.by_pair[&pair_key]
            .wrong_way_conflicts
            .value
            .expect("a count")
            >= 1.0
    );
}

/// The run ending is a close boundary: an interval still open on the final step
/// is closed by the capture and counted, and its close time is the run's final
/// simulation time.
#[test]
fn an_interval_open_at_the_run_end_is_counted() {
    let mut requested = false;
    let mut opposing_ticks = 0u32;
    let run = drive_wrong_way_run(
        Simulation::new(
            opposing_scenario(72.0, true, false, true, "forward"),
            RunConfig::new(0),
        )
        .expect("the simulation builds"),
        4000,
        |sim, _| {
            if !requested {
                if let [rider] = live_bodies(sim, 0)[..]
                    && (20.0..=30.0).contains(&rider.s_m)
                {
                    assert!(sim.request_wrong_way_entry(rider.id));
                    requested = true;
                }
                return Drive::Continue;
            }
            if !opposing_bodies(sim).is_empty() {
                opposing_ticks += 1;
                if opposing_ticks >= 40 {
                    return Drive::Stop;
                }
            }
            Drive::Continue
        },
    );
    assert!(requested, "the rider reaches the turn window");

    // The interval is still open at the final tick: the run-end closure is the
    // only producer of the tracker record, and the capture counts it.
    let intervals = run.sim.opposing_traversal_tracker().intervals();
    assert_eq!(
        intervals.len(),
        1,
        "the run ends inside the rider's opposing traversal"
    );
    assert_eq!(
        intervals[0].end_tick, run.final_tick,
        "the run-end closure's close time is the final simulation tick"
    );
    assert!(
        intervals[0].duration_s() > 0.0,
        "the interval spans whole steps"
    );
    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 1);
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.status,
        MetricStatus::Reported
    );
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.value,
        Some(intervals[0].duration_s())
    );
    assert_eq!(
        wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::Reported
    );

    // The version decision of this slice: the families land under the existing
    // metric definition v3, so the definition declares no new revision.
    assert_eq!(METRIC_DEFINITION_VERSION, 3);
}

// ---------------------------------------------------------------------------
// The checked-in contextual wrong-way fixture (TAS-131)
// ---------------------------------------------------------------------------
//
// `scenarios/phase2/inc2/narrow_wrong_way_v2.json5` is Increment 2's
// `CC-OPPOSE` reference: four isolated corridors author the permitted,
// prohibited-but-connected, disconnected, and occupied opposing cases the
// contract's *Contextual wrong-way traversal* section fixes. This suite loads
// the checked-in file through the CLI's own scenario loader and drives one entry
// per case.
//
// The entry is recorded through `Simulation::request_wrong_way_entry`, the seam
// a tactical leaf supplies, exactly as the landed TAS-124 fixture's tests do: a
// checked-in scenario authors no request and the kernel owns the decision, so
// the test supplies the request and every input the decision reads — the
// compiled wrong-way policy, the compiled topology, the authored `permissions[]`
// — stays the fixture's. Every profile entry is authored constant (`min == max`)
// with zero compliance under full urgency, so every draw selects the opposing
// option and no case depends on a particular stream value.

/// The checked-in Increment 2 wrong-way fixture, relative to the repo root.
const WRONG_WAY_FIXTURE: &str = "scenarios/phase2/inc2/narrow_wrong_way_v2.json5";

/// Load the checked-in wrong-way fixture through the CLI's own loader, so the
/// suite reads the file the CLI reads rather than a re-typed copy.
fn wrong_way_fixture() -> CompiledScenario {
    load(WRONG_WAY_FIXTURE).0
}

/// The dense index of the fixture's guide path named `name`.
fn guide_path(scenario: &CompiledScenario, name: &str) -> usize {
    scenario
        .paths()
        .iter()
        .position(|path| path.name() == name)
        .unwrap_or_else(|| panic!("the fixture authors a guide path named '{name}'"))
}

/// The dense index of the fixture's facility named `name`.
fn fixture_facility(scenario: &CompiledScenario, name: &str) -> usize {
    scenario
        .facilities()
        .iter()
        .position(|facility| facility.name() == name)
        .unwrap_or_else(|| panic!("the fixture authors a facility named '{name}'"))
}

/// The compiled movement the fixture authors as `name`.
fn fixture_movement(scenario: &CompiledScenario, name: &str) -> MovementId {
    scenario
        .movements()
        .iter()
        .find(|movement| movement.name() == name)
        .unwrap_or_else(|| panic!("the fixture authors a movement named '{name}'"))
        .id()
}

/// Whether an agent is still live in the run.
fn live(sim: &Simulation, agent: AgentId) -> bool {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .any(|sample| sample.id == agent)
}

/// One interval boundary of one agent, as the test reads the run's records.
#[derive(Debug, Clone, PartialEq)]
struct WrongWayBoundary {
    tick: u64,
    entering: bool,
    facility: usize,
    movement: Option<usize>,
    direction: MovementDirection,
    nominal_direction: NominalDirection,
    perceived_rule: Option<PermissionEffect>,
    reason: WrongWayReason,
    violating: bool,
}

/// One agent's records across a hand-driven run: its interval boundaries, the
/// ticks of its facility handoffs, and the tick and reason of its despawn.
#[derive(Debug, Default)]
struct SubjectRecords {
    boundaries: Vec<WrongWayBoundary>,
    handoffs: Vec<u64>,
    despawned: Option<(u64, DespawnReason)>,
}

/// Read one agent's wrong-way boundaries, facility handoffs, and despawn from
/// the run's own records.
///
/// A step advances one tick and the run reports every step in order with the
/// tick the last one completed, so the step at index `i` completed tick
/// `final_tick - (steps.len() - 1 - i)`; the interval assertions check that
/// mapping against the tracker's own tick records rather than assuming it.
fn subject_records(run: &WrongWayRun, agent: AgentId) -> SubjectRecords {
    let mut records = SubjectRecords::default();
    for (index, events) in run.steps.iter().enumerate() {
        let tick = run.final_tick - (run.steps.len() - 1 - index) as u64;
        for event in events {
            match event {
                Event::OpposingTraversal {
                    agent: subject,
                    facility,
                    movement,
                    direction,
                    nominal_direction,
                    perceived_rule,
                    reason,
                    violating,
                    entering,
                } if *subject == agent => records.boundaries.push(WrongWayBoundary {
                    tick,
                    entering: *entering,
                    facility: facility.index(),
                    movement: movement.map(MovementId::index),
                    direction: *direction,
                    nominal_direction: *nominal_direction,
                    perceived_rule: *perceived_rule,
                    reason: *reason,
                    violating: *violating,
                }),
                Event::FacilityTransition { agent: subject, .. } if *subject == agent => {
                    records.handoffs.push(tick);
                }
                Event::Despawned {
                    agent: subject,
                    reason,
                    ..
                } if *subject == agent => records.despawned = Some((tick, *reason)),
                _ => {}
            }
        }
    }
    records
}

/// The tracker's closed intervals for one agent, as `(facility, start tick, end
/// tick)` tuples.
fn interval_ticks(run: &WrongWayRun, agent: AgentId) -> Vec<(usize, u64, u64)> {
    run.sim
        .opposing_traversal_tracker()
        .intervals()
        .iter()
        .filter(|interval| interval.agent == agent)
        .map(|interval| {
            (
                interval.facility.index(),
                interval.start_tick,
                interval.end_tick,
            )
        })
        .collect()
}

/// The total seconds the tracker's closed intervals cover.
fn interval_duration(run: &WrongWayRun) -> f64 {
    run.sim
        .opposing_traversal_tracker()
        .intervals()
        .iter()
        .map(OpposingTraversalObservation::duration_s)
        .sum()
}

/// Step until two adjacent bodies ride `path` with the leading one inside
/// `lead_window` and their bumper gap inside `gap_window`, returning the leading
/// body (the one the test turns) and the body it turns into.
///
/// The fixture's occupied corridor carries both its demand sources, so the pair
/// is the deterministic first adjacent pair the run places inside the windows.
fn adjacent_pair_on(
    sim: &mut Simulation,
    path: usize,
    lead_window: std::ops::RangeInclusive<f64>,
    gap_window: std::ops::RangeInclusive<f64>,
) -> (BodyPlacement, BodyPlacement) {
    for _ in 0..8000 {
        sim.step();
        let bodies = live_bodies(sim, path);
        for pair in bodies.windows(2) {
            let (occupancy, lead) = (pair[0], pair[1]);
            let gap_m = bumper_gap_m(lead, occupancy);
            if lead_window.contains(&lead.s_m) && gap_window.contains(&gap_m) {
                return (lead, occupancy);
            }
        }
    }
    panic!(
        "the fixture places an adjacent pair on path {path} with the leader in \
         {lead_window:?} and a bumper gap in {gap_window:?}"
    );
}

/// The fixture's occupied corridor driven through one entry: the pair the
/// leading rider turned into, the run's records, and the least values the
/// ordinary machinery bounded the encounter by.
struct OccupiedCorridorRun {
    run: WrongWayRun,
    subject: AgentId,
    occupancy: AgentId,
    boundaries: Vec<WrongWayBoundary>,
    /// Least bumper-to-bumper gap between the subject and the body it turned
    /// into, in metres, over the driven steps.
    min_gap_m: f64,
    /// Least clearance the ordinary collision scan reported for the pair, in
    /// metres.
    min_clearance_m: f64,
    /// Contacting collisions the scan recorded for the pair.
    contacts: u64,
    /// Entering near misses the scan recorded for the pair.
    near_misses: u64,
    /// Whether the subject ever left its corridor or ran a lateral maneuver,
    /// which is what a bypass of the occupied corridor would be.
    bypassed: bool,
}

/// Drive the fixture's occupied corridor: find an adjacent pair whose leading
/// body turns into its follower inside the fixture's stopping distance, record
/// the entry, and run long enough for the encounter and for the interval the
/// encounter leaves open at the run end.
fn drive_occupied_corridor() -> OccupiedCorridorRun {
    let scenario = wrong_way_fixture();
    let corridor = guide_path(&scenario, "guide_d");
    let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("the fixture builds");
    let (subject, occupancy) = adjacent_pair_on(&mut sim, corridor, 25.0..=48.0, 8.0..=18.0);
    assert!(
        sim.request_wrong_way_entry(subject.id),
        "the occupied corridor's leading rider can request the entry"
    );

    let mut min_gap_m = f64::INFINITY;
    let mut bypassed = false;
    let run = drive_wrong_way_run(sim, 300, |sim, _| {
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let placed = |agent: AgentId| {
            snapshot
                .agents()
                .iter()
                .find(|sample| sample.id == agent)
                .and_then(|sample| sample.motion.as_ref())
        };
        let (Some(subject_now), Some(occupancy_now)) = (placed(subject.id), placed(occupancy.id))
        else {
            return Drive::Continue;
        };
        min_gap_m = min_gap_m.min(bumper_gap_m(
            BodyPlacement {
                id: subject.id,
                s_m: subject_now.path_distance_m,
                body_length_m: subject_now.body_length_m,
            },
            BodyPlacement {
                id: occupancy.id,
                s_m: occupancy_now.path_distance_m,
                body_length_m: occupancy_now.body_length_m,
            },
        ));
        bypassed |= subject_now.path.index() != corridor
            || subject_now.route_state.map(|state| state.maneuver_state)
                != Some(ManeuverState::Following);
        Drive::Continue
    });

    let mut contacts = 0;
    let mut near_misses = 0;
    let mut min_clearance_m = f64::INFINITY;
    for events in &run.steps {
        for event in events {
            let clearance_m = match event {
                Event::Collision {
                    agent,
                    other,
                    contacting: true,
                    clearance_m,
                } if contacts_pair(*agent, *other, subject.id, occupancy.id) => {
                    contacts += 1;
                    *clearance_m
                }
                Event::NearMiss {
                    agent,
                    other,
                    entering: true,
                    clearance_m,
                } if contacts_pair(*agent, *other, subject.id, occupancy.id) => {
                    near_misses += 1;
                    *clearance_m
                }
                _ => continue,
            };
            min_clearance_m = min_clearance_m.min(clearance_m);
        }
    }

    let boundaries = subject_records(&run, subject.id).boundaries;
    OccupiedCorridorRun {
        run,
        subject: subject.id,
        occupancy: occupancy.id,
        boundaries,
        min_gap_m,
        min_clearance_m,
        contacts,
        near_misses,
        bypassed,
    }
}

/// Whether an event's agent pair is the subject and the body it turned into.
fn contacts_pair(first: AgentId, second: AgentId, subject: AgentId, occupancy: AgentId) -> bool {
    (first == subject && second == occupancy) || (first == occupancy && second == subject)
}

/// The fixture's permitted corridor: the decision's opposing option is the
/// authored `permit` statement, so the interval is recorded under its rule and
/// never as a violation, its boundaries are the entry and the object boundary,
/// and the turned rider completes its route onto the connected continuance.
#[test]
fn the_checked_in_fixture_records_its_permitted_opposing_choice() {
    let scenario = wrong_way_fixture();
    let corridor = guide_path(&scenario, "guide_a");
    let facility = fixture_facility(&scenario, "a");
    let continuance_facility = fixture_facility(&scenario, "a_left");
    let movement = fixture_movement(&scenario, "through_a");

    let mut subject: Option<AgentId> = None;
    let run = drive_wrong_way_run(
        Simulation::new(scenario, RunConfig::new(0)).expect("the fixture builds"),
        6000,
        |sim, _| {
            if subject.is_none() {
                if let [body] = live_bodies(sim, corridor)[..]
                    && (5.0..=20.0).contains(&body.s_m)
                {
                    assert!(
                        sim.request_wrong_way_entry(body.id),
                        "the permitted corridor's rider can request the entry"
                    );
                    subject = Some(body.id);
                }
                return Drive::Continue;
            }
            match subject.is_some_and(|subject| !live(sim, subject)) {
                true => Drive::Stop,
                false => Drive::Continue,
            }
        },
    );
    let subject = subject.expect("the permitted corridor admits a lone rider");
    let records = subject_records(&run, subject);

    // Two intervals, each with an open and a close boundary: one per object the
    // turned rider traverses against its rule direction.
    assert_eq!(
        records.boundaries.len(),
        4,
        "the turned rider opens and closes one interval per object: {:?}",
        records.boundaries
    );
    let open = &records.boundaries[0];
    let close = &records.boundaries[1];
    let reopened = &records.boundaries[2];
    let closed_at_exit = &records.boundaries[3];
    let (despawned, despawn_reason) = records
        .despawned
        .expect("the turned rider completes its route and despawns");

    // The interval opens on the corridor's own reference, closes on the handoff
    // onto the continuance, and closes once more at the route exit.
    assert!(open.entering && !close.entering && reopened.entering && !closed_at_exit.entering);
    assert_eq!(open.facility, facility);
    assert_eq!(close.facility, facility, "the same object it opened on");
    assert_eq!(
        close.tick, records.handoffs[0],
        "the connector handoff is the close boundary"
    );
    assert_eq!(
        reopened.facility, continuance_facility,
        "the destination object opens its own interval"
    );
    assert_eq!(reopened.tick, close.tick, "the handoff is both boundaries");
    assert_eq!(closed_at_exit.facility, continuance_facility);
    assert_eq!(
        closed_at_exit.tick, despawned,
        "the route exit is the final close boundary"
    );
    assert_eq!(despawn_reason, DespawnReason::ExitedPath);
    assert!(open.tick < close.tick, "the interval spans whole steps");

    // The decision's own facts: the perceived rule, the reason code, the
    // affected movement, and the traversal's legality.
    assert_eq!(open.perceived_rule, Some(PermissionEffect::Permit));
    assert_eq!(open.reason, WrongWayReason::LegalPermission);
    assert!(
        !open.violating,
        "a permitted opposing traversal is never a violation"
    );
    assert_eq!(open.direction, MovementDirection::Reverse);
    assert_eq!(open.nominal_direction, NominalDirection::Forward);
    assert_eq!(
        open.movement,
        Some(movement.index()),
        "the affected movement is the one the rider entered on"
    );
    assert_eq!(
        close.reason, open.reason,
        "the record is the one it latched"
    );
    assert_eq!(close.violating, open.violating);

    // The tracker's intervals are the boundaries' own endpoints, which is where
    // the step-to-tick mapping is checked rather than assumed.
    assert_eq!(
        interval_ticks(&run, subject),
        vec![
            (facility, open.tick, close.tick),
            (continuance_facility, reopened.tick, closed_at_exit.tick),
        ]
    );

    // The six wrong-way families, each with its explicit status: two countable
    // intervals, a reported distance and duration, and the no-observation status
    // of a run whose subject met no one.
    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 2);
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.status,
        MetricStatus::Reported
    );
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.value,
        Some(interval_duration(&run))
    );
    assert_eq!(
        wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::Reported
    );
    let distance_m = wrong_way
        .run
        .wrong_way_distance_m
        .value
        .expect("the recorded intervals carry a distance");
    assert!(
        distance_m > 0.0,
        "the turned rider travels its opposing traversals: {distance_m} m"
    );
    assert!(
        distance_m <= 6.0 * interval_duration(&run),
        "no whole step exceeds the fixture's 6 m/s: {distance_m} m"
    );
    assert_eq!(
        wrong_way.run.wrong_way_exposure_agent_s.status,
        MetricStatus::NotObserved
    );
    assert_eq!(wrong_way.run.wrong_way_exposure_agent_s.value, None);
    assert_count(&wrong_way.run.wrong_way_encounters, 0);
    assert_count(&wrong_way.run.wrong_way_conflicts, 0);

    // The rule, facility, participant-pair, and mode-pair buckets name the same
    // two intervals, and the countable families of the inert buckets stay zero.
    // The decision's `permit` binds the corridor it was decided on; the
    // continuance carries no statement, so its own interval is bucketed under
    // `none` exactly as the compiled policy leaves it.
    assert_bucket_counts(
        &wrong_way.by_rule,
        &BTreeMap::from([("permit".to_owned(), 1), ("none".to_owned(), 1)]),
    );
    assert_bucket_counts(
        &wrong_way.by_facility,
        &BTreeMap::from([
            ("facility:a".to_owned(), 1),
            ("facility:a_left".to_owned(), 1),
        ]),
    );
    assert_bucket_counts(
        &wrong_way.by_pair,
        &BTreeMap::from([(format!("agent:{}", subject.get()), 2)]),
    );
    assert_count(
        &wrong_way.by_mode_pair["vehicle_vehicle"].wrong_way_intervals,
        2,
    );
    assert_eq!(
        wrong_way.by_facility["facility:b"]
            .wrong_way_intervals
            .status,
        MetricStatus::Reported
    );
    assert_count(&wrong_way.by_facility["facility:b"].wrong_way_intervals, 0);
    assert!(bucket_intervals(&wrong_way.by_movement) <= 2);

    // The version decision of this slice: the fixture's families land under the
    // existing metric definition v3, so the definition declares no new revision.
    assert_eq!(METRIC_DEFINITION_VERSION, 3);
}

/// The fixture's second corridor authors the same connected topology with no
/// statement binding its modes, so the opposing option is the default
/// prohibition: the decision records `noncompliant_choice`, the interval is a
/// violation, and it is bucketed under `none` rather than under a rule.
#[test]
fn the_checked_in_fixture_records_its_prohibited_but_connected_choice() {
    let scenario = wrong_way_fixture();
    let corridor = guide_path(&scenario, "guide_b");
    let facility = fixture_facility(&scenario, "b");
    let continuance_facility = fixture_facility(&scenario, "b_left");
    let movement = fixture_movement(&scenario, "through_b");

    let mut subject: Option<AgentId> = None;
    let run = drive_wrong_way_run(
        Simulation::new(scenario, RunConfig::new(0)).expect("the fixture builds"),
        6000,
        |sim, _| {
            if subject.is_none() {
                if let [body] = live_bodies(sim, corridor)[..]
                    && (5.0..=20.0).contains(&body.s_m)
                {
                    assert!(
                        sim.request_wrong_way_entry(body.id),
                        "the disconnected corridor is the only one that refuses the entry"
                    );
                    subject = Some(body.id);
                }
                return Drive::Continue;
            }
            match subject.is_some_and(|subject| !live(sim, subject)) {
                true => Drive::Stop,
                false => Drive::Continue,
            }
        },
    );
    let subject = subject.expect("the prohibited corridor admits a lone rider");
    let records = subject_records(&run, subject);

    assert_eq!(
        records.boundaries.len(),
        4,
        "the connected corridor records one interval per object: {:?}",
        records.boundaries
    );
    let open = &records.boundaries[0];
    let close = &records.boundaries[1];
    assert!(open.entering);
    assert_eq!(open.facility, facility);
    assert_eq!(records.boundaries[2].facility, continuance_facility);
    assert_eq!(
        close.tick, records.handoffs[0],
        "the handoff onto the continuance closes the interval"
    );
    let (despawned, despawn_reason) = records
        .despawned
        .expect("the turned rider completes its route and despawns");
    assert_eq!(
        records.boundaries[3].tick, despawned,
        "the route exit is the final close boundary"
    );
    assert_eq!(despawn_reason, DespawnReason::ExitedPath);
    assert_eq!(
        interval_ticks(&run, subject),
        vec![
            (facility, open.tick, close.tick),
            (
                continuance_facility,
                records.boundaries[2].tick,
                records.boundaries[3].tick
            ),
        ]
    );

    // The decision reaches the draw because the traversal is physically
    // possible, and legality alone rejects it: no statement binds the pair, so
    // the reason is the noncompliant choice and the traversal is a violation.
    assert_eq!(
        open.perceived_rule, None,
        "no statement binds the prohibited corridor's pair"
    );
    assert_eq!(open.reason, WrongWayReason::NoncompliantChoice);
    assert!(open.violating);
    assert_eq!(open.direction, MovementDirection::Reverse);
    assert_eq!(open.nominal_direction, NominalDirection::Forward);
    assert_eq!(open.movement, Some(movement.index()));

    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 2);
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.status,
        MetricStatus::Reported
    );
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.value,
        Some(interval_duration(&run))
    );
    assert_eq!(
        wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::Reported
    );
    assert!(
        wrong_way
            .run
            .wrong_way_distance_m
            .value
            .expect("the recorded intervals carry a distance")
            > 0.0
    );
    assert_eq!(
        wrong_way.run.wrong_way_exposure_agent_s.status,
        MetricStatus::NotObserved
    );
    assert_count(&wrong_way.run.wrong_way_encounters, 0);
    assert_count(&wrong_way.run.wrong_way_conflicts, 0);
    assert_bucket_counts(
        &wrong_way.by_rule,
        &BTreeMap::from([("none".to_owned(), 2)]),
    );
    assert_bucket_counts(
        &wrong_way.by_facility,
        &BTreeMap::from([
            ("facility:b".to_owned(), 1),
            ("facility:b_left".to_owned(), 1),
        ]),
    );
    assert_bucket_counts(
        &wrong_way.by_pair,
        &BTreeMap::from([(format!("agent:{}", subject.get()), 2)]),
    );
}

/// The fixture's third corridor authors no connector along its reverse
/// direction, so the opposing traversal is physically impossible: the entry is
/// refused before any draw, the rider emits no boundary and no metric
/// contribution, and it completes its nominal route.
#[test]
fn the_checked_in_fixture_refuses_its_disconnected_entry() {
    let scenario = wrong_way_fixture();
    let corridor = guide_path(&scenario, "guide_c");
    let facility = fixture_facility(&scenario, "c");

    let mut subject: Option<AgentId> = None;
    let run = drive_wrong_way_run(
        Simulation::new(scenario, RunConfig::new(0)).expect("the fixture builds"),
        6000,
        |sim, _| {
            if subject.is_none() {
                if let [body] = live_bodies(sim, corridor)[..]
                    && (5.0..=20.0).contains(&body.s_m)
                {
                    assert!(
                        !sim.request_wrong_way_entry(body.id),
                        "a corridor with no connected opposing traversal records no request"
                    );
                    subject = Some(body.id);
                }
                return Drive::Continue;
            }
            match subject.is_some_and(|subject| !live(sim, subject)) {
                true => Drive::Stop,
                false => Drive::Continue,
            }
        },
    );
    let subject = subject.expect("the disconnected corridor admits a lone rider");
    let records = subject_records(&run, subject);

    // The refusal is total: no boundary, no handoff onto a continuance, and the
    // ordinary spawn-to-despawn lifecycle of a rider that never turned.
    assert!(
        records.boundaries.is_empty(),
        "a refused entry emits no boundary: {:?}",
        records.boundaries
    );
    assert!(
        records.handoffs.is_empty(),
        "the refused rider never enters the opposing traversal that continues onto a connector"
    );
    let (_, despawn_reason) = records
        .despawned
        .expect("the refused rider completes its nominal route and despawns");
    assert_eq!(despawn_reason, DespawnReason::ExitedPath);

    // Every family is explicit: the countable families are observed zeros and
    // the value families report the run's no-observation status, because this
    // fixture declares the policy and the capability that could host a
    // traversal in the twin corridors.
    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 0);
    for value in [
        &wrong_way.run.wrong_way_distance_m,
        &wrong_way.run.wrong_way_duration_s,
        &wrong_way.run.wrong_way_exposure_agent_s,
    ] {
        assert_eq!(value.status, MetricStatus::NotObserved);
        assert_eq!(value.value, None);
    }
    assert_count(&wrong_way.run.wrong_way_encounters, 0);
    assert_count(&wrong_way.run.wrong_way_conflicts, 0);

    // The disconnected corridor is a bucket that could host an opposing
    // traversal and recorded none, and the refused rider names no rule and no
    // participant pair.
    let disconnected = format!("facility:{}", scenario_facility_name(&run, facility));
    assert_eq!(
        wrong_way.by_facility[&disconnected]
            .wrong_way_distance_m
            .status,
        MetricStatus::NotObserved
    );
    assert_count(&wrong_way.by_facility[&disconnected].wrong_way_intervals, 0);
    assert!(
        wrong_way.by_facility.keys().len() >= 7,
        "every compiled facility is a bucket: {:?}",
        wrong_way.by_facility.keys().collect::<Vec<_>>()
    );
    assert!(wrong_way.by_rule.is_empty(), "no interval names a rule");
    assert!(
        wrong_way.by_pair.is_empty(),
        "no interval names a participant pair"
    );
    assert!(
        wrong_way.by_movement.is_empty(),
        "no interval names a movement"
    );
}

/// The facility key one compiled facility is bucketed under, read through the
/// scenario the run ended with.
fn scenario_facility_name(run: &WrongWayRun, facility: usize) -> String {
    run.sim
        .scenario()
        .facility(FacilityId::from_index(facility))
        .expect("the facility is compiled")
        .name()
        .to_owned()
}

/// The fixture's fourth corridor is occupied: a leading rider turns into a body
/// travelling the rule direction, and the ordinary leader constraint, collision
/// scan, and anti-overlap cap bound the encounter with no bypass and no overlap.
/// The encounter is the run's conflict and its encounter, and the interval it
/// leaves open closes at the run end.
///
/// At the pinned seed the sources place the pair at step 119 with the leading
/// rider at 25.2 m and a 9.8 m bumper gap behind it, which is inside the 18 m
/// two 6 m/s bodies need to stop under the authored 2 m/s² braking, so the
/// head-on meeting is unavoidable. The scan reports one entering near miss and
/// one contact, the least bumper gap between the pair stays at zero within
/// floating-point noise, and the turned rider never leaves its corridor or runs
/// a lateral maneuver.
#[test]
fn the_checked_in_fixture_bounds_its_occupied_opposing_corridor() {
    let occupied = drive_occupied_corridor();
    let run = &occupied.run;
    let subject = occupied.subject;
    let occupancy = occupied.occupancy;
    let facility = fixture_facility(run.sim.scenario(), "d");
    let movement = fixture_movement(run.sim.scenario(), "through_d");

    // One boundary: the entry opens an interval that the run end closes, because
    // the blocked pair never leaves the corridor inside the driven horizon.
    assert_eq!(
        occupied.boundaries.len(),
        1,
        "the turned rider's interval is still open at the run end: {:?}",
        occupied.boundaries
    );
    let open = &occupied.boundaries[0];
    assert!(open.entering);
    assert_eq!(open.facility, facility);
    assert_eq!(open.perceived_rule, Some(PermissionEffect::Permit));
    assert_eq!(open.reason, WrongWayReason::LegalPermission);
    assert!(!open.violating);
    assert_eq!(open.direction, MovementDirection::Reverse);
    assert_eq!(open.nominal_direction, NominalDirection::Forward);
    assert_eq!(open.movement, Some(movement.index()));

    let intervals = run.sim.opposing_traversal_tracker().intervals();
    assert_eq!(intervals.len(), 1, "one interval, the subject's own");
    let interval = &intervals[0];
    assert_eq!(interval.agent, subject);
    assert_eq!(interval.facility.index(), facility);
    assert_eq!(
        interval.start_tick, open.tick,
        "the interval opens on the boundary's own tick"
    );
    assert_eq!(
        interval.end_tick, run.final_tick,
        "the run-end closure's close time is the run's final simulation time"
    );
    assert!(interval.duration_s() > 0.0);
    assert!(
        !interval_ticks(run, subject).is_empty(),
        "the tracker holds the subject's closed interval"
    );

    // The encounter's ordinary visibility: the collision scan reports the pair
    // in its near-miss band and then in its contact band, and holds them apart
    // rather than overlapping them. T-H1's minimum separation is the scan's own
    // signed clearance, which never falls below -1e-9 m.
    assert!(
        occupied.near_misses >= 1,
        "the collision scan reports the pair inside its near-miss band"
    );
    assert!(
        occupied.contacts >= 1,
        "the collision scan reports the pair inside its contact band"
    );
    assert!(
        occupied.min_clearance_m >= -1e-9,
        "T-H1: the pair's least reported clearance is {} m",
        occupied.min_clearance_m
    );
    assert!(
        occupied.min_gap_m >= -1e-9,
        "T-H2: the anti-overlap cap holds the bodies apart, least gap {} m",
        occupied.min_gap_m
    );
    // T-H2's bypass count is zero in the only way the fixture could produce one:
    // the turned rider never leaves its corridor and never runs a lateral
    // maneuver around the occupied corridor.
    assert!(
        !occupied.bypassed,
        "the turned rider never bypasses the occupied corridor"
    );

    // The six wrong-way families with their explicit statuses: one interval, a
    // reported distance and duration, and the reported exposure, encounter, and
    // conflict the head-on meeting produces.
    let wrong_way = &run.metrics.wrong_way;
    assert_count(&wrong_way.run.wrong_way_intervals, 1);
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.status,
        MetricStatus::Reported
    );
    assert_eq!(
        wrong_way.run.wrong_way_duration_s.value,
        Some(interval.duration_s())
    );
    assert_eq!(
        wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::Reported
    );
    assert!(
        wrong_way
            .run
            .wrong_way_distance_m
            .value
            .expect("the recorded interval carries a distance")
            > 0.0
    );
    assert_eq!(
        wrong_way.run.wrong_way_exposure_agent_s.status,
        MetricStatus::Reported
    );
    assert!(
        wrong_way
            .run
            .wrong_way_exposure_agent_s
            .value
            .expect("the co-present pair carries exposure")
            > 0.0
    );
    assert!(wrong_way.run.wrong_way_encounters.value.expect("a count") >= 1.0);
    assert!(
        wrong_way.run.wrong_way_conflicts.value.expect("a count")
            == occupied.contacts as f64 + occupied.near_misses as f64
    );

    // The meeting is bucketed under the participant pair, the corridor, and the
    // permit statement the subject acted under.
    let pair_key = format!(
        "agent:{}|agent:{}",
        subject.get().min(occupancy.get()),
        subject.get().max(occupancy.get())
    );
    assert!(
        wrong_way.by_pair.contains_key(&pair_key),
        "the encounter is keyed by the participant pair: {:?}",
        wrong_way.by_pair.keys().collect::<Vec<_>>()
    );
    assert!(
        wrong_way.by_pair[&pair_key]
            .wrong_way_conflicts
            .value
            .expect("a count")
            >= 1.0
    );
    assert_count(&wrong_way.by_rule["permit"].wrong_way_intervals, 1);
    assert_count(&wrong_way.by_facility["facility:d"].wrong_way_intervals, 1);
    assert_count(
        &wrong_way.by_mode_pair["vehicle_vehicle"].wrong_way_intervals,
        1,
    );
}

/// The occupied corridor's encounter is a pure function of the fixture and the
/// seed: an identical run turns the same pair at the same step, bounds it the
/// same way, and closes the same interval at the run end.
#[test]
fn the_checked_in_fixture_replays_its_occupied_corridor_identically() {
    let first = drive_occupied_corridor();
    let second = drive_occupied_corridor();

    assert_eq!(first.subject, second.subject);
    assert_eq!(first.occupancy, second.occupancy);
    assert_eq!(first.boundaries, second.boundaries);
    assert_eq!(
        interval_ticks(&first.run, first.subject),
        interval_ticks(&second.run, second.subject)
    );
    assert_eq!(first.min_gap_m, second.min_gap_m);
    assert_eq!(first.min_clearance_m, second.min_clearance_m);
    assert_eq!(first.contacts, second.contacts);
    assert_eq!(first.near_misses, second.near_misses);
}

/// The checked-in fixture reports its wrong-way block through the real CLI run
/// directory, with the explicit statuses its declarations imply: the fixture
/// authors the policy and the capability, so a run that requests no entry
/// reports no observation rather than an inapplicable status, and every compiled
/// facility is a bucket.
#[test]
fn the_checked_in_fixture_reports_its_wrong_way_block_through_the_cli() {
    let scratch = Scratch::new("wrong-way-fixture");
    let output = Command::new(CLI)
        .arg("run")
        .arg(repo_path(WRONG_WAY_FIXTURE))
        .args(["--seed", "0"])
        .args(["--run-dir", "run"])
        .current_dir(&scratch.dir)
        .output()
        .expect("hekate-cli runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let artifact = read_artifact(&scratch.path("run"));

    assert_count(&artifact.wrong_way.run.wrong_way_intervals, 0);
    assert_eq!(
        artifact.wrong_way.run.wrong_way_distance_m.status,
        MetricStatus::NotObserved
    );
    assert_eq!(
        artifact.wrong_way.run.wrong_way_duration_s.status,
        MetricStatus::NotObserved
    );
    assert_eq!(
        artifact.wrong_way.run.wrong_way_exposure_agent_s.status,
        MetricStatus::NotObserved
    );
    assert_count(&artifact.wrong_way.run.wrong_way_encounters, 0);
    assert_count(&artifact.wrong_way.run.wrong_way_conflicts, 0);

    let facilities: Vec<&str> = artifact
        .wrong_way
        .by_facility
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        facilities,
        vec![
            "facility:a",
            "facility:a_left",
            "facility:b",
            "facility:b_left",
            "facility:c",
            "facility:d",
            "facility:d_left",
        ],
        "every compiled facility is a bucket in the artifact"
    );
    assert!(artifact.wrong_way.by_rule.is_empty());
    assert!(artifact.wrong_way.by_pair.is_empty());
    assert_eq!(
        artifact.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
}
