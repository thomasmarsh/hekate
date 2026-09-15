//! Contract tests for the `aggregate` command.
//!
//! The aggregation is the batch-level experiment record: for every metric a
//! batch's runs report, the across-seed count, mean, spread, and 95% Student-t
//! confidence interval, disaggregated by mode pair and by movement. These tests
//! pin the statistics against hand-computed values, the statuses against a
//! known set of not-applicable and not-observed seeds (counted, never zeroed),
//! the manifest and definition-version links of every reported number, the
//! deterministic ordering, the refusal of a run that disagrees with what the
//! batch recorded, and the command's read-only treatment of completed artifacts.
//!
//! Most tests build a synthetic batch with hand-written `metrics.json` files,
//! because only known values can be asserted exactly; the last test runs a real
//! batch through the binary and checks that aggregating it mutates nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use hekate_cli::{
    AGGREGATION_FILE, AGGREGATION_VERSION, AggregateError, Aggregation, BATCH_MANIFEST_FILE,
    BatchManifest, BatchRequest, BatchRun, BatchSpec, CONFIDENCE_LEVEL, ClosePassMetrics,
    ClosePassMinimum, ClosePassValues, EVENT_FAMILY_LABELS, EventCounts,
    LARGEST_TABULATED_DEGREES_OF_FREEDOM, LEAST_INTERVAL_SEEDS, MANIFEST_FILE,
    METRIC_DEFINITION_VERSION, METRICS_FILE, MetricDistribution, MetricStatus, MetricValue,
    MovementMinima, NORMAL_CRITICAL_975, OperationalMetrics, OperationalValues, RunMetricsArtifact,
    SamplingPolicy, ScenarioProvenance, T_CRITICAL_975, aggregate_batch, load_scenario_provenance,
    run_batch,
};
use hekate_sim::RunConfig;
use sha2::{Digest, Sha256};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
const MIXED: &str = "scenarios/benchmarks/mixed_interaction_v1.json5";
/// Long enough that the mixed benchmark records a reported time to collision, a
/// cross-mode pair, and several movement buckets.
const MIXED_TICKS: u64 = 600;
/// Few enough ticks that a multi-seed batch stays fast while every run still
/// records events and a sampled trajectory.
const WIRING_TICKS: u64 = 60;

/// Every counted event family metric definition v1 reports.
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

/// The two kinded families and their variant kinds.
const KINDS: [(&str, [&str; 2]); 2] = [
    ("violations", ["ran_red_light", "crossed_against_signal"]),
    ("control_transitions", ["signal_stop", "crossing_wait"]),
];

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-cli-aggregate-{name}-{}",
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

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn file_sha256(path: &Path) -> String {
    sha256_hex(
        &std::fs::read(path)
            .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display())),
    )
}

/// The content hash of every file under `root`, keyed by its relative path.
fn tree_hashes(root: &Path) -> Vec<(String, String)> {
    fn collect(root: &Path, directory: &Path, out: &mut Vec<(String, String)>) {
        let entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("cannot read '{}': {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("directory entry is readable").path();
            if path.is_dir() {
                collect(root, &path, out);
            } else {
                let name = path
                    .strip_prefix(root)
                    .expect("every file is inside the root")
                    .to_string_lossy()
                    .into_owned();
                out.push((name, file_sha256(&path)));
            }
        }
    }
    let mut hashes = Vec::new();
    collect(root, root, &mut hashes);
    hashes.sort();
    hashes
}

/// The pretty JSON the run and aggregation writers put on disk.
fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    let mut json = serde_json::to_string_pretty(value).expect("artifact serializes");
    json.push('\n');
    std::fs::write(path, json)
        .unwrap_or_else(|error| panic!("cannot write '{}': {error}", path.display()));
}

/// A reported metric value.
fn reported(value: f64) -> MetricValue {
    MetricValue {
        status: MetricStatus::Reported,
        value: Some(value),
        agent: Some(0),
        other: Some(1),
        mode_pair: Some("vehicle_vehicle".to_owned()),
        tick: Some(7),
    }
}

/// A metric value with no value and an explicit status.
fn absent(status: MetricStatus) -> MetricValue {
    MetricValue {
        status,
        value: None,
        agent: None,
        other: None,
        mode_pair: None,
        tick: None,
    }
}

/// The three mode-pair entries every run artifact carries.
fn mode_pairs(
    vehicle_vehicle: MetricValue,
    vehicle_pedestrian: MetricValue,
    pedestrian_pedestrian: MetricValue,
) -> BTreeMap<String, MetricValue> {
    BTreeMap::from([
        ("vehicle_vehicle".to_owned(), vehicle_vehicle),
        ("vehicle_pedestrian".to_owned(), vehicle_pedestrian),
        ("pedestrian_pedestrian".to_owned(), pedestrian_pedestrian),
    ])
}

/// One movement bucket entry: its sorted two-key bucket key and its minima.
fn movement_bucket(
    keys: [&str; 2],
    separation: MetricValue,
    ttc: MetricValue,
    pet: MetricValue,
) -> (String, MovementMinima) {
    assert!(keys[0] <= keys[1], "the caller sorts the movement keys");
    (
        format!("{}|{}", keys[0], keys[1]),
        MovementMinima {
            movement_keys: [keys[0].to_owned(), keys[1].to_owned()],
            minimum_separation_m: separation,
            minimum_ttc_s: ttc,
            minimum_post_encroachment_s: pet,
        },
    )
}

/// Event counts with every family present and `collisions` set.
///
/// `by_mode` and `by_movement` are keyed by the mode or movement the caller
/// names, and are transposed into the run artifact's own shape, whose outer key
/// is the family and whose inner key is the mode or movement.
fn event_counts(
    collisions: u64,
    by_mode: &BTreeMap<String, BTreeMap<String, u64>>,
    by_movement: &BTreeMap<String, BTreeMap<String, u64>>,
) -> EventCounts {
    let mut by_family: BTreeMap<String, u64> = FAMILIES
        .into_iter()
        .map(|family| (family.to_owned(), 0))
        .collect();
    by_family.insert("collisions".to_owned(), collisions);
    let by_family_kind = KINDS
        .into_iter()
        .map(|(family, kinds)| {
            (
                family.to_owned(),
                kinds.into_iter().map(|kind| (kind.to_owned(), 0)).collect(),
            )
        })
        .collect();
    // Every family is present in the transposed map, `0` for the modes and
    // movements the caller did not name: the run artifact's own `by_family` set
    // always holds all ten, and a count of an absent family is an observed zero.
    let transpose = |slices: &BTreeMap<String, BTreeMap<String, u64>>| {
        FAMILIES
            .into_iter()
            .map(|family| {
                let counts = slices
                    .iter()
                    .map(|(key, counts)| (key.clone(), counts.get(family).copied().unwrap_or(0)))
                    .collect();
                (family.to_owned(), counts)
            })
            .collect()
    };
    EventCounts {
        total: by_family.values().sum(),
        by_family,
        by_family_kind,
        by_family_mode: transpose(by_mode),
        by_family_movement: transpose(by_movement),
    }
}

/// One operational bucket with every metric reported at `value` and `agents`
/// standing agents queued, so a test can read a known value back.
fn operational_bucket(value: f64, agents: f64) -> OperationalValues {
    OperationalValues {
        throughput_agents_per_s: reported(value),
        mean_travel_time_s: reported(value),
        total_travel_time_s: reported(value),
        mean_stopped_delay_s: reported(value),
        total_stopped_delay_s: reported(value),
        mean_control_delay_s: reported(value),
        total_control_delay_s: reported(value),
        maximum_queue_length_agents: reported(agents),
        maximum_queue_duration_s: reported(value),
        mean_queue_duration_s: reported(value),
    }
}

/// The metric values one synthetic seed reports.
#[derive(Clone)]
struct SyntheticSeed {
    separation: MetricValue,
    ttc: MetricValue,
    pet: MetricValue,
    mode_pairs: BTreeMap<String, MetricValue>,
    movements: BTreeMap<String, MovementMinima>,
    collisions: u64,
    /// Counted records per mode and family.
    event_families_by_mode: BTreeMap<String, BTreeMap<String, u64>>,
    /// Counted records per movement key and family.
    event_families_by_movement: BTreeMap<String, BTreeMap<String, u64>>,
    /// The operational values of each movement key the run carried.
    operational_by_movement: BTreeMap<String, OperationalValues>,
    /// The overtaking and close-pass families of every bucket the run reports.
    close_pass: ClosePassMetrics,
}

impl Default for SyntheticSeed {
    fn default() -> Self {
        Self {
            separation: reported(1.0),
            ttc: absent(MetricStatus::NotApplicable),
            pet: absent(MetricStatus::NotObserved),
            mode_pairs: mode_pairs(
                reported(1.0),
                absent(MetricStatus::NotObserved),
                absent(MetricStatus::NotObserved),
            ),
            movements: BTreeMap::new(),
            collisions: 0,
            event_families_by_mode: BTreeMap::new(),
            event_families_by_movement: BTreeMap::new(),
            operational_by_movement: BTreeMap::new(),
            close_pass: ClosePassMetrics::not_observed(),
        }
    }
}

/// One bucket's close-pass families: the overtaking counts, the closed
/// observations, the least clearance, and the violations the test supplies,
/// with no bands.
fn close_pass_bucket(
    overtaking: [u64; 4],
    close_passes: u64,
    minimum: ClosePassMinimum,
    violations: u64,
) -> ClosePassValues {
    ClosePassValues {
        overtake_attempts: reported(overtaking[0] as f64),
        overtake_commits: reported(overtaking[1] as f64),
        overtake_completions: reported(overtaking[2] as f64),
        overtake_aborts: reported(overtaking[3] as f64),
        close_passes: reported(close_passes as f64),
        close_pass_minimum_clearance_m: minimum,
        clearance_band_durations_s: BTreeMap::new(),
        close_pass_violations: reported(violations as f64),
    }
}

/// A reported close-pass minimum, with the pair, time, and relative speed the
/// artifact records beside it.
fn minimum_reported(value: f64) -> ClosePassMinimum {
    ClosePassMinimum {
        status: MetricStatus::Reported,
        value: Some(value),
        agent: Some(0),
        other: Some(1),
        mode_pair: Some("vehicle_vehicle".to_owned()),
        time_s: Some(12.0),
        relative_speed_mps: Some(4.0),
    }
}

/// A close-pass minimum with no value and an explicit applicability.
fn minimum_absent(status: MetricStatus) -> ClosePassMinimum {
    ClosePassMinimum {
        status,
        value: None,
        agent: None,
        other: None,
        mode_pair: None,
        time_s: None,
        relative_speed_mps: None,
    }
}

/// The close-pass families of one bucket as a band-carrying value: the given
/// bucket with its declared clearance bands set.
fn with_bands(mut values: ClosePassValues, bands: &[(u32, MetricValue)]) -> ClosePassValues {
    values.clearance_band_durations_s = bands.iter().cloned().collect();
    values
}

/// Write a batch root whose seeds report `seeds`, in the order given.
///
/// The run directories are synthetic — a stand-in `manifest.json` whose bytes
/// the aggregation hashes, and a `metrics.json` built from the given values —
/// so the batch is well formed by construction and every expected statistic is
/// known exactly. The `runs` are written in the order given, which lets a test
/// hand a batch manifest an unsorted run list.
fn write_synthetic_batch(root: &Path, seeds: &[(u64, SyntheticSeed)]) {
    std::fs::create_dir_all(root).expect("the batch root is created");
    let mut runs = Vec::new();
    for (seed, values) in seeds {
        let directory = format!("seed-{seed}");
        let run_dir = root.join(&directory);
        std::fs::create_dir_all(&run_dir).expect("the run directory is created");
        let manifest_json = format!("{{\"manifest_version\": 1, \"seed\": {seed}}}\n");
        std::fs::write(run_dir.join(MANIFEST_FILE), &manifest_json)
            .expect("the manifest is written");
        let manifest_sha256 = sha256_hex(manifest_json.as_bytes());
        write_json(
            &run_dir.join(METRICS_FILE),
            &RunMetricsArtifact {
                metric_definition_version: METRIC_DEFINITION_VERSION,
                manifest_sha256: manifest_sha256.clone(),
                minimum_separation_m: values.separation.clone(),
                minimum_ttc_s: values.ttc.clone(),
                minimum_post_encroachment_s: values.pet.clone(),
                mode_pair_minimum_separation_m: values.mode_pairs.clone(),
                movement_minima: values.movements.clone(),
                event_counts: event_counts(
                    values.collisions,
                    &values.event_families_by_mode,
                    &values.event_families_by_movement,
                ),
                operational: OperationalMetrics {
                    run: OperationalValues::not_observed(),
                    by_mode: ["vehicle", "pedestrian"]
                        .into_iter()
                        .map(|mode| (mode.to_owned(), OperationalValues::not_observed()))
                        .collect(),
                    by_movement: values.operational_by_movement.clone(),
                },
                close_pass: values.close_pass.clone(),
            },
        );
        runs.push(BatchRun {
            seed: *seed,
            directory,
            trace_sha256: "0".repeat(64),
            manifest_sha256,
        });
    }
    write_json(
        &root.join(BATCH_MANIFEST_FILE),
        &BatchManifest {
            batch_manifest_version: 1,
            spec: BatchSpec {
                scenario: ScenarioProvenance {
                    id: "synthetic".to_owned(),
                    source_path: "synthetic.json5".to_owned(),
                    schema_version: 1,
                    content_sha256: "0".repeat(64),
                    normalized_sha256: "0".repeat(64),
                    migration_version: 1,
                },
                ticks: 1,
                fidelity: "standard".to_owned(),
                step_s: 0.05,
                event_version: 2,
                model_version: "synthetic".to_owned(),
                build_revision: "0.0.0".to_owned(),
                sampling: SamplingPolicy::default(),
            },
            seed_bank: None,
            seeds: seeds.iter().map(|(seed, _)| *seed).collect(),
            runs,
        },
    );
}

/// Assert two floats agree to within a relative tolerance no rounding can beat.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} differs from the expected {expected}"
    );
}

/// Assert a two-sided Student-t interval built from `values` by hand.
fn assert_interval(interval: &hekate_cli::ConfidenceInterval, values: &[f64]) {
    let count = values.len();
    let mean = values.iter().sum::<f64>() / count as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (count as f64 - 1.0);
    let standard_error = variance.sqrt() / (count as f64).sqrt();
    let critical_value = T_CRITICAL_975[count - 2];
    assert_close(interval.critical_value, critical_value);
    assert_eq!(interval.degrees_of_freedom as usize, count - 1);
    assert_close(interval.standard_error, standard_error);
    assert_close(interval.half_width, critical_value * standard_error);
    assert_close(interval.lower, mean - critical_value * standard_error);
    assert_close(interval.upper, mean + critical_value * standard_error);
}

/// The run-level metric keys a run artifact reports, spelled independently of
/// the aggregation: the three interaction minima, the total, every family, and
/// every variant kind.
/// The operational metrics metric definition v2 reports for one bucket, by
/// artifact field name. Restated here so the aggregation's metric set is pinned
/// independently of the writer that produced it.
const OPERATIONAL_FIELDS: [&str; 10] = [
    "maximum_queue_duration_s",
    "maximum_queue_length_agents",
    "mean_control_delay_s",
    "mean_queue_duration_s",
    "mean_stopped_delay_s",
    "mean_travel_time_s",
    "throughput_agents_per_s",
    "total_control_delay_s",
    "total_stopped_delay_s",
    "total_travel_time_s",
];

/// The metric keys one seed's artifact contributes to the aggregation, restated
/// from the artifact rather than from the writer.
fn expected_metric_keys(artifact: &RunMetricsArtifact) -> Vec<String> {
    let mut keys = vec![
        "minimum_post_encroachment_s".to_owned(),
        "minimum_separation_m".to_owned(),
        "minimum_ttc_s".to_owned(),
        "event_counts.total".to_owned(),
    ];
    keys.extend(
        artifact
            .event_counts
            .by_family
            .keys()
            .map(|family| format!("event_counts.by_family.{family}")),
    );
    for (family, kinds) in &artifact.event_counts.by_family_kind {
        keys.extend(
            kinds
                .keys()
                .map(|kind| format!("event_counts.by_family_kind.{family}.{kind}")),
        );
    }
    keys.extend(
        OPERATIONAL_FIELDS
            .iter()
            .map(|field| format!("operational.run.{field}")),
    );
    // The run bucket's close-pass families, restated from metric definition v3
    // rather than from the aggregation: the four overtaking counts, the two
    // close-pass counts, and the least clearance, plus one key per declared
    // clearance band.
    keys.extend(
        [
            "overtake_attempts",
            "overtake_commits",
            "overtake_completions",
            "overtake_aborts",
            "close_passes",
            "close_pass_minimum_clearance_m",
            "close_pass_violations",
        ]
        .into_iter()
        .map(|family| format!("close_pass.run.{family}")),
    );
    keys.extend(
        artifact
            .close_pass
            .run
            .clearance_band_durations_s
            .keys()
            .map(|band| format!("close_pass.run.clearance_band_durations_s.{band}")),
    );
    assert_eq!(
        artifact.operational.by_mode.keys().collect::<Vec<_>>(),
        vec!["pedestrian", "vehicle"],
        "every artifact reports both mode buckets"
    );
    for mode in ["pedestrian", "vehicle"] {
        keys.extend(
            OPERATIONAL_FIELDS
                .iter()
                .map(|field| format!("operational.by_mode.{mode}.{field}")),
        );
    }
    keys.sort();
    keys
}

/// The three statuses of a distribution must account for every seed exactly
/// once: a metric that is reported, not applicable, and not observed must not
/// leave a seed unaccounted for, which is how an absent value could be read as
/// a zero.
fn assert_statuses_account_for_every_seed(
    key: &str,
    distribution: &MetricDistribution,
    seeds: usize,
) {
    assert_eq!(
        distribution.count
            + distribution.not_applicable_seeds.len()
            + distribution.not_observed_seeds.len(),
        seeds,
        "the statuses of '{key}' must cover every seed"
    );
    assert_eq!(distribution.count, distribution.reported_seeds.len());
    assert_eq!(
        distribution.manifests.len(),
        distribution.reported_seeds.len()
    );
}

#[test]
fn every_reported_metric_gets_a_count_mean_spread_and_interval() {
    let scratch = Scratch::new("distribution");
    let root = scratch.path("batch");
    let values = [1.0, 2.0, 3.0, 4.0];
    let seeds: Vec<(u64, SyntheticSeed)> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                index as u64,
                SyntheticSeed {
                    separation: reported(*value),
                    collisions: index as u64,
                    ..SyntheticSeed::default()
                },
            )
        })
        .collect();
    write_synthetic_batch(&root, &seeds);

    let aggregation = aggregate_batch(&root).expect("a well-formed batch aggregates");
    assert_eq!(aggregation.aggregation_version, AGGREGATION_VERSION);
    assert_eq!(
        aggregation.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(
        aggregation
            .seeds
            .iter()
            .map(|seed| seed.seed)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );

    // The documented method the artifact reports with every interval.
    assert_eq!(
        aggregation.statistics.confidence_interval_method,
        "student_t"
    );
    assert_eq!(aggregation.statistics.confidence_level, CONFIDENCE_LEVEL);
    assert_eq!(aggregation.statistics.variance_denominator, "n - 1");
    assert_eq!(
        aggregation.statistics.smallest_tabulated_degrees_of_freedom,
        1
    );
    assert_eq!(
        aggregation.statistics.largest_tabulated_degrees_of_freedom,
        LARGEST_TABULATED_DEGREES_OF_FREEDOM
    );
    assert_eq!(
        aggregation.statistics.critical_value_above_the_table,
        NORMAL_CRITICAL_975
    );
    assert_eq!(
        aggregation.statistics.least_interval_seeds,
        LEAST_INTERVAL_SEEDS
    );

    let separation = &aggregation.metrics["minimum_separation_m"];
    assert_eq!(
        separation.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(separation.unit, "metres");
    assert_eq!(separation.count, 4);
    assert_close(separation.mean.expect("a mean"), 2.5);
    assert_close(separation.spread.minimum.expect("a least"), 1.0);
    assert_close(separation.spread.maximum.expect("a greatest"), 4.0);
    assert_close(separation.spread.range.expect("a range"), 3.0);
    assert_close(
        separation.spread.sample_variance.expect("a variance"),
        5.0 / 3.0,
    );
    assert_close(
        separation
            .spread
            .sample_standard_deviation
            .expect("a deviation"),
        (5.0_f64 / 3.0).sqrt(),
    );
    assert_interval(
        separation
            .confidence_interval
            .as_ref()
            .expect("four values have an interval"),
        &values,
    );
    assert_eq!(separation.reported_seeds, vec![0, 1, 2, 3]);

    // An always-reported count is a metric like any other: its mean over seeds
    // is a distribution, and a reported zero is a measured zero.
    let collisions = &aggregation.metrics["event_counts.by_family.collisions"];
    assert_eq!(collisions.unit, "records");
    assert_eq!(collisions.count, 4);
    assert_close(collisions.mean.expect("a mean"), 1.5);
    assert_interval(
        collisions
            .confidence_interval
            .as_ref()
            .expect("four values have an interval"),
        &[0.0, 1.0, 2.0, 3.0],
    );
    assert_eq!(aggregation.metrics["event_counts.total"].count, 4);

    // Every metric the artifact reports is present, including the kinded ones.
    for family in FAMILIES {
        assert!(
            aggregation
                .metrics
                .contains_key(&format!("event_counts.by_family.{family}")),
            "family '{family}' must be a metric"
        );
    }
    for (family, kinds) in KINDS {
        for kind in kinds {
            assert!(
                aggregation
                    .metrics
                    .contains_key(&format!("event_counts.by_family_kind.{family}.{kind}"))
            );
        }
    }
}

#[test]
fn not_applicable_and_not_observed_seeds_are_counted_not_zeroed() {
    let scratch = Scratch::new("statuses");
    let root = scratch.path("batch");
    let seeds = vec![
        (
            0,
            SyntheticSeed {
                ttc: reported(1.0),
                ..SyntheticSeed::default()
            },
        ),
        (
            1,
            SyntheticSeed {
                ttc: absent(MetricStatus::NotApplicable),
                ..SyntheticSeed::default()
            },
        ),
        (
            2,
            SyntheticSeed {
                ttc: reported(3.0),
                separation: absent(MetricStatus::NotObserved),
                ..SyntheticSeed::default()
            },
        ),
    ];
    write_synthetic_batch(&root, &seeds);

    let aggregation = aggregate_batch(&root).expect("a well-formed batch aggregates");

    // Time to collision: two reported seeds, one not applicable. The mean is the
    // mean of the reported values, not of the seeds.
    let ttc = &aggregation.metrics["minimum_ttc_s"];
    assert_eq!(ttc.count, 2);
    assert_close(ttc.mean.expect("a mean"), 2.0);
    assert_eq!(ttc.reported_seeds, vec![0, 2]);
    assert_eq!(ttc.not_applicable_seeds, vec![1]);
    assert_eq!(ttc.not_observed_seeds, Vec::<u64>::new());
    assert_interval(
        ttc.confidence_interval
            .as_ref()
            .expect("two values have an interval"),
        &[1.0, 3.0],
    );
    assert!(
        (ttc.mean.expect("a mean") - (1.0 + 3.0) / 3.0).abs() > 0.5,
        "a not-applicable seed must not be averaged in as zero"
    );

    // Separation: two reported seeds, one that made no observation at all.
    let separation = &aggregation.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 2);
    assert_eq!(separation.not_observed_seeds, vec![2]);
    assert!(separation.not_applicable_seeds.is_empty());

    // No seed reported a post-encroachment time: no value at all, never zero.
    let pet = &aggregation.metrics["minimum_post_encroachment_s"];
    assert_eq!(pet.count, 0);
    assert_eq!(pet.mean, None);
    assert_eq!(pet.confidence_interval, None);
    assert_eq!(pet.not_observed_seeds, vec![0, 1, 2]);

    // Every distribution accounts for every seed exactly once.
    for distribution in aggregation.metrics.values() {
        assert_eq!(
            distribution.count
                + distribution.not_applicable_seeds.len()
                + distribution.not_observed_seeds.len(),
            3,
            "the statuses of every metric must cover every seed"
        );
    }
}

#[test]
fn a_single_reported_seed_has_no_interval() {
    let scratch = Scratch::new("single");
    let root = scratch.path("batch");
    let seeds = vec![
        (
            0,
            SyntheticSeed {
                separation: absent(MetricStatus::NotApplicable),
                ..SyntheticSeed::default()
            },
        ),
        (1, SyntheticSeed::default()),
        (
            2,
            SyntheticSeed {
                separation: absent(MetricStatus::NotApplicable),
                ..SyntheticSeed::default()
            },
        ),
    ];
    write_synthetic_batch(&root, &seeds);

    let aggregation = aggregate_batch(&root).expect("a well-formed batch aggregates");
    let separation = &aggregation.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 1);
    assert_close(separation.mean.expect("a mean"), 1.0);
    assert_close(separation.spread.minimum.expect("a least"), 1.0);
    assert_close(separation.spread.maximum.expect("a greatest"), 1.0);
    assert_close(separation.spread.range.expect("a range"), 0.0);
    assert_eq!(separation.spread.sample_variance, None);
    assert_eq!(separation.spread.sample_standard_deviation, None);
    assert_eq!(
        separation.confidence_interval, None,
        "one observation measures no spread and has no interval"
    );
    assert_eq!(separation.reported_seeds, vec![1]);
    assert_eq!(separation.not_applicable_seeds, vec![0, 2]);
}

#[test]
fn the_mode_pair_and_movement_slices_disaggregate_by_mode_and_movement() {
    let scratch = Scratch::new("slices");
    let root = scratch.path("batch");
    let bucket = || {
        movement_bucket(
            ["movement:west_to_north", "pedestrian_route:east_to_south"],
            reported(4.0),
            reported(0.5),
            absent(MetricStatus::NotObserved),
        )
    };
    let (bucket_key, bucket_minima) = bucket();
    let (_, later_minima) = movement_bucket(
        ["movement:west_to_north", "pedestrian_route:east_to_south"],
        reported(6.0),
        absent(MetricStatus::NotApplicable),
        reported(8.0),
    );
    let seeds = vec![
        (
            0,
            SyntheticSeed {
                mode_pairs: mode_pairs(
                    reported(4.0),
                    absent(MetricStatus::NotObserved),
                    absent(MetricStatus::NotObserved),
                ),
                movements: BTreeMap::from([(bucket_key.clone(), bucket_minima)]),
                ..SyntheticSeed::default()
            },
        ),
        (
            1,
            SyntheticSeed {
                mode_pairs: mode_pairs(
                    reported(6.0),
                    reported(2.0),
                    absent(MetricStatus::NotObserved),
                ),
                movements: BTreeMap::from([(bucket_key.clone(), later_minima)]),
                ..SyntheticSeed::default()
            },
        ),
        (
            2,
            SyntheticSeed {
                // This run observed no pair at all, so every mode slice is
                // explicit about making no observation.
                mode_pairs: mode_pairs(
                    absent(MetricStatus::NotObserved),
                    absent(MetricStatus::NotObserved),
                    absent(MetricStatus::NotObserved),
                ),
                ..SyntheticSeed::default()
            },
        ),
    ];
    write_synthetic_batch(&root, &seeds);

    let aggregation = aggregate_batch(&root).expect("a well-formed batch aggregates");

    // The mode slice: the separation minimum of each mode pair, the one metric
    // a run reports per mode pair.
    assert_eq!(
        aggregation.mode_pair_slices.keys().collect::<Vec<_>>(),
        vec![
            "pedestrian_pedestrian",
            "vehicle_pedestrian",
            "vehicle_vehicle"
        ]
    );
    for slice in aggregation.mode_pair_slices.values() {
        assert_eq!(
            slice.keys().collect::<Vec<_>>(),
            vec!["minimum_separation_m"]
        );
    }
    let vehicle_vehicle = &aggregation.mode_pair_slices["vehicle_vehicle"]["minimum_separation_m"];
    assert_eq!(vehicle_vehicle.count, 2);
    assert_close(vehicle_vehicle.mean.expect("a mean"), 5.0);
    assert_interval(
        vehicle_vehicle
            .confidence_interval
            .as_ref()
            .expect("two values have an interval"),
        &[4.0, 6.0],
    );
    assert_eq!(vehicle_vehicle.reported_seeds, vec![0, 1]);
    assert_eq!(vehicle_vehicle.not_observed_seeds, vec![2]);

    let vehicle_pedestrian =
        &aggregation.mode_pair_slices["vehicle_pedestrian"]["minimum_separation_m"];
    assert_eq!(vehicle_pedestrian.count, 1);
    assert_close(vehicle_pedestrian.mean.expect("a mean"), 2.0);
    assert_eq!(vehicle_pedestrian.not_observed_seeds, vec![0, 2]);

    // The movement slice: the bucket's metrics, keyed by its two movement keys.
    assert_eq!(
        aggregation.movement_slices.keys().collect::<Vec<_>>(),
        vec![bucket_key.as_str()]
    );
    let slice = &aggregation.movement_slices[&bucket_key];
    assert_eq!(
        slice.movement_keys,
        [
            "movement:west_to_north".to_owned(),
            "pedestrian_route:east_to_south".to_owned()
        ]
    );
    assert_eq!(
        slice.metrics.keys().collect::<Vec<_>>(),
        vec![
            "minimum_post_encroachment_s",
            "minimum_separation_m",
            "minimum_ttc_s"
        ]
    );

    let separation = &slice.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 2);
    assert_close(separation.mean.expect("a mean"), 5.0);
    assert_eq!(separation.reported_seeds, vec![0, 1]);
    // The third seed's run recorded no such bucket at all, which is no
    // observation, not a zero.
    assert_eq!(separation.not_observed_seeds, vec![2]);
    assert!(separation.not_applicable_seeds.is_empty());

    let ttc = &slice.metrics["minimum_ttc_s"];
    assert_eq!(ttc.count, 1);
    assert_close(ttc.mean.expect("a mean"), 0.5);
    assert_close(ttc.spread.maximum.expect("a greatest"), 0.5);
    assert_eq!(ttc.not_applicable_seeds, vec![1]);

    let pet = &slice.metrics["minimum_post_encroachment_s"];
    assert_eq!(pet.count, 1);
    assert_close(pet.mean.expect("a mean"), 8.0);
    assert_eq!(pet.not_observed_seeds, vec![0, 2]);

    // The slices account for every seed exactly once, like the metrics do.
    for slice in aggregation.mode_pair_slices.values() {
        for distribution in slice.values() {
            assert_eq!(
                distribution.count
                    + distribution.not_applicable_seeds.len()
                    + distribution.not_observed_seeds.len(),
                3
            );
        }
    }
    for slice in aggregation.movement_slices.values() {
        for distribution in slice.metrics.values() {
            assert_eq!(
                distribution.count
                    + distribution.not_applicable_seeds.len()
                    + distribution.not_observed_seeds.len(),
                3
            );
        }
    }
}

#[test]
fn the_event_slices_disaggregate_the_counted_families_by_mode_and_movement() {
    let scratch = Scratch::new("event-slices");
    let root = scratch.path("batch");
    let families = |counts: &[(&str, u64)]| -> BTreeMap<String, u64> {
        counts
            .iter()
            .map(|(family, count)| ((*family).to_owned(), *count))
            .collect()
    };
    let seeds = vec![
        (
            0,
            SyntheticSeed {
                event_families_by_mode: BTreeMap::from([(
                    "vehicle".to_owned(),
                    families(&[("collisions", 1), ("queue_events", 4)]),
                )]),
                event_families_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    families(&[("collisions", 1), ("yields", 2)]),
                )]),
                operational_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    operational_bucket(3.0, 2.0),
                )]),
                ..SyntheticSeed::default()
            },
        ),
        (
            1,
            SyntheticSeed {
                event_families_by_mode: BTreeMap::from([(
                    "vehicle".to_owned(),
                    families(&[("collisions", 3)]),
                )]),
                // Only the counted records reach this movement here: the run
                // carried no operational block for it, so its operational
                // metrics are unobserved rather than zero.
                event_families_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    families(&[("collisions", 3)]),
                )]),
                ..SyntheticSeed::default()
            },
        ),
        (
            2,
            SyntheticSeed {
                // This run placed no agent on east and carried west instead.
                operational_by_movement: BTreeMap::from([(
                    "movement:west".to_owned(),
                    operational_bucket(5.0, 1.0),
                )]),
                ..SyntheticSeed::default()
            },
        ),
    ];
    write_synthetic_batch(&root, &seeds);

    let aggregation = aggregate_batch(&root).expect("a well-formed batch aggregates");

    // The mode slices: both modes, every counted family present, and a family a
    // mode recorded nothing for is the observed zero the run artifact's own
    // `by_family` set reports — a count, not an absent value.
    assert_eq!(
        aggregation.mode_event_slices.keys().collect::<Vec<_>>(),
        vec!["pedestrian", "vehicle"]
    );
    let mut expected_keys: Vec<String> = FAMILIES
        .iter()
        .map(|family| format!("event_counts.{family}"))
        .collect();
    expected_keys.sort();
    for (mode, slice) in &aggregation.mode_event_slices {
        assert_eq!(
            slice.keys().collect::<Vec<_>>(),
            expected_keys.iter().collect::<Vec<_>>(),
            "mode '{mode}' carries every counted family"
        );
        for (key, distribution) in slice {
            assert_eq!(distribution.count, 3, "'{mode}/{key}' counts every seed");
            assert!(distribution.not_observed_seeds.is_empty());
            assert_eq!(distribution.unit, "records");
        }
    }
    let vehicle_collisions = &aggregation.mode_event_slices["vehicle"]["event_counts.collisions"];
    assert_close(vehicle_collisions.mean.expect("a mean"), 4.0 / 3.0);
    assert_eq!(vehicle_collisions.reported_seeds, vec![0, 1, 2]);
    assert_close(vehicle_collisions.spread.minimum.expect("a least"), 0.0);
    assert_close(vehicle_collisions.spread.maximum.expect("a greatest"), 3.0);
    assert_close(
        aggregation.mode_event_slices["vehicle"]["event_counts.queue_events"]
            .mean
            .expect("a mean"),
        4.0 / 3.0,
    );
    assert_close(
        aggregation.mode_event_slices["pedestrian"]["event_counts.collisions"]
            .mean
            .expect("a mean"),
        0.0,
    );

    // The agent movement slices: keyed by one movement key, holding that
    // movement's operational values and its counted families.
    assert_eq!(
        aggregation.agent_movement_slices.keys().collect::<Vec<_>>(),
        vec!["movement:east", "movement:west"]
    );
    let mut expected_movement_keys: Vec<String> =
        OPERATIONAL_FIELDS.map(|field| field.to_owned()).to_vec();
    expected_movement_keys.extend(
        FAMILIES
            .iter()
            .map(|family| format!("event_counts.{family}")),
    );
    expected_movement_keys.sort();
    let east = &aggregation.agent_movement_slices["movement:east"];
    assert_eq!(
        east.keys().collect::<Vec<_>>(),
        expected_movement_keys.iter().collect::<Vec<_>>()
    );
    let east_collisions = &east["event_counts.collisions"];
    assert_eq!(east_collisions.count, 2);
    assert_close(east_collisions.mean.expect("a mean"), 2.0);
    assert_eq!(east_collisions.reported_seeds, vec![0, 1]);
    assert_eq!(
        east_collisions.not_observed_seeds,
        vec![2],
        "a run with no agent on this movement made no observation of it"
    );
    let east_throughput = &east["throughput_agents_per_s"];
    assert_eq!(east_throughput.count, 1);
    assert_close(east_throughput.mean.expect("a mean"), 3.0);
    assert_eq!(east_throughput.reported_seeds, vec![0]);
    assert_eq!(east_throughput.not_observed_seeds, vec![1, 2]);
    assert_eq!(east_throughput.unit, "agents_per_second");
    assert_close(
        east["maximum_queue_length_agents"].mean.expect("a mean"),
        2.0,
    );
    assert_eq!(east["maximum_queue_length_agents"].unit, "agents");
    let west = &aggregation.agent_movement_slices["movement:west"];
    assert_eq!(west["event_counts.collisions"].count, 1);
    assert_close(west["event_counts.collisions"].mean.expect("a mean"), 0.0);
    assert_eq!(
        west["event_counts.collisions"].not_observed_seeds,
        vec![0, 1]
    );
    assert_close(west["throughput_agents_per_s"].mean.expect("a mean"), 5.0);

    // Every slice accounts for every seed exactly once.
    for slice in aggregation
        .mode_event_slices
        .values()
        .chain(aggregation.agent_movement_slices.values())
    {
        for (key, distribution) in slice {
            assert_statuses_account_for_every_seed(key, distribution, 3);
        }
    }
}

/// The close-pass families of metric definition v3 aggregate over the run and
/// per mode pair, per movement, and per facility, each with its unit and its
/// explicit applicability, in cells of their own; the batch refuses nothing.
#[test]
fn the_close_pass_families_aggregate_over_the_run_and_every_dimension() {
    let scratch = Scratch::new("close-pass");
    let root = scratch.path("batch");
    let bucket = "movement:through|movement:through";

    // Seed 0 recorded a closed pass on the shared road: one overtake from attempt
    // to completion, two closed observations whose least clearance is 0.4 m, a
    // violation, and a duration inside the declared `bicycle_close` band.
    let recorded = with_bands(
        close_pass_bucket([1, 1, 1, 0], 2, minimum_reported(0.4), 1),
        &[(7, reported(1.2)), (9, absent(MetricStatus::NotApplicable))],
    );
    let mut recorded_block = ClosePassMetrics::not_observed();
    recorded_block.run = recorded.clone();
    recorded_block
        .by_mode_pair
        .insert("vehicle_vehicle".to_owned(), recorded.clone());
    recorded_block.by_mode_pair.insert(
        "pedestrian_pedestrian".to_owned(),
        close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
    );
    recorded_block.by_movement = BTreeMap::from([(bucket.to_owned(), recorded.clone())]);
    recorded_block.by_facility = BTreeMap::from([
        ("facility:road".to_owned(), recorded),
        (
            "facility:centered".to_owned(),
            close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
        ),
    ]);

    // Seed 1 completed the same run without closing an observation: every
    // countable family is the observed `0` v3 fixes, the least clearance is not
    // observed because that bucket could host a pass and recorded none, and the
    // same declared bands are reported with their own applicabilities.
    let quiet = with_bands(
        close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotObserved), 0),
        &[
            (7, absent(MetricStatus::NotObserved)),
            (9, absent(MetricStatus::NotObserved)),
        ],
    );
    let mut quiet_block = ClosePassMetrics::not_observed();
    quiet_block.run = quiet.clone();
    quiet_block
        .by_mode_pair
        .insert("vehicle_vehicle".to_owned(), quiet.clone());
    // The applicability is a property of the scenario, not of the run: the pair
    // whose templates declare no passing tactic reports the same not-applicable
    // bucket in both seeds.
    quiet_block.by_mode_pair.insert(
        "pedestrian_pedestrian".to_owned(),
        close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
    );
    quiet_block.by_facility = BTreeMap::from([
        ("facility:road".to_owned(), quiet),
        (
            "facility:centered".to_owned(),
            close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
        ),
    ]);

    write_synthetic_batch(
        &root,
        &[
            (
                0,
                SyntheticSeed {
                    close_pass: recorded_block,
                    ..SyntheticSeed::default()
                },
            ),
            (
                1,
                SyntheticSeed {
                    close_pass: quiet_block,
                    ..SyntheticSeed::default()
                },
            ),
        ],
    );

    let aggregation =
        aggregate_batch(&root).expect("a batch whose runs recorded a closed pass aggregates");

    // The run bucket: the families are whole-batch metrics under the artifact
    // block they are read from, each in its own unit.
    assert_eq!(
        aggregation.metrics["close_pass.run.close_passes"].unit,
        "observations"
    );
    assert_eq!(
        aggregation.metrics["close_pass.run.overtake_attempts"].unit,
        "records"
    );
    assert_eq!(
        aggregation.metrics["close_pass.run.close_pass_minimum_clearance_m"].unit,
        "metres"
    );
    assert_eq!(
        aggregation.metrics["close_pass.run.clearance_band_durations_s.7"].unit,
        "seconds"
    );

    // A countable family is an observed zero in the run that recorded none, so
    // both seeds report it and the mean is over both.
    let close_passes = &aggregation.metrics["close_pass.run.close_passes"];
    assert_eq!(close_passes.count, 2);
    assert_close(close_passes.mean.expect("a mean"), 1.0);
    assert_eq!(close_passes.reported_seeds, vec![0, 1]);
    assert_close(
        aggregation.metrics["close_pass.run.overtake_completions"]
            .mean
            .expect("a mean"),
        0.5,
    );
    assert_close(
        aggregation.metrics["close_pass.run.close_pass_violations"]
            .mean
            .expect("a mean"),
        0.5,
    );

    // A value family keeps the run bucket's applicability: the least clearance was
    // reported by one seed and is not observed in the other, never a zero.
    let minimum = &aggregation.metrics["close_pass.run.close_pass_minimum_clearance_m"];
    assert_eq!(minimum.count, 1);
    assert_close(minimum.mean.expect("a mean"), 0.4);
    assert_eq!(minimum.reported_seeds, vec![0]);
    assert_eq!(minimum.not_observed_seeds, vec![1]);
    assert!(minimum.not_applicable_seeds.is_empty());

    // Each declared band is its own cell in its own unit, and a band the bucket's
    // mode gate excludes is not applicable rather than a zero duration.
    let band = &aggregation.metrics["close_pass.run.clearance_band_durations_s.7"];
    assert_eq!(band.count, 1);
    assert_close(band.mean.expect("a mean"), 1.2);
    assert_eq!(band.not_observed_seeds, vec![1]);
    let excluded = &aggregation.metrics["close_pass.run.clearance_band_durations_s.9"];
    assert_eq!(excluded.count, 0);
    assert_eq!(excluded.not_applicable_seeds, vec![0]);
    assert_eq!(excluded.not_observed_seeds, vec![1]);

    // The mode-pair cells: the pair that recorded the pass reports it, the pair
    // that cannot host one is not applicable, and the pair that could host one
    // and recorded none is not observed — three different statements.
    let pair = |label: &str, key: &str| &aggregation.close_pass_mode_pair_slices[label][key];
    assert_eq!(pair("vehicle_vehicle", "close_passes").unit, "observations");
    assert_close(
        pair("vehicle_vehicle", "close_passes")
            .mean
            .expect("a mean"),
        1.0,
    );
    assert_close(
        pair("vehicle_vehicle", "close_pass_minimum_clearance_m")
            .mean
            .expect("a mean"),
        0.4,
    );
    assert_eq!(
        pair("pedestrian_pedestrian", "close_pass_minimum_clearance_m").not_applicable_seeds,
        vec![0, 1]
    );
    assert_eq!(
        pair("vehicle_pedestrian", "close_pass_minimum_clearance_m").not_observed_seeds,
        vec![0, 1]
    );
    assert!(
        pair("vehicle_pedestrian", "close_pass_minimum_clearance_m")
            .not_applicable_seeds
            .is_empty()
    );
    // A countable family of a pair that recorded no pass is still an observed
    // zero, reported by both seeds.
    assert_eq!(pair("pedestrian_pedestrian", "close_passes").count, 2);
    assert_close(
        pair("pedestrian_pedestrian", "close_passes")
            .mean
            .expect("a mean"),
        0.0,
    );

    // The facility cells: the road that hosted the pass, and the centered
    // facility that cannot host one.
    let road = &aggregation.close_pass_facility_slices["facility:road"];
    assert_eq!(road["close_passes"].unit, "observations");
    assert_close(road["close_passes"].mean.expect("a mean"), 1.0);
    assert_eq!(
        aggregation.close_pass_facility_slices["facility:centered"]
            ["close_pass_minimum_clearance_m"]
            .not_applicable_seeds,
        vec![0, 1]
    );

    // The movement bucket is sparse: seed 1's run named none, so it made no
    // observation of that bucket rather than recording a zero family.
    let movement = &aggregation.close_pass_movement_slices[bucket];
    assert_eq!(movement["close_passes"].count, 1);
    assert_close(movement["close_passes"].mean.expect("a mean"), 2.0);
    assert_eq!(movement["close_passes"].not_observed_seeds, vec![1]);

    // No existing cell widened: the mode-pair and movement cells still hold
    // exactly the interaction metrics they held before the close-pass families.
    for slice in aggregation.mode_pair_slices.values() {
        assert_eq!(
            slice.keys().collect::<Vec<_>>(),
            vec!["minimum_separation_m"]
        );
    }
    for slice in aggregation.movement_slices.values() {
        assert_eq!(
            slice.metrics.keys().collect::<Vec<_>>(),
            vec![
                "minimum_post_encroachment_s",
                "minimum_separation_m",
                "minimum_ttc_s"
            ]
        );
    }

    // Every close-pass cell accounts for every seed exactly once, whichever
    // status that seed reported.
    for slice in aggregation
        .close_pass_mode_pair_slices
        .values()
        .chain(aggregation.close_pass_movement_slices.values())
        .chain(aggregation.close_pass_facility_slices.values())
    {
        for (key, distribution) in slice {
            assert_statuses_account_for_every_seed(key, distribution, 2);
        }
    }
}

#[test]
fn every_aggregated_metric_links_to_its_manifests_and_definition_version() {
    let scratch = Scratch::new("links");
    let root = scratch.path("batch");
    let (bucket_key, bucket_minima) = movement_bucket(
        ["movement:a", "movement:b"],
        reported(2.0),
        reported(0.5),
        reported(1.0),
    );
    let seeds = vec![
        (
            0,
            SyntheticSeed {
                movements: BTreeMap::from([(bucket_key.clone(), bucket_minima)]),
                ..SyntheticSeed::default()
            },
        ),
        (1, SyntheticSeed::default()),
    ];
    write_synthetic_batch(&root, &seeds);

    let aggregation = aggregate_batch(&root).expect("a well-formed batch aggregates");

    // The batch link is the exact bytes that were read.
    assert_eq!(aggregation.batch.path, BATCH_MANIFEST_FILE);
    assert_eq!(
        aggregation.batch.sha256,
        file_sha256(&root.join(BATCH_MANIFEST_FILE))
    );
    assert_eq!(aggregation.batch.batch_manifest_version, 1);

    // The seed links name the artifacts on disk.
    for seed in &aggregation.seeds {
        let run_dir = root.join(&seed.directory);
        assert_eq!(
            seed.manifest_sha256,
            file_sha256(&run_dir.join(MANIFEST_FILE))
        );
        assert_eq!(
            seed.metrics_sha256,
            file_sha256(&run_dir.join(METRICS_FILE))
        );
    }
    let manifests: BTreeMap<u64, &str> = aggregation
        .seeds
        .iter()
        .map(|seed| (seed.seed, seed.manifest_sha256.as_str()))
        .collect();

    // Every aggregated metric carries the definition version and links each
    // reported seed to that seed's manifest.
    // The mode-and-movement event slices make the same link: a synthetic batch
    // always reports both modes, so the link walk covers them.
    assert!(!aggregation.mode_event_slices.is_empty());

    let distributions = aggregation
        .metrics
        .values()
        .chain(
            aggregation
                .mode_pair_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            aggregation
                .movement_slices
                .values()
                .flat_map(|slice| slice.metrics.values()),
        )
        .chain(
            aggregation
                .mode_event_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            aggregation
                .agent_movement_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            aggregation
                .close_pass_mode_pair_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            aggregation
                .close_pass_movement_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            aggregation
                .close_pass_facility_slices
                .values()
                .flat_map(|slice| slice.values()),
        );
    let mut checked = 0;
    for distribution in distributions {
        assert_eq!(
            distribution.metric_definition_version,
            METRIC_DEFINITION_VERSION
        );
        assert!(!distribution.unit.is_empty());
        assert_eq!(
            distribution.manifests.len(),
            distribution.reported_seeds.len()
        );
        assert_eq!(distribution.count, distribution.reported_seeds.len());
        for (seed, manifest) in distribution
            .reported_seeds
            .iter()
            .zip(&distribution.manifests)
        {
            assert_eq!(
                Some(manifest.as_str()),
                manifests.get(seed).copied(),
                "seed {seed}'s reported value must link to that seed's manifest"
            );
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} distributions were checked");
}

#[test]
fn a_real_batch_aggregates_its_mode_and_movement_slices() {
    let scratch = Scratch::new("mixed");
    let (scenario, provenance) =
        load_scenario_provenance(&repo_path(MIXED)).expect("the mixed benchmark loads");
    let root = scratch.path("batch");
    run_batch(BatchRequest {
        root: root.clone(),
        scenario,
        provenance,
        ticks: MIXED_TICKS,
        step_s: RunConfig::new(0).step().as_secs(),
        sampling: SamplingPolicy::default(),
        seeds: vec![0, 1, 2],
        seed_bank: None,
        jobs: 1,
    })
    .expect("the batch runs");

    let aggregation = aggregate_batch(&root).expect("the batch aggregates");

    // Every metric the runs report is aggregated, and nothing else is.
    let artifact: RunMetricsArtifact = serde_json::from_str(
        &std::fs::read_to_string(root.join("seed-0").join(METRICS_FILE)).expect("readable"),
    )
    .expect("the run metrics are JSON");
    let expected = expected_metric_keys(&artifact);
    assert_eq!(
        aggregation.metrics.keys().collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );

    for (key, distribution) in &aggregation.metrics {
        assert_statuses_account_for_every_seed(key, distribution, 3);
        assert_eq!(
            distribution.confidence_interval.is_some(),
            distribution.count >= LEAST_INTERVAL_SEEDS as usize,
            "'{key}' has an interval exactly when two or more seeds report"
        );
    }

    // The benchmark is a crossing one: some seed reports a time to collision,
    // and the seeds that do not are counted behind the status, not zeroed.
    let ttc = &aggregation.metrics["minimum_ttc_s"];
    assert!(
        ttc.count >= 1,
        "the mixed benchmark must report a time to collision in a seed"
    );
    assert!(ttc.mean.expect("a mean") > 0.0);
    assert!(ttc.not_observed_seeds.is_empty());

    // The close-pass families of a real artifact are aggregated like any other:
    // the benchmark closes no observation, and an absent count is the observed
    // zero metric definition v3 fixes rather than an absent value.
    let close_passes = &aggregation.metrics["close_pass.run.close_passes"];
    assert_eq!(close_passes.unit, "observations");
    assert_eq!(close_passes.count, 3);
    assert_eq!(close_passes.mean, Some(0.0));

    // The mode slice: all three mode pairs, each reporting the separation
    // minimum, each accounting for every seed.
    assert_eq!(
        aggregation.mode_pair_slices.keys().collect::<Vec<_>>(),
        vec![
            "pedestrian_pedestrian",
            "vehicle_pedestrian",
            "vehicle_vehicle"
        ]
    );
    for (label, slice) in &aggregation.mode_pair_slices {
        assert_eq!(
            slice.keys().collect::<Vec<_>>(),
            vec!["minimum_separation_m"]
        );
        let distribution = &slice["minimum_separation_m"];
        assert_eq!(distribution.unit, "metres");
        assert_statuses_account_for_every_seed(label, distribution, 3);
    }

    // The movement slice: every bucket keyed by two sorted movement keys, each
    // holding the three interaction metrics, each accounting for every seed.
    assert!(!aggregation.movement_slices.is_empty());
    let mut reported = false;
    for (bucket, slice) in &aggregation.movement_slices {
        assert!(slice.movement_keys[0] <= slice.movement_keys[1]);
        assert_eq!(
            format!("{}|{}", slice.movement_keys[0], slice.movement_keys[1]),
            *bucket
        );
        assert_eq!(
            slice.metrics.keys().collect::<Vec<_>>(),
            vec![
                "minimum_post_encroachment_s",
                "minimum_separation_m",
                "minimum_ttc_s"
            ]
        );
        for (key, distribution) in &slice.metrics {
            assert_statuses_account_for_every_seed(key, distribution, 3);
            reported |= distribution.count > 0;
        }
    }
    assert!(reported, "a movement bucket must report a value");

    // The mode event slice: both modes, every counted family, and the families
    // one mode recorded no record of reported as an observed zero rather than
    // as an absent value.
    let manifests: BTreeMap<u64, &str> = aggregation
        .seeds
        .iter()
        .map(|seed| (seed.seed, seed.manifest_sha256.as_str()))
        .collect();
    assert_eq!(
        aggregation.mode_event_slices.keys().collect::<Vec<_>>(),
        vec!["pedestrian", "vehicle"]
    );
    let mut expected_families: Vec<String> = EVENT_FAMILY_LABELS
        .iter()
        .map(|family| format!("event_counts.{family}"))
        .collect();
    expected_families.sort();
    for (mode, slice) in &aggregation.mode_event_slices {
        assert_eq!(
            slice.keys().collect::<Vec<_>>(),
            expected_families.iter().collect::<Vec<_>>()
        );
        for (key, distribution) in slice {
            assert_eq!(distribution.unit, "records");
            assert_eq!(distribution.count, 3, "'{mode}/{key}'");
            for (seed, manifest) in distribution
                .reported_seeds
                .iter()
                .zip(&distribution.manifests)
            {
                assert_eq!(Some(manifest.as_str()), manifests.get(seed).copied());
            }
        }
    }

    // Every record a mode's own agents produced adds to the run-level family
    // count, so the mode slices partition the run-level count.
    for family in EVENT_FAMILY_LABELS {
        let key = format!("event_counts.{family}");
        let run_level = &aggregation.metrics[&format!("event_counts.by_family.{family}")];
        let mode_total: f64 = ["pedestrian", "vehicle"]
            .into_iter()
            .map(|mode| {
                aggregation.mode_event_slices[mode][&key]
                    .mean
                    .expect("a mode count is always reported")
            })
            .sum();
        assert!(
            (mode_total - run_level.mean.expect("a run-level count")).abs() < 1e-9,
            "the mode slices of '{family}' must sum to its run-level count: {mode_total} against {:?}",
            run_level.mean
        );
    }

    // The agent movement slice: every movement the runs observed, holding the
    // ten operational metrics and every counted family, each accounting for
    // every seed. The benchmark drives several movements, so the slice is not
    // empty.
    assert!(!aggregation.agent_movement_slices.is_empty());
    let mut movement_keys: Vec<String> = OPERATIONAL_FIELDS.map(|field| field.to_owned()).to_vec();
    movement_keys.extend(expected_families.iter().cloned());
    movement_keys.sort();
    for (movement, slice) in &aggregation.agent_movement_slices {
        assert!(movement.starts_with("movement:") || movement.starts_with("pedestrian_route:"));
        assert_eq!(
            slice.keys().collect::<Vec<_>>(),
            movement_keys.iter().collect::<Vec<_>>()
        );
        for (key, distribution) in slice {
            assert_statuses_account_for_every_seed(key, distribution, 3);
        }
    }
}

#[test]
fn the_output_ordering_is_deterministic() {
    let scratch = Scratch::new("ordering");
    let ordered = scratch.path("ordered");
    let unordered = scratch.path("unordered");
    let seeds: Vec<(u64, SyntheticSeed)> = [3u64, 0, 2, 1]
        .into_iter()
        .map(|seed| {
            let (bucket_key, bucket_minima) = movement_bucket(
                ["movement:a", "movement:b"],
                reported(seed as f64 + 1.0),
                reported(0.5),
                absent(MetricStatus::NotObserved),
            );
            (
                seed,
                SyntheticSeed {
                    separation: reported(seed as f64 + 1.0),
                    collisions: seed,
                    movements: BTreeMap::from([(bucket_key, bucket_minima)]),
                    ..SyntheticSeed::default()
                },
            )
        })
        .collect();
    write_synthetic_batch(&ordered, &seeds);
    // The same four seeds, with the batch manifest naming them out of order.
    let mut reversed = seeds.clone();
    reversed.reverse();
    write_synthetic_batch(&unordered, &reversed);

    let first = aggregate_batch(&ordered).expect("the batch aggregates");
    let second = aggregate_batch(&unordered).expect("the batch aggregates");
    // The two batch manifests hold the same runs in different orders, so they
    // are different bytes and link differently; everything the aggregation
    // derives from them must be identical.
    assert_eq!(
        first.batch.sha256,
        file_sha256(&ordered.join(BATCH_MANIFEST_FILE))
    );
    assert_eq!(
        second.batch.sha256,
        file_sha256(&unordered.join(BATCH_MANIFEST_FILE))
    );
    assert_ne!(first.batch.sha256, second.batch.sha256);
    let without_batch_link = |mut aggregation: Aggregation| {
        aggregation.batch.sha256 = "batch".to_owned();
        aggregation
    };
    assert_eq!(
        without_batch_link(first.clone()),
        without_batch_link(second),
        "the aggregation must not depend on the order the batch names its runs"
    );

    // The ordering contract: metrics, slices, and seeds ascending, and every
    // distribution's seed lists ascending.
    assert_eq!(
        first.seeds.iter().map(|seed| seed.seed).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    let ascending = |keys: Vec<&String>| {
        assert!(
            keys.windows(2).all(|pair| pair[0] < pair[1]),
            "keys are not ascending: {keys:?}"
        );
    };
    ascending(first.metrics.keys().collect());
    ascending(first.mode_pair_slices.keys().collect());
    ascending(first.mode_event_slices.keys().collect());
    ascending(first.movement_slices.keys().collect());
    ascending(first.agent_movement_slices.keys().collect());
    ascending(first.close_pass_mode_pair_slices.keys().collect());
    ascending(first.close_pass_movement_slices.keys().collect());
    ascending(first.close_pass_facility_slices.keys().collect());
    for slice in first.mode_pair_slices.values() {
        ascending(slice.keys().collect());
    }
    for slice in first.mode_event_slices.values() {
        ascending(slice.keys().collect());
    }
    for slice in first.movement_slices.values() {
        ascending(slice.metrics.keys().collect());
    }
    for slice in first.agent_movement_slices.values() {
        ascending(slice.keys().collect());
    }
    for slice in first
        .close_pass_mode_pair_slices
        .values()
        .chain(first.close_pass_movement_slices.values())
        .chain(first.close_pass_facility_slices.values())
    {
        ascending(slice.keys().collect());
    }
    for distribution in first
        .metrics
        .values()
        .chain(
            first
                .mode_pair_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            first
                .movement_slices
                .values()
                .flat_map(|slice| slice.metrics.values()),
        )
        .chain(
            first
                .mode_event_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            first
                .agent_movement_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            first
                .close_pass_mode_pair_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            first
                .close_pass_movement_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
        .chain(
            first
                .close_pass_facility_slices
                .values()
                .flat_map(|slice| slice.values()),
        )
    {
        for seeds in [
            &distribution.reported_seeds,
            &distribution.not_applicable_seeds,
            &distribution.not_observed_seeds,
        ] {
            assert!(seeds.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    // The command writes exactly the bytes the library computes, so the file
    // ordering is pinned as well.
    let output = aggregate_command(&scratch, "ordered", None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let written = std::fs::read_to_string(ordered.join(AGGREGATION_FILE))
        .expect("the aggregation is written");
    let mut expected = serde_json::to_string_pretty(&first).expect("the aggregation serializes");
    expected.push('\n');
    assert_eq!(written, expected);
}

#[test]
fn a_run_that_disagrees_with_the_batch_is_refused() {
    let scratch = Scratch::new("refused");

    // A metrics artifact that links to a manifest the batch did not record.
    let linked = scratch.path("linked");
    write_synthetic_batch(&linked, &[(0, SyntheticSeed::default())]);
    let path = linked.join("seed-0").join(METRICS_FILE);
    let mut artifact: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("JSON");
    artifact["manifest_sha256"] = serde_json::Value::String("f".repeat(64));
    write_json(&path, &artifact);
    assert!(matches!(
        aggregate_batch(&linked),
        Err(AggregateError::ManifestLink { seed: 0, .. })
    ));

    // A metrics artifact from another definition revision.
    let revision = scratch.path("revision");
    write_synthetic_batch(&revision, &[(0, SyntheticSeed::default())]);
    let path = revision.join("seed-0").join(METRICS_FILE);
    let mut artifact: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("JSON");
    artifact["metric_definition_version"] = serde_json::Value::from(METRIC_DEFINITION_VERSION + 1);
    write_json(&path, &artifact);
    assert!(matches!(
        aggregate_batch(&revision),
        Err(AggregateError::DefinitionVersion {
            version,
            expected: METRIC_DEFINITION_VERSION,
            ..
        }) if version == METRIC_DEFINITION_VERSION + 1
    ));

    // A run manifest that changed after the batch recorded it.
    let changed = scratch.path("changed");
    write_synthetic_batch(&changed, &[(0, SyntheticSeed::default())]);
    let path = changed.join("seed-0").join(MANIFEST_FILE);
    let mut bytes = std::fs::read(&path).expect("readable");
    bytes.extend_from_slice(b" \n");
    std::fs::write(&path, bytes).expect("the manifest is rewritten");
    assert!(matches!(
        aggregate_batch(&changed),
        Err(AggregateError::ManifestDigest { seed: 0, .. })
    ));

    // A metric one seed reports that another does not.
    let missing = scratch.path("missing");
    write_synthetic_batch(
        &missing,
        &[(0, SyntheticSeed::default()), (1, SyntheticSeed::default())],
    );
    let path = missing.join("seed-1").join(METRICS_FILE);
    let mut artifact: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("JSON");
    artifact["event_counts"]["by_family"]
        .as_object_mut()
        .expect("a family map")
        .remove("collisions");
    write_json(&path, &artifact);
    assert!(matches!(
        aggregate_batch(&missing),
        Err(AggregateError::MissingMetric { seed: 1, .. })
    ));

    // A batch root with no batch manifest at all.
    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).expect("the directory is created");
    assert!(matches!(
        aggregate_batch(&empty),
        Err(AggregateError::Io { .. })
    ));
}

/// Run the `aggregate` command over `root` inside `scratch`, optionally naming
/// an output path.
fn aggregate_command(scratch: &Scratch, root: &str, output: Option<&str>) -> Output {
    let mut command = Command::new(CLI);
    command.arg("aggregate").arg(root).current_dir(&scratch.dir);
    if let Some(output) = output {
        command.args(["--output", output]);
    }
    command.output().expect("hekate-cli runs")
}

#[test]
fn the_aggregate_command_writes_the_aggregation_and_mutates_no_run_artifact() {
    let scratch = Scratch::new("command");
    let (scenario, provenance) =
        load_scenario_provenance(&repo_path(WALKING)).expect("the walking scenario loads");
    let root = scratch.path("batch");
    run_batch(BatchRequest {
        root: root.clone(),
        scenario,
        provenance,
        ticks: WIRING_TICKS,
        step_s: RunConfig::new(0).step().as_secs(),
        sampling: SamplingPolicy::default(),
        seeds: vec![0, 1, 2],
        seed_bank: None,
        jobs: 1,
    })
    .expect("the batch runs");

    // Snapshot every run artifact before the aggregation.
    let before = tree_hashes(&root);
    let batch_json = std::fs::read(root.join(BATCH_MANIFEST_FILE)).expect("readable");

    let output = aggregate_command(&scratch, "batch", None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let aggregation: Aggregation = serde_json::from_str(
        &std::fs::read_to_string(root.join(AGGREGATION_FILE)).expect("the aggregation is written"),
    )
    .expect("the aggregation is JSON");
    assert_eq!(aggregation.aggregation_version, AGGREGATION_VERSION);
    assert_eq!(
        aggregation.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(aggregation.seeds.len(), 3);
    assert_eq!(
        aggregation.batch.sha256,
        sha256_hex(&batch_json),
        "the aggregation links to the batch manifest bytes"
    );

    // The walking run has no crossing pair to close, so its time to collision is
    // not applicable in every seed: no mean, no interval, and three seeds
    // counted behind the status — never a zero.
    let ttc = &aggregation.metrics["minimum_ttc_s"];
    assert_eq!(ttc.count, 0);
    assert_eq!(ttc.mean, None);
    assert_eq!(ttc.confidence_interval, None);
    assert_eq!(ttc.not_applicable_seeds, vec![0, 1, 2]);
    assert_eq!(ttc.manifests, Vec::<String>::new());
    let spawns = &aggregation.metrics["event_counts.by_family.spawns"];
    assert_eq!(spawns.count, 3);
    assert!(spawns.mean.expect("a mean") > 0.0);

    // No run artifact changed: the same paths still hold the same bytes, and the
    // batch manifest is byte-identical.
    let after = tree_hashes(&root);
    for (name, hash) in &before {
        assert!(
            after.contains(&(name.clone(), hash.clone())),
            "the aggregation changed '{name}'"
        );
    }
    assert_eq!(
        std::fs::read(root.join(BATCH_MANIFEST_FILE)).expect("readable"),
        batch_json
    );
    assert_eq!(
        after.len(),
        before.len() + 1,
        "the aggregation writes exactly one file and nothing else"
    );

    // Aggregating again is idempotent, and `--output -` writes no file at all.
    let first = std::fs::read(root.join(AGGREGATION_FILE)).expect("readable");
    let again = aggregate_command(&scratch, "batch", None);
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(
        std::fs::read(root.join(AGGREGATION_FILE)).expect("readable"),
        first
    );
    let stdout_run = aggregate_command(&scratch, "batch", Some("-"));
    assert_eq!(stdout_run.status.code(), Some(0));
    let printed: Aggregation =
        serde_json::from_slice(&stdout_run.stdout).expect("stdout is the aggregation JSON");
    assert_eq!(printed, aggregation);
    assert_eq!(tree_hashes(&root).len(), before.len() + 1);
}
