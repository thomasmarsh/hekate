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
use serde_json::Value;
use sha2::{Digest, Sha256};
use tangle_cli::{
    EVENT_STREAM_FILE, MANIFEST_FILE, METRIC_DEFINITION_VERSION, METRICS_FILE, MetricStatus,
    MetricValue, OperationalValues, RunDirectoryRequest, RunMetrics, RunMetricsArtifact,
    SUMMARY_FILE, SamplingPolicy, ScenarioProvenance, canonical_run_captured, load_scenario_hashed,
    write_run_directory,
};
use tangle_model::CompiledScenario;
use tangle_sim::{
    AgentId, AgentMode, Event, MetricMinimum, ModePair, MovementKey, OperationValues,
    PostEncroachment, RunConfig, Simulation,
};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

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
            "tangle-cli-run-metrics-{name}-{}",
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

fn load(relative: &str) -> (CompiledScenario, String) {
    load_scenario_hashed(&repo_path(relative)).expect("scenario loads")
}

fn provenance(
    relative: &str,
    scenario: &CompiledScenario,
    content_sha256: &str,
) -> ScenarioProvenance {
    ScenarioProvenance {
        id: scenario.id().to_owned(),
        source_path: relative.to_owned(),
        schema_version: scenario.schema_version(),
        content_sha256: content_sha256.to_owned(),
    }
}

/// Run one scenario through the capturing loop and write its run directory.
fn write_run(directory: &Path, relative: &str, seed: u64, ticks: u64) -> RunMetrics {
    let (scenario, content_sha256) = load(relative);
    let provenance = provenance(relative, &scenario, &content_sha256);
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
    let summary: tangle_cli::RunSummary = serde_json::from_str(
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
fn expected_event_counts(directory: &Path, reference: &Reference) -> tangle_cli::EventCounts {
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
    tangle_cli::EventCounts {
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
        .expect("tangle-cli runs")
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
        .expect("tangle-cli runs");
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
