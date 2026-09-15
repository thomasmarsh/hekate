//! Contract tests for the `compare` command.
//!
//! A comparison is the paired A/B record the common-random-number seed bank
//! exists for: two batches that ran one bank have their runs paired by seed, and
//! for every reported metric the per-seed difference `d_i = A_i - B_i` with its
//! mean and a paired confidence interval is written to `comparison.json`. These
//! tests pin the paired statistic against hand-computed values, show that
//! pairing removes the between-seed variance both sides share, cover the mode
//! and movement disaggregation, the manifest and definition-version links, the
//! deterministic ordering, every refusal case, and the command's read-only
//! treatment of both batch roots.
//!
//! Most tests build two synthetic batches with hand-written `metrics.json`
//! files, because only known values can be asserted exactly; the last test runs
//! two real batches over one bank through the binary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use hekate_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, BatchRun, BatchSpec, COMPARISON_FILE, COMPARISON_VERSION,
    CONFIDENCE_LEVEL, ClosePassMetrics, ClosePassMinimum, ClosePassValues, CompareError,
    ComparedPair, Comparison, EventCounts, MANIFEST_FILE, METRIC_DEFINITION_VERSION, METRICS_FILE,
    MetricStatus, MetricValue, MovementMinima, OperationalMetrics, OperationalValues,
    PAIRED_DIFFERENCE, PAIRED_INTERVAL_METHOD, PairedDistribution, RunMetricsArtifact,
    SamplingPolicy, ScenarioProvenance, SeedBank, SeedBankReference, Side, T_CRITICAL_975,
    WrongWayMetrics, compare_batches, read_seed_bank,
};
use sha2::{Digest, Sha256};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

/// Side A of the real comparison: the walking-skeleton scenario.
const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
/// Side B of the real comparison: another scenario, so the two sides exercise
/// different movement buckets and a sparse slice.
const CAR_FOLLOWING: &str = "scenarios/benchmarks/car_following_v1.json5";
/// Few enough ticks that a two-seed pair of batches stays fast.
const TICKS: u64 = 60;

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

/// The operational metrics metric definition v2 reports for one bucket, by
/// artifact field name. Restated here so the comparison's metric set is pinned
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

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("hekate-cli-compare-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Run the CLI in the scratch directory with the given arguments.
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(CLI)
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("hekate-cli runs")
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

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
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

/// The pretty JSON the crate's writers put on disk.
fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    let mut json = serde_json::to_string_pretty(value).expect("artifact serializes");
    json.push('\n');
    std::fs::write(path, json)
        .unwrap_or_else(|error| panic!("cannot write '{}': {error}", path.display()));
}

/// Write a seed bank file and return its path and the reference a batch records.
fn write_bank(scratch: &Scratch, name: &str, seeds: &[u64]) -> (PathBuf, SeedBankReference) {
    let path = scratch.path(name);
    let bank = SeedBank::from_seeds(seeds).expect("the bank builds");
    write_json(&path, &bank);
    let loaded = read_seed_bank(&path).expect("the bank reads");
    let reference = SeedBankReference {
        path: path.display().to_string(),
        content_sha256: loaded.content_sha256,
    };
    (path, reference)
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

/// The mode-pair entries a synthetic run artifact carries, from the three
/// values a test supplies.
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

/// One movement bucket: its sorted two-key bucket key and its three minima.
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

/// Event counts with every family present and `collisions` set; `drop` removes
/// one family, which makes that metric one-sided against the other batch.
///
/// `by_mode` and `by_movement` are keyed by the mode or movement the caller
/// names, and are transposed into the run artifact's own shape, whose outer key
/// is the family.
fn event_counts(
    collisions: u64,
    drop: Option<&str>,
    by_mode: &BTreeMap<String, BTreeMap<String, u64>>,
    by_movement: &BTreeMap<String, BTreeMap<String, u64>>,
) -> EventCounts {
    let mut by_family: BTreeMap<String, u64> = FAMILIES
        .into_iter()
        .filter(|family| Some(*family) != drop)
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

/// The close-pass block of one run: the run bucket the test names, the three
/// mode pairs, and the movement and facility buckets it records.
fn close_pass_block(run: ClosePassValues, movement: &str, facility: &str) -> ClosePassMetrics {
    let mut block = ClosePassMetrics::not_observed();
    block.run = run.clone();
    block
        .by_mode_pair
        .insert("vehicle_vehicle".to_owned(), run.clone());
    // The pedestrian pair's templates declare no passing tactic in either seed,
    // so the pair cannot host a pass: its bucket is not applicable rather than a
    // zero clearance.
    block.by_mode_pair.insert(
        "pedestrian_pedestrian".to_owned(),
        close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
    );
    block.by_movement = BTreeMap::from([(movement.to_owned(), run.clone())]);
    block.by_facility = BTreeMap::from([
        (facility.to_owned(), run),
        (
            "facility:centered".to_owned(),
            close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
        ),
    ]);
    block
}

/// The close-pass block of a run that recorded no observation: every countable
/// family is the observed zero, and every value family carries the bucket's own
/// applicability, which is a property of the scenario rather than of the run.
fn close_pass_quiet() -> ClosePassMetrics {
    let mut block = ClosePassMetrics::not_observed();
    block.by_mode_pair.insert(
        "pedestrian_pedestrian".to_owned(),
        close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
    );
    block.by_facility = BTreeMap::from([(
        "facility:centered".to_owned(),
        close_pass_bucket([0; 4], 0, minimum_absent(MetricStatus::NotApplicable), 0),
    )]);
    block
}

/// The synthetic seeds for a list of separation values, seeds `0..n`.
fn separations(values: &[f64]) -> Vec<(u64, SyntheticSeed)> {
    values
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
        .collect()
}

/// How a synthetic side deviates from a well-formed batch.
#[derive(Default, Clone)]
struct Deviation {
    /// Remove this seed's run entry, keeping its run directory on disk.
    drop_run: Option<u64>,
    /// Add a run entry whose seed the manifest's own seed list does not hold.
    stray_seed: Option<u64>,
    /// Repeat this seed's run entry.
    duplicate_seed: Option<u64>,
    /// Record this bank content hash instead of the bank's own.
    seed_bank_sha256: Option<String>,
    /// Record no seed bank at all.
    without_bank: bool,
    /// Record this seed list instead of the bank's ordered seeds.
    seeds: Option<Vec<u64>>,
    /// Drop this event family from every metrics artifact.
    drop_family: Option<String>,
    /// Write the manifest's runs in descending seed order.
    reverse_runs: bool,
}

/// Write one side's synthetic batch root: a `batch.json` naming `bank` and one
/// run directory per given seed, each with a stand-in `manifest.json` and a
/// `metrics.json` built from the given values.
fn write_side(
    root: &Path,
    bank: &SeedBankReference,
    bank_seeds: &[u64],
    seeds: &[(u64, SyntheticSeed)],
    deviation: &Deviation,
) {
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
                    deviation.drop_family.as_deref(),
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
                wrong_way: WrongWayMetrics::not_observed(),
            },
        );
        runs.push(BatchRun {
            seed: *seed,
            directory,
            trace_sha256: "0".repeat(64),
            manifest_sha256,
        });
    }

    if let Some(seed) = deviation.duplicate_seed {
        let repeated = runs
            .iter()
            .find(|run| run.seed == seed)
            .expect("the caller repeats a written seed")
            .clone();
        runs.push(repeated);
    }
    if let Some(seed) = deviation.drop_run {
        runs.retain(|run| run.seed != seed);
    }
    if let Some(seed) = deviation.stray_seed {
        runs.push(BatchRun {
            seed,
            directory: format!("seed-{seed}"),
            trace_sha256: "0".repeat(64),
            manifest_sha256: "0".repeat(64),
        });
    }
    if deviation.reverse_runs {
        runs.reverse();
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
            seed_bank: (!deviation.without_bank).then(|| SeedBankReference {
                path: bank.path.clone(),
                content_sha256: deviation
                    .seed_bank_sha256
                    .clone()
                    .unwrap_or_else(|| bank.content_sha256.clone()),
            }),
            seeds: deviation
                .seeds
                .clone()
                .unwrap_or_else(|| bank_seeds.to_vec()),
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

/// The standard error of the mean of `values`, as an unpaired per-side interval
/// computes it: the `n - 1` sample standard deviation over the square root of
/// the count.
fn unpaired_standard_error(values: &[f64]) -> f64 {
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    variance.sqrt() / count.sqrt()
}

/// Assert a paired interval built by hand from the per-seed differences.
fn assert_paired_interval(interval: &hekate_cli::ConfidenceInterval, differences: &[f64]) {
    let count = differences.len();
    let se = unpaired_standard_error(differences);
    let mean = differences.iter().sum::<f64>() / count as f64;
    let critical_value = T_CRITICAL_975[count - 2];
    assert_eq!(interval.method, PAIRED_INTERVAL_METHOD);
    assert_eq!(interval.confidence_level, CONFIDENCE_LEVEL);
    assert_eq!(interval.degrees_of_freedom as usize, count - 1);
    assert_close(interval.critical_value, critical_value);
    assert_close(interval.standard_error, se);
    assert_close(interval.half_width, critical_value * se);
    assert_close(interval.lower, mean - critical_value * se);
    assert_close(interval.upper, mean + critical_value * se);
}

/// Every paired distribution in the artifact, with the key path it lives under.
fn every_distribution(comparison: &Comparison) -> Vec<(String, &PairedDistribution)> {
    let mut all = Vec::new();
    for (key, distribution) in &comparison.metrics {
        all.push((format!("metrics.{key}"), distribution));
    }
    for (label, slice) in &comparison.mode_pair_slices {
        for (key, distribution) in slice {
            all.push((format!("mode_pair_slices.{label}.{key}"), distribution));
        }
    }
    for (mode, slice) in &comparison.mode_event_slices {
        for (key, distribution) in slice {
            all.push((format!("mode_event_slices.{mode}.{key}"), distribution));
        }
    }
    for (bucket, slice) in &comparison.movement_slices {
        for (key, distribution) in &slice.metrics {
            all.push((format!("movement_slices.{bucket}.{key}"), distribution));
        }
    }
    for (movement, slice) in &comparison.agent_movement_slices {
        for (key, distribution) in slice {
            all.push((
                format!("agent_movement_slices.{movement}.{key}"),
                distribution,
            ));
        }
    }
    for (dimension, slices) in [
        (
            "close_pass_mode_pair_slices",
            &comparison.close_pass_mode_pair_slices,
        ),
        (
            "close_pass_movement_slices",
            &comparison.close_pass_movement_slices,
        ),
        (
            "close_pass_facility_slices",
            &comparison.close_pass_facility_slices,
        ),
    ] {
        for (bucket, slice) in slices {
            for (key, distribution) in slice {
                all.push((format!("{dimension}.{bucket}.{key}"), distribution));
            }
        }
    }
    all
}

/// The pair table keyed by seed, for resolving a distribution's links.
fn pairs_by_seed(comparison: &Comparison) -> BTreeMap<u64, &ComparedPair> {
    comparison
        .pairs
        .iter()
        .map(|pair| (pair.seed, pair))
        .collect()
}

/// Write one pair of synthetic batches over one bank and return every path the
/// test needs.
fn synthetic_pair(
    scratch: &Scratch,
    name: &str,
    bank_seeds: &[u64],
    a_seeds: &[(u64, SyntheticSeed)],
    b_seeds: &[(u64, SyntheticSeed)],
) -> (PathBuf, PathBuf, PathBuf, SeedBankReference) {
    let (bank_path, bank) = write_bank(scratch, &format!("{name}-bank.json"), bank_seeds);
    let a_root = scratch.path(&format!("{name}-a"));
    let b_root = scratch.path(&format!("{name}-b"));
    write_side(&a_root, &bank, bank_seeds, a_seeds, &Deviation::default());
    write_side(&b_root, &bank, bank_seeds, b_seeds, &Deviation::default());
    (a_root, b_root, bank_path, bank)
}

/// The hand-computed paired mean difference, spread, and interval, the links to
/// both sides' manifests, and the documented method block.
#[test]
fn hand_computed_paired_mean_difference_and_interval() {
    let scratch = Scratch::new("hand-computed");
    let a = separations(&[1.0, 2.0, 3.0, 4.0]);
    // Side B counts no collision, so the paired count difference is A's count.
    let b: Vec<(u64, SyntheticSeed)> = [0.5, 1.0, 2.5, 4.0]
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                index as u64,
                SyntheticSeed {
                    separation: reported(*value),
                    ..SyntheticSeed::default()
                },
            )
        })
        .collect();
    let (a_root, b_root, bank_path, bank) = synthetic_pair(&scratch, "hand", &[0, 1, 2, 3], &a, &b);

    let comparison =
        compare_batches(&a_root, &b_root, &bank_path).expect("two batches over one bank compare");
    assert_eq!(comparison.comparison_version, COMPARISON_VERSION);
    assert_eq!(
        comparison.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );

    // The documented method: a Student-t interval on the paired differences.
    assert_eq!(
        comparison.method.confidence_interval_method,
        PAIRED_INTERVAL_METHOD
    );
    assert_eq!(comparison.method.confidence_level, CONFIDENCE_LEVEL);
    assert_eq!(comparison.method.variance_denominator, "n - 1");
    assert_eq!(comparison.method.paired_difference, PAIRED_DIFFERENCE);
    assert_eq!(comparison.method.smallest_tabulated_degrees_of_freedom, 1);
    assert_eq!(comparison.method.largest_tabulated_degrees_of_freedom, 30);
    assert_eq!(comparison.method.least_interval_seeds, 2);

    // One bank, proven: the file hashes to what both batches recorded.
    assert_eq!(comparison.seed_bank.path, bank_path.display().to_string());
    assert_eq!(comparison.seed_bank.content_sha256, bank.content_sha256);
    assert_eq!(comparison.a.seed_bank_sha256, bank.content_sha256);
    assert_eq!(comparison.b.seed_bank_sha256, bank.content_sha256);
    assert_eq!(comparison.a.seed_bank_path, bank.path);
    assert_eq!(comparison.b.seed_bank_path, bank.path);

    // Both batch manifests, linked by hash.
    assert_eq!(
        comparison.a.sha256,
        file_sha256(&a_root.join(BATCH_MANIFEST_FILE))
    );
    assert_eq!(
        comparison.b.sha256,
        file_sha256(&b_root.join(BATCH_MANIFEST_FILE))
    );
    assert_eq!(comparison.a.batch_manifest_version, 1);
    assert_eq!(comparison.b.batch_manifest_version, 1);

    // The pairs follow the bank's order and name both sides' run artifacts.
    assert_eq!(
        comparison
            .pairs
            .iter()
            .map(|pair| pair.seed)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    for pair in &comparison.pairs {
        assert_eq!(pair.a.directory, format!("seed-{}", pair.seed));
        assert_eq!(pair.b.directory, format!("seed-{}", pair.seed));
        assert_eq!(
            pair.a.metrics_sha256,
            file_sha256(&a_root.join(&pair.a.directory).join(METRICS_FILE))
        );
        assert_eq!(
            pair.b.metrics_sha256,
            file_sha256(&b_root.join(&pair.b.directory).join(METRICS_FILE))
        );
    }

    // The paired statistic over 1-0.5, 2-1, 3-2.5, 4-4.
    let separation = &comparison.metrics["minimum_separation_m"];
    assert_eq!(
        separation.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(separation.unit, "metres");
    assert_eq!(separation.count, 4);
    assert_eq!(separation.paired_seeds, vec![0, 1, 2, 3]);
    assert!(separation.unpaired.is_empty());
    assert_close(separation.mean_difference.expect("four pairs"), 0.5);
    assert_close(separation.spread.minimum.expect("a minimum"), 0.0);
    assert_close(separation.spread.maximum.expect("a maximum"), 1.0);
    assert_close(separation.spread.range.expect("a range"), 1.0);
    assert_close(
        separation.spread.sample_variance.expect("four pairs"),
        0.5 / 3.0,
    );
    assert_paired_interval(
        comparison.metrics["minimum_separation_m"]
            .confidence_interval
            .as_ref()
            .expect("four pairs have an interval"),
        &[0.5, 1.0, 0.5, 0.0],
    );

    // Every paired number links to both sides' manifests at that seed, in
    // `paired_seeds` order.
    let pairs = pairs_by_seed(&comparison);
    for (position, seed) in separation.paired_seeds.iter().enumerate() {
        let pair = pairs[seed];
        assert_eq!(separation.a_manifests[position], pair.a.manifest_sha256);
        assert_eq!(separation.b_manifests[position], pair.b.manifest_sha256);
    }

    // An always-reported count is a real value: A counted 0..3 collisions, B
    // counted none, so the paired differences are 0, 1, 2, 3.
    let collisions = &comparison.metrics["event_counts.by_family.collisions"];
    assert_eq!(collisions.unit, "records");
    assert_eq!(collisions.count, 4);
    assert_close(collisions.mean_difference.expect("four pairs"), 1.5);
    assert!(collisions.unpaired.is_empty());
}

/// Pairing by seed removes the between-seed variance both sides share, so the
/// paired interval is narrower than either side's unpaired across-seed interval.
#[test]
fn pairing_removes_the_between_seed_variance_the_sides_share() {
    let scratch = Scratch::new("crn-payoff");
    let a = [1.0, 2.0, 3.0, 4.0];
    let b = [0.5, 1.0, 2.5, 4.0];
    let differences = [0.5, 1.0, 0.5, 0.0];
    let (a_root, b_root, bank_path, _) = synthetic_pair(
        &scratch,
        "crn",
        &[0, 1, 2, 3],
        &separations(&a),
        &separations(&b),
    );

    let paired = compare_batches(&a_root, &b_root, &bank_path).expect("the batches compare");
    let paired_interval = paired.metrics["minimum_separation_m"]
        .confidence_interval
        .clone()
        .expect("four pairs have an interval");

    // The unpaired per-side interval `aggregate` reports for each side alone.
    let a_error = unpaired_standard_error(&a);
    let b_error = unpaired_standard_error(&b);
    let unpaired_error = (a_error.powi(2) + b_error.powi(2)).sqrt();

    assert_close(
        paired_interval.standard_error,
        unpaired_standard_error(&differences),
    );
    assert!(
        paired_interval.standard_error < a_error,
        "the paired standard error {} must be below side A's unpaired {a_error}",
        paired_interval.standard_error
    );
    assert!(
        paired_interval.standard_error < b_error,
        "the paired standard error {} must be below side B's unpaired {b_error}",
        paired_interval.standard_error
    );
    assert!(
        paired_interval.standard_error < unpaired_error,
        "the paired standard error {} must be below the unpaired difference-of-means {unpaired_error}",
        paired_interval.standard_error
    );
    assert!(
        paired_interval.half_width < T_CRITICAL_975[2] * a_error,
        "the paired interval must be narrower than side A's unpaired interval"
    );
}

/// The comparison disaggregates by mode pair and by movement, exactly as the
/// aggregation keys the same slices.
#[test]
fn the_mode_and_movement_slices_pair_by_mode_and_movement() {
    let scratch = Scratch::new("slices");
    let bucket = ["movement:a", "pedestrian_route:b"];
    let solo = ["movement:c", "movement:c"];
    let a_seed = |separation: f64, pedestrian: f64, bucket_separation: f64| SyntheticSeed {
        separation: reported(separation),
        mode_pairs: mode_pairs(
            reported(separation),
            absent(MetricStatus::NotObserved),
            reported(pedestrian),
        ),
        movements: BTreeMap::from([movement_bucket(
            bucket,
            reported(bucket_separation),
            absent(MetricStatus::NotApplicable),
            reported(0.5),
        )]),
        ..SyntheticSeed::default()
    };
    let a = vec![(0, a_seed(1.0, 2.0, 1.0)), (1, a_seed(2.0, 3.0, 2.0))];
    let b_seed = |separation: f64, pedestrian: f64, bucket_separation: f64| SyntheticSeed {
        separation: reported(separation),
        mode_pairs: mode_pairs(
            reported(separation),
            absent(MetricStatus::NotObserved),
            reported(pedestrian),
        ),
        movements: BTreeMap::from([
            movement_bucket(
                bucket,
                reported(bucket_separation),
                absent(MetricStatus::NotApplicable),
                reported(0.25),
            ),
            // Only side B visits `movement:c`: the bucket is sparse, so side A
            // counts it as no observation rather than a missing measurement.
            movement_bucket(
                solo,
                reported(9.0),
                absent(MetricStatus::NotObserved),
                absent(MetricStatus::NotObserved),
            ),
        ]),
        ..SyntheticSeed::default()
    };
    let b = vec![(0, b_seed(0.5, 1.0, 0.5)), (1, b_seed(1.0, 1.5, 1.0))];
    // Side B visits `movement:c` at one seed only: the bucket covers the run it
    // reached and counts the seed it did not reach as unobserved on both sides.
    let mut b = b;
    b[1].1.movements.remove(&format!("{}|{}", solo[0], solo[1]));
    let (a_root, b_root, bank_path, _) = synthetic_pair(&scratch, "slices", &[0, 1], &a, &b);
    let comparison = compare_batches(&a_root, &b_root, &bank_path).expect("the batches compare");

    // Mode slices: the pair's minimum separation, keyed by the ModePair label.
    assert_eq!(
        comparison
            .mode_pair_slices
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            "pedestrian_pedestrian".to_owned(),
            "vehicle_pedestrian".to_owned(),
            "vehicle_vehicle".to_owned()
        ]
    );
    let vehicle_vehicle = &comparison.mode_pair_slices["vehicle_vehicle"]["minimum_separation_m"];
    assert_eq!(vehicle_vehicle.unit, "metres");
    assert_eq!(vehicle_vehicle.count, 2);
    assert_close(vehicle_vehicle.mean_difference.expect("two pairs"), 0.75);
    let pedestrian = &comparison.mode_pair_slices["pedestrian_pedestrian"]["minimum_separation_m"];
    assert_eq!(pedestrian.count, 2);
    assert_close(pedestrian.mean_difference.expect("two pairs"), 1.25);

    // A mode pair neither side observed is counted unpaired, not read as zero.
    let vehicle_pedestrian =
        &comparison.mode_pair_slices["vehicle_pedestrian"]["minimum_separation_m"];
    assert_eq!(vehicle_pedestrian.count, 0);
    assert_eq!(vehicle_pedestrian.mean_difference, None);
    assert_eq!(vehicle_pedestrian.confidence_interval, None);
    assert_eq!(vehicle_pedestrian.unpaired.len(), 2);
    assert!(
        vehicle_pedestrian
            .unpaired
            .iter()
            .all(|seed| seed.a == MetricStatus::NotObserved && seed.b == MetricStatus::NotObserved)
    );

    // Movement slices: the bucket key is the two movement keys sorted and
    // joined, and the bucket carries its three metrics.
    let shared = format!("{}|{}", bucket[0], bucket[1]);
    let shared_slice = &comparison.movement_slices[&shared];
    assert_eq!(
        shared_slice.movement_keys,
        [bucket[0].to_owned(), bucket[1].to_owned()]
    );
    assert_eq!(
        shared_slice.metrics.keys().cloned().collect::<Vec<_>>(),
        vec![
            "minimum_post_encroachment_s".to_owned(),
            "minimum_separation_m".to_owned(),
            "minimum_ttc_s".to_owned()
        ]
    );
    let separation = &shared_slice.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 2);
    assert_close(separation.mean_difference.expect("two pairs"), 0.75);
    let pet = &shared_slice.metrics["minimum_post_encroachment_s"];
    assert_eq!(pet.unit, "seconds");
    assert_close(pet.mean_difference.expect("two pairs"), 0.25);
    let ttc = &shared_slice.metrics["minimum_ttc_s"];
    assert_eq!(ttc.count, 0);
    assert_eq!(ttc.mean_difference, None);

    // The bucket only side B carries is unpaired on side A, by seed, and the
    // seed the bucket never reached is unobserved on both sides.
    let solo_slice = &comparison.movement_slices[&format!("{}|{}", solo[0], solo[1])];
    let solo_separation = &solo_slice.metrics["minimum_separation_m"];
    assert_eq!(solo_separation.count, 0);
    assert_eq!(
        solo_separation
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![
            (0, MetricStatus::NotObserved, MetricStatus::Reported),
            (1, MetricStatus::NotObserved, MetricStatus::NotObserved)
        ]
    );
    assert_eq!(
        solo_separation.count + solo_separation.unpaired.len(),
        comparison.pairs.len()
    );
}

/// The counted event families pair by mode and by one agent's own movement, and
/// the agent movement slice carries the movement's operational values as well.
#[test]
fn the_event_families_pair_by_mode_and_by_agent_movement() {
    let scratch = Scratch::new("event-slices");
    let families = |counts: &[(&str, u64)]| -> BTreeMap<String, u64> {
        counts
            .iter()
            .map(|(family, count)| ((*family).to_owned(), *count))
            .collect()
    };
    let a = vec![
        (
            0,
            SyntheticSeed {
                event_families_by_mode: BTreeMap::from([(
                    "vehicle".to_owned(),
                    families(&[("collisions", 1), ("queue_events", 4)]),
                )]),
                event_families_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    families(&[("collisions", 1)]),
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
                event_families_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    families(&[("collisions", 3)]),
                )]),
                // Only the counted records reach this movement on side A here,
                // so side A makes no observation of its operational values.
                ..SyntheticSeed::default()
            },
        ),
    ];
    let b = vec![
        (
            0,
            SyntheticSeed {
                event_families_by_mode: BTreeMap::from([(
                    "vehicle".to_owned(),
                    families(&[("collisions", 0)]),
                )]),
                event_families_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    families(&[("collisions", 0)]),
                )]),
                operational_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    operational_bucket(1.0, 1.0),
                )]),
                ..SyntheticSeed::default()
            },
        ),
        (
            1,
            SyntheticSeed {
                event_families_by_mode: BTreeMap::from([(
                    "vehicle".to_owned(),
                    families(&[("collisions", 1)]),
                )]),
                event_families_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    families(&[("collisions", 1)]),
                )]),
                operational_by_movement: BTreeMap::from([(
                    "movement:east".to_owned(),
                    operational_bucket(2.0, 3.0),
                )]),
                ..SyntheticSeed::default()
            },
        ),
    ];
    let (a_root, b_root, bank_path, _) = synthetic_pair(&scratch, "event-slices", &[0, 1], &a, &b);
    let comparison = compare_batches(&a_root, &b_root, &bank_path).expect("the batches compare");

    // Mode slices: both modes carry every counted family, and a family a mode
    // recorded no record of is an observed zero on both sides.
    assert_eq!(
        comparison
            .mode_event_slices
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["pedestrian".to_owned(), "vehicle".to_owned()]
    );
    let mut expected_families: Vec<String> = FAMILIES
        .iter()
        .map(|family| format!("event_counts.{family}"))
        .collect();
    expected_families.sort();
    for (mode, slice) in &comparison.mode_event_slices {
        assert_eq!(
            slice.keys().cloned().collect::<Vec<_>>(),
            expected_families,
            "mode '{mode}' carries every counted family"
        );
        for (key, distribution) in slice {
            assert_eq!(distribution.unit, "records");
            assert_eq!(distribution.count, 2, "'{mode}/{key}'");
            assert!(distribution.unpaired.is_empty());
        }
    }
    let collisions = &comparison.mode_event_slices["vehicle"]["event_counts.collisions"];
    assert_close(collisions.mean_difference.expect("two pairs"), 1.5);
    assert_eq!(collisions.paired_seeds, vec![0, 1]);
    let queue_events = &comparison.mode_event_slices["vehicle"]["event_counts.queue_events"];
    assert_close(queue_events.mean_difference.expect("two pairs"), 2.0);
    assert_close(
        comparison.mode_event_slices["pedestrian"]["event_counts.collisions"]
            .mean_difference
            .expect("two pairs"),
        0.0,
    );

    // Agent movement slices: one movement key, its operational values and its
    // counted families, each paired over the seeds both sides report.
    assert_eq!(
        comparison
            .agent_movement_slices
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["movement:east".to_owned()]
    );
    let east = &comparison.agent_movement_slices["movement:east"];
    let mut expected_movement: Vec<String> = OPERATIONAL_FIELDS
        .iter()
        .map(|field| (*field).to_owned())
        .collect();
    expected_movement.extend(expected_families.iter().cloned());
    expected_movement.sort();
    assert_eq!(east.keys().cloned().collect::<Vec<_>>(), expected_movement);
    let east_collisions = &east["event_counts.collisions"];
    assert_close(east_collisions.mean_difference.expect("two pairs"), 1.5);
    let east_throughput = &east["throughput_agents_per_s"];
    assert_eq!(east_throughput.unit, "agents_per_second");
    // Seed 1's side A carried no operational block for the movement, so the
    // pair is counted unpaired with both sides' statuses rather than zeroed.
    assert_eq!(east_throughput.count, 1);
    assert_close(east_throughput.mean_difference.expect("one pair"), 2.0);
    assert_eq!(east_throughput.paired_seeds, vec![0]);
    assert_eq!(
        east_throughput
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![(1, MetricStatus::NotObserved, MetricStatus::Reported)]
    );
    let east_queue = &east["maximum_queue_length_agents"];
    assert_eq!(east_queue.unit, "agents");
    assert_eq!(east_queue.count, 1);
    assert_close(east_queue.mean_difference.expect("one pair"), 1.0);
    assert_eq!(east_queue.paired_seeds, vec![0]);

    // Every distribution accounts for every pair exactly once.
    for (path, distribution) in every_distribution(&comparison) {
        assert_eq!(
            distribution.count + distribution.unpaired.len(),
            comparison.pairs.len(),
            "'{path}' must account for every pair"
        );
    }
}

/// The close-pass families pair over the seed bank under the same cells the
/// aggregation carries, each with its unit and its explicit applicability, and a
/// batch whose runs recorded a closed pass refuses nothing.
#[test]
fn the_close_pass_families_pair_over_the_seed_bank() {
    let scratch = Scratch::new("close-pass");
    let movement = "movement:through|movement:through";

    // Every run of both batches declares the same two bands, so both sides report
    // the same band cells: band 7's duration where the pair recorded one, and band
    // 9 excluded by its own mode gate, which is not applicable rather than zero.
    let with_band_7 = |mut block: ClosePassMetrics, duration: MetricValue| {
        block.run.clearance_band_durations_s =
            BTreeMap::from([(7, duration), (9, absent(MetricStatus::NotApplicable))]);
        block
    };

    // Side A recorded a closed pass on the shared road in both seeds: one overtake
    // from attempt to completion, two closed observations whose least clearance is
    // 0.4 m, a violation, and 1.2 s inside the declared band.
    let recorded = || {
        with_band_7(
            close_pass_block(
                close_pass_bucket([1, 1, 1, 0], 2, minimum_reported(0.4), 1),
                movement,
                "facility:road",
            ),
            reported(1.2),
        )
    };
    let a = vec![
        (
            0,
            SyntheticSeed {
                close_pass: recorded(),
                ..SyntheticSeed::default()
            },
        ),
        (
            1,
            SyntheticSeed {
                close_pass: recorded(),
                ..SyntheticSeed::default()
            },
        ),
    ];

    // Side B closed one observation in the first seed and none in the second, so
    // the paired count difference and the paired clearance difference are known
    // exactly and the second seed's clearance is a counted pair.
    let b = vec![
        (
            0,
            SyntheticSeed {
                close_pass: with_band_7(
                    close_pass_block(
                        close_pass_bucket([0, 0, 0, 0], 1, minimum_reported(0.6), 0),
                        movement,
                        "facility:road",
                    ),
                    reported(0.8),
                ),
                ..SyntheticSeed::default()
            },
        ),
        (
            1,
            SyntheticSeed {
                close_pass: with_band_7(close_pass_quiet(), absent(MetricStatus::NotObserved)),
                ..SyntheticSeed::default()
            },
        ),
    ];

    let (a_root, b_root, bank_path, _) = synthetic_pair(&scratch, "close-pass", &[0, 1], &a, &b);
    let comparison = compare_batches(&a_root, &b_root, &bank_path)
        .expect("a batch whose runs recorded a closed pass compares");

    // The run bucket: the families are paired whole-batch metrics under the
    // artifact block they are read from, each in its own unit, and a countable
    // family is paired in every seed because a run that recorded none reports the
    // observed zero.
    let close_passes = &comparison.metrics["close_pass.run.close_passes"];
    assert_eq!(close_passes.unit, "observations");
    assert_eq!(
        close_passes.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(close_passes.count, 2);
    assert_close(close_passes.mean_difference.expect("two pairs"), 1.5);
    assert_eq!(close_passes.paired_seeds, vec![0, 1]);
    assert!(close_passes.unpaired.is_empty());
    assert_close(
        comparison.metrics["close_pass.run.overtake_attempts"]
            .mean_difference
            .expect("two pairs"),
        1.0,
    );

    // A value family keeps its applicability: the pair whose side B recorded none
    // is counted unpaired rather than read as a zero clearance difference.
    let minimum = &comparison.metrics["close_pass.run.close_pass_minimum_clearance_m"];
    assert_eq!(minimum.unit, "metres");
    assert_eq!(minimum.count, 1);
    assert_close(minimum.mean_difference.expect("one pair"), -0.2);
    assert_eq!(minimum.paired_seeds, vec![0]);
    assert_eq!(
        minimum
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![(1, MetricStatus::Reported, MetricStatus::NotObserved)]
    );

    // Each declared band is its own cell in its own unit, and the band its mode
    // gate excludes is unpaired with both sides' not-applicable status.
    let band = &comparison.metrics["close_pass.run.clearance_band_durations_s.7"];
    assert_eq!(band.unit, "seconds");
    assert_eq!(band.count, 1);
    assert_close(band.mean_difference.expect("one pair"), 0.4);
    let excluded = &comparison.metrics["close_pass.run.clearance_band_durations_s.9"];
    assert_eq!(excluded.count, 0);
    assert_eq!(
        excluded
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![
            (0, MetricStatus::NotApplicable, MetricStatus::NotApplicable),
            (1, MetricStatus::NotApplicable, MetricStatus::NotApplicable)
        ]
    );

    // The mode-pair cells: the pair that recorded the pass pairs per seed, the
    // pair that cannot host one is unpaired on both sides as not applicable, and
    // the pair that could host one and recorded none is unpaired as not observed.
    let pair = |label: &str, key: &str| &comparison.close_pass_mode_pair_slices[label][key];
    assert_eq!(pair("vehicle_vehicle", "close_passes").unit, "observations");
    assert_close(
        pair("vehicle_vehicle", "close_passes")
            .mean_difference
            .expect("two pairs"),
        1.5,
    );
    assert_eq!(
        pair("pedestrian_pedestrian", "close_pass_minimum_clearance_m").count,
        0
    );
    assert!(
        pair("pedestrian_pedestrian", "close_pass_minimum_clearance_m")
            .unpaired
            .iter()
            .all(|seed| seed.a == MetricStatus::NotApplicable
                && seed.b == MetricStatus::NotApplicable)
    );
    assert!(
        pair("vehicle_pedestrian", "close_pass_minimum_clearance_m")
            .unpaired
            .iter()
            .all(|seed| seed.a == MetricStatus::NotObserved && seed.b == MetricStatus::NotObserved)
    );

    // The facility cells: the road that hosted the pass, and the centered facility
    // that cannot host one. Side B's second seed carried no road bucket at all,
    // which is no observation on that side rather than a zero.
    let road = &comparison.close_pass_facility_slices["facility:road"];
    assert_eq!(road["close_passes"].unit, "observations");
    assert_eq!(road["close_passes"].count, 1);
    assert_close(road["close_passes"].mean_difference.expect("one pair"), 1.0);
    assert_eq!(
        road["close_passes"]
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![(1, MetricStatus::Reported, MetricStatus::NotObserved)]
    );
    assert_eq!(
        comparison.close_pass_facility_slices["facility:centered"]
            ["close_pass_minimum_clearance_m"]
            .count,
        0
    );

    // The movement bucket is sparse on both sides the same way it is in the
    // aggregation: side B's second seed named none.
    let movement_slice = &comparison.close_pass_movement_slices[movement];
    assert_eq!(movement_slice["close_passes"].count, 1);
    assert_close(
        movement_slice["close_passes"]
            .mean_difference
            .expect("one pair"),
        1.0,
    );
    assert_eq!(movement_slice["close_passes"].unpaired.len(), 1);

    // No existing cell widened: the mode-pair and movement cells still hold
    // exactly the metrics they held before the close-pass families.
    for slice in comparison.mode_pair_slices.values() {
        assert_eq!(
            slice.keys().cloned().collect::<Vec<_>>(),
            vec!["minimum_separation_m".to_owned()]
        );
    }
    for slice in comparison.movement_slices.values() {
        assert_eq!(
            slice.metrics.keys().cloned().collect::<Vec<_>>(),
            vec![
                "minimum_post_encroachment_s".to_owned(),
                "minimum_separation_m".to_owned(),
                "minimum_ttc_s".to_owned()
            ]
        );
    }

    // Every distribution, close-pass cells included, accounts for every pair.
    for (path, distribution) in every_distribution(&comparison) {
        assert_eq!(
            distribution.count + distribution.unpaired.len(),
            comparison.pairs.len(),
            "'{path}' must account for every pair"
        );
    }
}

/// A sparse bucket first reached at a later seed still reports its unpaired
/// seeds ascending: the seed the bucket never reached at all is appended after
/// the readings, so the published list must be sorted rather than left in
/// insertion order.
#[test]
fn a_bucket_first_reached_at_a_later_seed_still_orders_its_unpaired_seeds() {
    let scratch = Scratch::new("sparse-order");
    let bucket = ["movement:d", "movement:d"];
    // Only side B carries the bucket, and only at seed 1. At seed 0 neither side
    // reports it, so seed 0 reaches the distribution through the fill alone.
    let carried = SyntheticSeed {
        movements: BTreeMap::from([movement_bucket(
            bucket,
            reported(2.0),
            absent(MetricStatus::NotApplicable),
            reported(0.5),
        )]),
        ..SyntheticSeed::default()
    };
    let a = vec![(0, SyntheticSeed::default()), (1, SyntheticSeed::default())];
    let b = vec![(0, SyntheticSeed::default()), (1, carried)];
    let (a_root, b_root, bank_path, _) = synthetic_pair(&scratch, "sparse-order", &[0, 1], &a, &b);
    let comparison = compare_batches(&a_root, &b_root, &bank_path).expect("the batches compare");

    let slice = &comparison.movement_slices[&format!("{}|{}", bucket[0], bucket[1])];
    let separation = &slice.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 0);
    assert_eq!(
        separation
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![
            (0, MetricStatus::NotObserved, MetricStatus::NotObserved),
            (1, MetricStatus::NotObserved, MetricStatus::Reported)
        ],
        "the seed the bucket never reached must be listed below the seed it did"
    );
    assert_eq!(
        separation.count + separation.unpaired.len(),
        comparison.pairs.len()
    );
}

/// A pair with a not-applicable or not-observed side is excluded from the
/// statistic and counted, so a metric with fewer reported pairs reports its
/// paired `n` rather than a fabricated zero.
#[test]
fn unpaired_values_are_counted_not_zeroed() {
    let scratch = Scratch::new("unpaired");
    let a = vec![
        (0, SyntheticSeed::default()),
        (1, SyntheticSeed::default()),
        (
            2,
            SyntheticSeed {
                separation: absent(MetricStatus::NotApplicable),
                ..SyntheticSeed::default()
            },
        ),
        (
            3,
            SyntheticSeed {
                separation: absent(MetricStatus::NotObserved),
                ..SyntheticSeed::default()
            },
        ),
    ];
    let b = separations(&[0.0, 0.0, 2.0, 3.0]);
    let (a_root, b_root, bank_path, _) =
        synthetic_pair(&scratch, "unpaired", &[0, 1, 2, 3], &a, &b);
    let comparison = compare_batches(&a_root, &b_root, &bank_path).expect("the batches compare");

    // Side A reports 1.0 at seeds 0 and 1, which differ from B's 0.0 and 0.0.
    let separation = &comparison.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 2);
    assert_eq!(separation.paired_seeds, vec![0, 1]);
    assert_close(separation.mean_difference.expect("two pairs"), 1.0);
    assert_close(separation.spread.minimum.expect("a minimum"), 1.0);
    assert_close(separation.spread.maximum.expect("a maximum"), 1.0);
    assert_eq!(
        separation
            .unpaired
            .iter()
            .map(|seed| (seed.seed, seed.a, seed.b))
            .collect::<Vec<_>>(),
        vec![
            (2, MetricStatus::NotApplicable, MetricStatus::Reported),
            (3, MetricStatus::NotObserved, MetricStatus::Reported)
        ]
    );
    assert_eq!(separation.a_manifests.len(), 2);
    assert_eq!(separation.b_manifests.len(), 2);

    // A metric neither side ever observes has no mean at all, not a zero one.
    let ttc = &comparison.metrics["minimum_ttc_s"];
    assert_eq!(ttc.count, 0);
    assert_eq!(ttc.mean_difference, None);
    assert_eq!(ttc.spread.minimum, None);
    assert_eq!(ttc.confidence_interval, None);
    assert_eq!(ttc.unpaired.len(), 4);
    assert!(ttc.a_manifests.is_empty());

    // A single paired seed measures no spread, so it reports no interval.
    let single = vec![(
        7,
        SyntheticSeed {
            separation: reported(5.0),
            ..SyntheticSeed::default()
        },
    )];
    let (single_a, single_b, single_bank_path, _) = synthetic_pair(
        &scratch,
        "single",
        &[7],
        &single,
        &[(7, SyntheticSeed::default())],
    );
    let single_comparison = compare_batches(&single_a, &single_b, &single_bank_path)
        .expect("one paired seed still compares");
    let separation = &single_comparison.metrics["minimum_separation_m"];
    assert_eq!(separation.count, 1);
    assert_eq!(separation.mean_difference, Some(4.0));
    assert_eq!(separation.confidence_interval, None);
}

/// Every comparison links to both run manifests and to
/// `metric_definition_version: 3`, and its statuses account for every pair.
#[test]
fn every_comparison_links_to_both_manifests_and_the_definition_version() {
    let scratch = Scratch::new("links");
    let (a_root, b_root, bank_path, bank) = synthetic_pair(
        &scratch,
        "links",
        &[0, 1, 2],
        &separations(&[1.0, 2.0, 3.0]),
        &separations(&[0.5, 2.5, 3.5]),
    );
    let comparison = compare_batches(&a_root, &b_root, &bank_path).expect("the batches compare");

    let pairs = pairs_by_seed(&comparison);
    for (key, distribution) in every_distribution(&comparison) {
        assert_eq!(
            distribution.metric_definition_version, METRIC_DEFINITION_VERSION,
            "'{key}' reports another definition revision"
        );
        assert!(
            [
                "seconds",
                "metres",
                "records",
                "observations",
                "agents_per_second",
                "agents",
                "intervals",
                "agent_seconds"
            ]
            .contains(&distribution.unit.as_str()),
            "'{key}' reports the unit {}",
            distribution.unit
        );
        assert_eq!(
            distribution.count,
            distribution.paired_seeds.len(),
            "'{key}' counts its paired seeds"
        );
        assert_eq!(
            distribution.a_manifests.len(),
            distribution.paired_seeds.len(),
            "'{key}' links side A's manifests"
        );
        assert_eq!(
            distribution.b_manifests.len(),
            distribution.paired_seeds.len(),
            "'{key}' links side B's manifests"
        );
        assert_eq!(
            distribution.count + distribution.unpaired.len(),
            comparison.pairs.len(),
            "'{key}' must account for every pair"
        );
        assert!(
            distribution
                .paired_seeds
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "'{key}' orders its paired seeds"
        );
        assert!(
            distribution
                .unpaired
                .windows(2)
                .all(|pair| pair[0].seed < pair[1].seed),
            "'{key}' orders its unpaired seeds"
        );
        for (position, seed) in distribution.paired_seeds.iter().enumerate() {
            let pair = pairs
                .get(seed)
                .unwrap_or_else(|| panic!("'{key}' pairs a seed the table holds"));
            assert_eq!(
                distribution.a_manifests[position], pair.a.manifest_sha256,
                "'{key}' links the wrong side A manifest at seed {seed}"
            );
            assert_eq!(
                distribution.b_manifests[position], pair.b.manifest_sha256,
                "'{key}' links the wrong side B manifest at seed {seed}"
            );
        }
    }

    // The batch and bank links are the files on disk.
    assert_eq!(
        comparison.a.sha256,
        file_sha256(&a_root.join(BATCH_MANIFEST_FILE))
    );
    assert_eq!(
        comparison.b.sha256,
        file_sha256(&b_root.join(BATCH_MANIFEST_FILE))
    );
    assert_eq!(comparison.seed_bank.content_sha256, bank.content_sha256);
    assert_eq!(comparison.seed_bank.content_sha256, file_sha256(&bank_path));
}

/// The ordering is deterministic: the same runs written in another order in the
/// batch manifest compare to the same artifact, and every list ascends.
#[test]
fn the_ordering_is_deterministic() {
    let scratch = Scratch::new("ordering");
    let bank_seeds = [0, 1, 2];
    let a = separations(&[1.0, 2.0, 3.0]);
    let b = separations(&[0.5, 2.5, 3.5]);
    let (bank_path, bank) = write_bank(&scratch, "ordering-bank.json", &bank_seeds);
    let a_root = scratch.path("ordering-a");
    let a_reversed = scratch.path("ordering-a-reversed");
    let b_root = scratch.path("ordering-b");
    write_side(&a_root, &bank, &bank_seeds, &a, &Deviation::default());
    write_side(
        &a_reversed,
        &bank,
        &bank_seeds,
        &a,
        &Deviation {
            reverse_runs: true,
            ..Deviation::default()
        },
    );
    write_side(&b_root, &bank, &bank_seeds, &b, &Deviation::default());

    let ordered =
        compare_batches(&a_root, &b_root, &bank_path).expect("the batches over one bank compare");
    let reversed = compare_batches(&a_reversed, &b_root, &bank_path)
        .expect("a reversed run list still compares");
    // Only the batch manifest's own bytes differ, so only its hash differs.
    assert_ne!(ordered.a.sha256, reversed.a.sha256);
    let mut normalized = reversed;
    normalized.a.sha256 = ordered.a.sha256.clone();
    assert_eq!(ordered, normalized);

    assert_eq!(
        ordered
            .pairs
            .iter()
            .map(|pair| pair.seed)
            .collect::<Vec<_>>(),
        vec![0, 1, 2],
        "the pairs follow the seed bank's order"
    );
    for (key, distribution) in every_distribution(&ordered) {
        assert!(
            distribution
                .paired_seeds
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "'{key}' orders its paired seeds ascending"
        );
    }
    assert!(
        ordered
            .metrics
            .keys()
            .zip(ordered.metrics.keys().skip(1))
            .all(|(left, right)| left < right)
    );

    // The command writes the same bytes every time and nothing to the roots.
    let output = scratch.cli(&[
        "compare",
        "--a",
        a_root.to_str().expect("the path is UTF-8"),
        "--b",
        b_root.to_str().expect("the path is UTF-8"),
        "--seed-bank",
        bank_path.to_str().expect("the path is UTF-8"),
        "--output",
        "comparison.json",
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let written = std::fs::read(scratch.path(COMPARISON_FILE)).expect("the comparison is written");
    let decoded: Comparison = serde_json::from_slice(&written).expect("the comparison is JSON");
    assert_eq!(decoded, ordered);
}

/// An input that would silently mis-pair is refused: a batch without a bank, two
/// batches that name different banks, and a bank file that is not the one they
/// recorded.
#[test]
fn a_disagreeing_seed_bank_is_refused() {
    let scratch = Scratch::new("bank-refusals");
    let bank_seeds = [0, 1];
    let a = separations(&[1.0, 2.0]);
    let b = separations(&[0.5, 1.5]);
    let (bank_path, bank) = write_bank(&scratch, "bank.json", &bank_seeds);

    // A side that names no bank cannot be proven to have paired with the other.
    let no_bank = scratch.path("no-bank");
    write_side(
        &no_bank,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            without_bank: true,
            ..Deviation::default()
        },
    );
    let a_root = scratch.path("a");
    write_side(&a_root, &bank, &bank_seeds, &a, &Deviation::default());
    assert!(matches!(
        compare_batches(&a_root, &no_bank, &bank_path),
        Err(CompareError::NoSeedBank { side: Side::B, .. })
    ));

    // Two batches that name different banks pair nothing.
    let other_bank = scratch.path("other-bank");
    write_side(
        &other_bank,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            seed_bank_sha256: Some("f".repeat(64)),
            ..Deviation::default()
        },
    );
    assert!(matches!(
        compare_batches(&a_root, &other_bank, &bank_path),
        Err(CompareError::SeedBankMismatch { .. })
    ));

    // A bank file that is not what both batches recorded is refused, even when
    // its seeds happen to overlap.
    let (other_file, _) = write_bank(&scratch, "other-file.json", &[0, 1, 9]);
    let b_root = scratch.path("b");
    write_side(&b_root, &bank, &bank_seeds, &b, &Deviation::default());
    let error = compare_batches(&a_root, &b_root, &other_file)
        .expect_err("a bank file the batches did not run is refused");
    assert!(matches!(error, CompareError::SeedBankFile { .. }));
    assert!(
        error.to_string().contains("both batches recorded"),
        "the diagnostic must say what disagreed: {error}"
    );

    // The seed bank's own failures reach the caller unchanged.
    assert!(matches!(
        compare_batches(&a_root, &b_root, &scratch.path("absent.json")),
        Err(CompareError::SeedBank(_))
    ));
}

/// A batch whose seed order is not the bank's order, or whose seed count is not
/// the bank's, is refused rather than paired positionally.
#[test]
fn a_different_seed_order_is_refused() {
    let scratch = Scratch::new("order-refusals");
    let bank_seeds = [0, 1, 2];
    let a = separations(&[1.0, 2.0, 3.0]);
    let b = separations(&[0.5, 2.5, 3.5]);
    let (bank_path, bank) = write_bank(&scratch, "bank.json", &bank_seeds);
    let a_root = scratch.path("a");
    write_side(&a_root, &bank, &bank_seeds, &a, &Deviation::default());

    let descending = scratch.path("descending");
    write_side(
        &descending,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            seeds: Some(vec![2, 1, 0]),
            ..Deviation::default()
        },
    );
    assert!(matches!(
        compare_batches(&a_root, &descending, &bank_path),
        Err(CompareError::SeedOrder {
            side: Side::B,
            position: 0,
            expected: 0,
            observed: 2,
            ..
        })
    ));

    let short = scratch.path("short");
    write_side(
        &short,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            seeds: Some(vec![0, 1]),
            ..Deviation::default()
        },
    );
    assert!(matches!(
        compare_batches(&a_root, &short, &bank_path),
        Err(CompareError::SeedCount {
            side: Side::B,
            count: 2,
            expected: 3,
            ..
        })
    ));
}

/// A seed named twice, a bank seed with no run, a run the batch's own seed list
/// does not pair, and a run artifact that changed are each refused.
#[test]
fn unpaired_runs_and_missing_artifacts_are_refused() {
    let scratch = Scratch::new("run-refusals");
    let bank_seeds = [0, 1, 2];
    let a = separations(&[1.0, 2.0, 3.0]);
    let b = separations(&[0.5, 2.5, 3.5]);
    let (bank_path, bank) = write_bank(&scratch, "bank.json", &bank_seeds);
    let a_root = scratch.path("a");
    write_side(&a_root, &bank, &bank_seeds, &a, &Deviation::default());

    let b_root = scratch.path("b");
    write_side(&b_root, &bank, &bank_seeds, &b, &Deviation::default());

    let duplicated = scratch.path("duplicated");
    write_side(
        &duplicated,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            duplicate_seed: Some(1),
            ..Deviation::default()
        },
    );
    assert!(matches!(
        compare_batches(&a_root, &duplicated, &bank_path),
        Err(CompareError::DuplicateSeed {
            side: Side::B,
            seed: 1,
            ..
        })
    ));

    let missing = scratch.path("missing-run");
    write_side(
        &missing,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            drop_run: Some(2),
            ..Deviation::default()
        },
    );
    assert!(matches!(
        compare_batches(&a_root, &missing, &bank_path),
        Err(CompareError::MissingSeed {
            side: Side::B,
            seed: 2,
            ..
        })
    ));

    let stray = scratch.path("stray-run");
    write_side(
        &stray,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            stray_seed: Some(7),
            ..Deviation::default()
        },
    );
    assert!(matches!(
        compare_batches(&a_root, &stray, &bank_path),
        Err(CompareError::StrayRun {
            side: Side::B,
            seed: 7,
            ..
        })
    ));

    // A run whose metrics artifact is gone cannot be paired.
    std::fs::remove_file(b_root.join("seed-1").join(METRICS_FILE))
        .expect("the metrics artifact is removed");
    assert!(matches!(
        compare_batches(&a_root, &b_root, &bank_path),
        Err(CompareError::Io { side: Side::B, .. })
    ));
}

/// A metric one side reports and the other does not is refused: the two sides
/// were not measured under one metric definition.
#[test]
fn a_metric_available_on_one_side_only_is_refused() {
    let scratch = Scratch::new("one-sided-metric");
    let bank_seeds = [0, 1];
    let a = separations(&[1.0, 2.0]);
    let b = separations(&[0.5, 1.5]);
    let (bank_path, bank) = write_bank(&scratch, "bank.json", &bank_seeds);
    let a_root = scratch.path("a");
    write_side(&a_root, &bank, &bank_seeds, &a, &Deviation::default());
    let b_root = scratch.path("b");
    write_side(
        &b_root,
        &bank,
        &bank_seeds,
        &b,
        &Deviation {
            drop_family: Some("yields".to_owned()),
            ..Deviation::default()
        },
    );

    let error =
        compare_batches(&a_root, &b_root, &bank_path).expect_err("a one-sided metric is refused");
    assert!(matches!(
        error,
        CompareError::MetricAvailability {
            side: Side::B,
            seed: 0,
            ..
        }
    ));
    assert!(
        error.to_string().contains("event_counts.by_family.yields"),
        "the diagnostic must name the metric: {error}"
    );
}

/// A run that disagrees with what its batch recorded is refused, and a manifest
/// link or definition revision that disagrees is refused.
#[test]
fn a_run_that_disagrees_with_its_batch_is_refused() {
    let scratch = Scratch::new("run-guards");
    let bank_seeds = [0, 1];
    let a = separations(&[1.0, 2.0]);
    let b = separations(&[0.5, 1.5]);
    let (bank_path, bank) = write_bank(&scratch, "bank.json", &bank_seeds);
    let a_root = scratch.path("a");
    write_side(&a_root, &bank, &bank_seeds, &a, &Deviation::default());
    let b_root = scratch.path("b");
    write_side(&b_root, &bank, &bank_seeds, &b, &Deviation::default());

    // A run manifest that changed after the batch finished.
    let manifest_path = b_root.join("seed-0").join(MANIFEST_FILE);
    let original = std::fs::read(&manifest_path).expect("the manifest is read");
    std::fs::write(
        &manifest_path,
        b"{\"manifest_version\": 1, \"seed\": 0, \"x\": 1}\n",
    )
    .expect("the manifest is rewritten");
    assert!(matches!(
        compare_batches(&a_root, &b_root, &bank_path),
        Err(CompareError::ManifestDigest {
            side: Side::B,
            seed: 0,
            ..
        })
    ));
    std::fs::write(&manifest_path, &original).expect("the manifest is restored");

    // A metrics artifact that links to another manifest.
    let metrics_path = b_root.join("seed-1").join(METRICS_FILE);
    let original_metrics: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metrics_path).expect("the metrics are read"))
            .expect("the metrics are JSON");
    let manifest_sha256 = original_metrics["manifest_sha256"].clone();
    let mut metrics = original_metrics.clone();
    metrics["manifest_sha256"] = serde_json::Value::String("a".repeat(64));
    write_json(&metrics_path, &metrics);
    assert!(matches!(
        compare_batches(&a_root, &b_root, &bank_path),
        Err(CompareError::ManifestLink {
            side: Side::B,
            seed: 1,
            ..
        })
    ));

    // A metrics artifact from another definition revision.
    let mut metrics = original_metrics.clone();
    metrics["metric_definition_version"] = serde_json::json!(METRIC_DEFINITION_VERSION + 1);
    write_json(&metrics_path, &metrics);
    assert!(matches!(
        compare_batches(&a_root, &b_root, &bank_path),
        Err(CompareError::DefinitionVersion {
            side: Side::B,
            seed: 1,
            version,
            ..
        }) if version == METRIC_DEFINITION_VERSION + 1
    ));

    // A metrics artifact that claims a reported value without one.
    let mut metrics = original_metrics.clone();
    metrics["minimum_separation_m"] = serde_json::json!({
        "status": "reported",
        "agent": 0,
        "other": 1,
        "mode_pair": "vehicle_vehicle",
        "tick": 3
    });
    write_json(&metrics_path, &metrics);
    assert!(matches!(
        compare_batches(&a_root, &b_root, &bank_path),
        Err(CompareError::ReportedWithoutValue {
            side: Side::B,
            seed: 1,
            ..
        })
    ));
    assert_eq!(metrics["manifest_sha256"], manifest_sha256);
}

/// The command pairs two real batches over one bank, writes `comparison.json` to
/// the named path or the default, and mutates no artifact of either batch.
#[test]
fn the_compare_command_pairs_real_batches_and_mutates_no_run_artifact() {
    let scratch = Scratch::new("real");
    let written = scratch.cli(&[
        "seed-bank",
        "--start",
        "0",
        "--count",
        "2",
        "--output",
        "bank.json",
    ]);
    assert_eq!(
        written.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&written)
    );

    let ticks = TICKS.to_string();
    for (scenario, root) in [(WALKING, "a"), (CAR_FOLLOWING, "b")] {
        let scenario = repo_path(scenario);
        let output = scratch.cli(&[
            "batch",
            scenario.to_str().expect("the scenario path is UTF-8"),
            "--seed-bank",
            "bank.json",
            "--ticks",
            &ticks,
            "--out-root",
            root,
        ]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    }

    let a_root = scratch.path("a");
    let b_root = scratch.path("b");
    let before = (tree_hashes(&a_root), tree_hashes(&b_root));

    let output = scratch.cli(&[
        "compare",
        "--a",
        "a",
        "--b",
        "b",
        "--seed-bank",
        "bank.json",
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("comparison:"),
        "the command reports what it wrote: {}",
        stderr(&output)
    );

    let path = scratch.path(COMPARISON_FILE);
    let bytes = std::fs::read(&path).expect("the default output path is comparison.json");
    let comparison: Comparison = serde_json::from_slice(&bytes).expect("the comparison is JSON");
    assert_eq!(comparison.pairs.len(), 2);
    assert_eq!(
        comparison
            .pairs
            .iter()
            .map(|pair| pair.seed)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(comparison.seed_bank.path, "bank.json");
    assert_eq!(
        comparison.seed_bank.content_sha256,
        file_sha256(&scratch.path("bank.json"))
    );
    assert_eq!(comparison.a.seed_bank_sha256, comparison.b.seed_bank_sha256);
    assert!(!comparison.metrics.is_empty());
    for (key, distribution) in every_distribution(&comparison) {
        assert_eq!(
            distribution.count + distribution.unpaired.len(),
            comparison.pairs.len(),
            "'{key}' must account for every pair"
        );
        assert_eq!(
            distribution.metric_definition_version, METRIC_DEFINITION_VERSION,
            "'{key}' reports another definition revision"
        );
    }
    // Two different scenarios: at least one metric is sparse, and every
    // distribution still accounts for both pairs.
    assert!(
        every_distribution(&comparison)
            .iter()
            .any(|(_, distribution)| !distribution.unpaired.is_empty()),
        "a comparison of two scenarios must count unpaired seeds"
    );

    // The wrong-way families and their dimensions reach the comparison: the
    // three mode-pair slices carry the six families of metric definition v3,
    // and the run bucket is paired under the artifact block it is read from.
    assert_eq!(
        comparison
            .wrong_way_mode_pair_slices
            .keys()
            .collect::<Vec<_>>(),
        vec![
            "pedestrian_pedestrian",
            "vehicle_pedestrian",
            "vehicle_vehicle"
        ]
    );
    let wrong_way_intervals = &comparison.metrics["wrong_way.run.wrong_way_intervals"];
    assert_eq!(wrong_way_intervals.unit, "intervals");
    assert_eq!(
        wrong_way_intervals.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(
        comparison.metrics["wrong_way.run.wrong_way_exposure_agent_s"].unit,
        "agent_seconds"
    );
    for slice in comparison.wrong_way_mode_pair_slices.values() {
        assert_eq!(
            slice.keys().cloned().collect::<Vec<_>>(),
            vec![
                "wrong_way_conflicts",
                "wrong_way_distance_m",
                "wrong_way_duration_s",
                "wrong_way_encounters",
                "wrong_way_exposure_agent_s",
                "wrong_way_intervals",
            ]
        );
    }

    // Neither batch root changed, and neither gained a file.
    assert_eq!(before.0, tree_hashes(&a_root));
    assert_eq!(before.1, tree_hashes(&b_root));

    // Writing it again, or to stdout, produces exactly the same bytes.
    let again = scratch.cli(&[
        "compare",
        "--a",
        "a",
        "--b",
        "b",
        "--seed-bank",
        "bank.json",
    ]);
    assert_eq!(again.status.code(), Some(0), "stderr: {}", stderr(&again));
    assert_eq!(std::fs::read(&path).expect("the comparison is read"), bytes);

    let to_stdout = scratch.cli(&[
        "compare",
        "--a",
        "a",
        "--b",
        "b",
        "--seed-bank",
        "bank.json",
        "--output",
        "-",
    ]);
    assert_eq!(
        to_stdout.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&to_stdout)
    );
    assert_eq!(to_stdout.stdout, bytes);
    assert_eq!(before.0, tree_hashes(&a_root));
    assert_eq!(before.1, tree_hashes(&b_root));

    // A mismatched pair of batches exits non-zero and writes no comparison.
    std::fs::write(
        scratch.path("other.json"),
        "{\"seed_bank_version\": 1, \"seeds\": [0, 1, 2]}\n",
    )
    .expect("the other bank is written");
    let refused = scratch.cli(&[
        "compare",
        "--a",
        "a",
        "--b",
        "b",
        "--seed-bank",
        "other.json",
        "--output",
        "refused.json",
    ]);
    assert_eq!(
        refused.status.code(),
        Some(1),
        "stderr: {}",
        stderr(&refused)
    );
    assert!(
        !scratch.path("refused.json").exists(),
        "a refused comparison writes no artifact"
    );
}
