//! Contract tests for the `converge` command and its sensitivity report.
//!
//! A convergence report is the Fast/Standard/Fine experiment record the fidelity
//! presets exist for: three batches of one scenario, one per preset, run one
//! common-random-number seed bank over one simulated duration, read into one
//! `convergence.json` that gives every reported metric its value at each
//! fidelity, the paired refinement change from Fast to Standard and from
//! Standard to Fine, and a verdict against the declared tolerance. These tests
//! pin the tolerance rule against hand-chosen values, show an engineered
//! sensitivity being flagged rather than hidden, cover the mode and movement
//! disaggregation, the manifest and definition-version links, the reporting
//! statuses, the deterministic ordering, the refusal of three batches that do not
//! share one step, duration, and scenario, and the command's treatment of the run
//! artifacts it reads.
//!
//! Most tests build three synthetic fidelity batches with hand-written
//! `metrics.json` files, because only known values can be asserted exactly; the
//! last test runs the command over a real scenario and seed bank.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};
use tangle_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, BatchRun, BatchSpec, CONTINUOUS_CLASS,
    CONVERGENCE_COUNT_TOLERANCE, CONVERGENCE_FILE, CONVERGENCE_TOLERANCE, CONVERGENCE_VERSION,
    COUNT_CLASS, COUNT_UNITS, ConvergenceError, ConvergenceReport, ConvergenceVerdict, EventCounts,
    MANIFEST_FILE, METRIC_DEFINITION_VERSION, METRICS_FILE, MetricSensitivity, MetricStatus,
    MetricTolerance, MetricValue, MovementMinima, OperationalMetrics, OperationalValues, PRESETS,
    Preset, RunMetricsArtifact, SamplingPolicy, ScenarioProvenance, SeedBank, SeedBankReference,
    TOLERANCE_MEASURE, TOLERANCE_RULE, VERDICT_REFINEMENT, converge_batches, fidelity_ticks,
    read_seed_bank,
};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

/// The scenario the command-level test runs: it carries one movement, so the
/// report has a movement slice, and its demand makes the interaction metrics
/// observable at a small tick budget.
const SCENARIO: &str = "scenarios/benchmarks/car_following_v1.json5";

/// The Standard tick count the synthetic fixtures and the command-level test use.
/// Three simulated seconds: Fast advances 30 steps, Fine 150.
const STANDARD_TICKS: u64 = 60;

/// The declared fidelity order every fixture writes and every report reads.
const FIDELITIES: [&str; 3] = ["fast", "standard", "fine"];

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
        let dir =
            std::env::temp_dir().join(format!("tangle-cli-converge-{name}-{}", std::process::id()));
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
            .expect("tangle-cli runs")
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

/// Event counts with every family present and `collisions` set, so the always
/// reported counts are identical across the fidelities of a fixture.
fn event_counts(collisions: u64) -> EventCounts {
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
    EventCounts {
        total: by_family.values().sum(),
        by_family,
        by_family_kind,
        by_family_mode: BTreeMap::new(),
        by_family_movement: BTreeMap::new(),
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
        }
    }
}

/// The synthetic seeds of one fidelity for a list of separation values, seeds
/// `0..n`, with the default statuses.
fn separations(values: &[f64]) -> Vec<(u64, SyntheticSeed)> {
    values
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
        .collect()
}

/// The two seeds a fixture repeats one value into, seeds `0` and `1`.
fn two(value: SyntheticSeed) -> Vec<(u64, SyntheticSeed)> {
    vec![(0, value.clone()), (1, value)]
}

/// How a synthetic fidelity batch deviates from a well-formed one.
#[derive(Default, Clone)]
struct Deviation {
    /// Write the manifest's runs in descending seed order.
    reverse_runs: bool,
    /// Record this step instead of the preset's declared one.
    step_s: Option<f64>,
    /// Record this tick count instead of the declared rule's.
    ticks: Option<u64>,
    /// Record this scenario content hash instead of the shared one.
    scenario_sha256: Option<String>,
    /// Record this bank content hash instead of the bank's own.
    bank_sha256: Option<String>,
}

/// The scenario content hash every synthetic fidelity batch records: the three
/// batches must name one scenario.
fn shared_scenario_sha256() -> String {
    "a".repeat(64)
}

/// Write one fidelity's synthetic batch root: a `batch.json` naming `bank` at
/// `preset` and one run directory per given seed, each with a stand-in
/// `manifest.json` and a `metrics.json` built from the given values.
fn write_side(
    root: &Path,
    bank: &SeedBankReference,
    preset: Preset,
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
                event_counts: event_counts(values.collisions),
                operational: OperationalMetrics {
                    run: OperationalValues::not_observed(),
                    by_mode: ["vehicle", "pedestrian"]
                        .into_iter()
                        .map(|mode| (mode.to_owned(), OperationalValues::not_observed()))
                        .collect(),
                    by_movement: BTreeMap::new(),
                },
            },
        );
        runs.push(BatchRun {
            seed: *seed,
            directory,
            trace_sha256: "0".repeat(64),
            manifest_sha256,
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
                    content_sha256: deviation
                        .scenario_sha256
                        .clone()
                        .unwrap_or_else(shared_scenario_sha256),
                },
                ticks: deviation
                    .ticks
                    .unwrap_or_else(|| fidelity_ticks(preset.step_s, STANDARD_TICKS)),
                fidelity: preset.name.to_owned(),
                step_s: deviation.step_s.unwrap_or(preset.step_s),
                event_version: 2,
                model_version: "synthetic".to_owned(),
                build_revision: "0.0.0".to_owned(),
                sampling: SamplingPolicy::default(),
            },
            seed_bank: Some(SeedBankReference {
                path: bank.path.clone(),
                content_sha256: deviation
                    .bank_sha256
                    .clone()
                    .unwrap_or_else(|| bank.content_sha256.clone()),
            }),
            seeds: bank_seeds.to_vec(),
            runs,
        },
    );
}

/// The paths one synthetic convergence fixture needs: the shared bank and the
/// three fidelity batch roots, in the declared Fast, Standard, Fine order.
struct Fixture {
    dir: PathBuf,
    bank: SeedBankReference,
    bank_path: PathBuf,
    roots: [PathBuf; 3],
}

impl Fixture {
    /// Converge the fixture's three batches against its bank.
    fn report(&self) -> Result<ConvergenceReport, ConvergenceError> {
        converge_batches(
            &self.roots[0],
            &self.roots[1],
            &self.roots[2],
            &self.bank_path,
            CONVERGENCE_TOLERANCE,
        )
    }

    /// Write one more batch for the fixture's bank at the fidelity `fidelity`
    /// indexes, and return its root.
    fn side(
        &self,
        name: &str,
        fidelity: usize,
        seeds: &[(u64, SyntheticSeed)],
        deviation: &Deviation,
    ) -> PathBuf {
        let root = self.dir.join(name);
        write_side(
            &root,
            &self.bank,
            PRESETS[fidelity],
            &[0, 1],
            seeds,
            deviation,
        );
        root
    }
}

/// Write three synthetic fidelity batches over one bank of seeds `0, 1`, from
/// the per-fidelity seed values a test supplies in the declared order.
fn fixture(scratch: &Scratch, name: &str, values: [&[(u64, SyntheticSeed)]; 3]) -> Fixture {
    let (bank_path, bank) = write_bank(scratch, &format!("{name}-bank.json"), &[0, 1]);
    let mut roots = [PathBuf::new(), PathBuf::new(), PathBuf::new()];
    for (index, preset) in PRESETS.into_iter().enumerate() {
        let root = scratch.path(&format!("{name}-{}", preset.name));
        write_side(
            &root,
            &bank,
            preset,
            &[0, 1],
            values[index],
            &Deviation::default(),
        );
        roots[index] = root;
    }
    Fixture {
        dir: scratch.dir.clone(),
        bank,
        bank_path,
        roots,
    }
}

/// Assert two floats agree to within a relative tolerance no rounding can beat.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} differs from the expected {expected}"
    );
}

/// Every sensitivity in the report, with the key path it lives under.
fn every_sensitivity(report: &ConvergenceReport) -> Vec<(String, &MetricSensitivity)> {
    let mut all = Vec::new();
    for (key, sensitivity) in &report.metrics {
        all.push((format!("metrics.{key}"), sensitivity));
    }
    for (label, slice) in &report.mode_pair_slices {
        for (key, sensitivity) in slice {
            all.push((format!("mode_pair_slices.{label}.{key}"), sensitivity));
        }
    }
    for (bucket, slice) in &report.movement_slices {
        for (key, sensitivity) in &slice.metrics {
            all.push((format!("movement_slices.{bucket}.{key}"), sensitivity));
        }
    }
    all
}

/// The failure a report attempt produced.
fn refusal(result: Result<ConvergenceReport, ConvergenceError>) -> ConvergenceError {
    match result {
        Ok(report) => panic!(
            "expected a refusal, but {} metrics converged",
            report.metrics.len()
        ),
        Err(error) => error,
    }
}

/// An engineered refinement change above the tolerance is flagged materially
/// sensitive, one within it is converged, and the values behind both are the
/// across-seed means and the paired differences.
#[test]
fn an_engineered_refinement_change_above_the_tolerance_is_flagged() {
    let scratch = Scratch::new("engineered");
    // Separation 0.5 -> 1.0 -> 1.5: a 100% and a 50% refinement change, both far
    // outside the declared 5% tolerance.
    let mut fast = SyntheticSeed {
        ttc: reported(1.0),
        ..SyntheticSeed::default()
    };
    fast.separation = reported(0.5);
    let standard = SyntheticSeed {
        ttc: reported(1.0),
        ..SyntheticSeed::default()
    };
    // TTC 1.0 -> 1.0 -> 1.01 is a 1% change, inside the tolerance.
    let mut fine = SyntheticSeed {
        separation: reported(1.5),
        ttc: reported(1.01),
        ..SyntheticSeed::default()
    };
    // Collisions 0 -> 0 -> 1 gives the count class's absolute part a zero
    // reference to judge: one record of drift is inside it.
    fine.collisions = 1;

    let fixture = fixture(
        &scratch,
        "engineered",
        [&two(fast), &two(standard), &two(fine)],
    );
    let report = fixture
        .report()
        .expect("three fidelities over one bank converge");

    // The value at each fidelity is the across-seed mean, in declared order.
    let separation = &report.metrics["minimum_separation_m"];
    assert_eq!(
        separation
            .fidelities
            .iter()
            .map(|value| (
                value.fidelity.as_str(),
                value.value.as_ref().expect("reported").mean
            ))
            .collect::<Vec<_>>(),
        vec![
            ("fast", Some(0.5)),
            ("standard", Some(1.0)),
            ("fine", Some(1.5)),
        ]
    );

    // Fast -> Standard changed by 100%, Standard -> Fine by 50%: both exceed the
    // declared tolerance, so the metric is materially sensitive, not averaged.
    assert_eq!(separation.verdict, ConvergenceVerdict::MateriallySensitive);
    assert_eq!(
        separation
            .refinements
            .iter()
            .map(|step| (step.from.as_str(), step.to.as_str()))
            .collect::<Vec<_>>(),
        vec![("fast", "standard"), ("standard", "fine")]
    );
    assert_close(separation.refinements[0].relative_change.unwrap(), 1.0);
    assert_close(separation.refinements[1].relative_change.unwrap(), 0.5);
    assert_eq!(separation.refinements[1].materially_sensitive, Some(true));
    let paired = separation.refinements[1].paired.as_ref().expect("paired");
    assert_eq!(paired.count, 2);
    assert_eq!(paired.paired_seeds, vec![0, 1]);
    assert_close(paired.mean_difference.unwrap(), 0.5);

    // A 1% refinement change is inside the tolerance and stays converged.
    let ttc = &report.metrics["minimum_ttc_s"];
    assert_eq!(ttc.verdict, ConvergenceVerdict::Converged);
    assert_close(ttc.refinements[1].relative_change.unwrap(), 0.01);
    assert_eq!(ttc.refinements[1].materially_sensitive, Some(false));
    assert_close(
        ttc.refinements[1]
            .paired
            .as_ref()
            .expect("paired")
            .mean_difference
            .expect("a paired change"),
        0.01,
    );

    // A zero coarser count has no relative scale, so the count class's absolute
    // part decides: one record of drift is noise, not an unbounded relative
    // change. The relative change is still absent, because the ratio has none.
    let collisions = &report.metrics["event_counts.by_family.collisions"];
    assert_eq!(
        collisions.fidelities[1]
            .value
            .as_ref()
            .expect("reported")
            .mean,
        Some(0.0)
    );
    assert_eq!(
        collisions.refinements[1].relative_change, None,
        "a zero reference has no relative change"
    );
    assert_eq!(collisions.refinements[1].materially_sensitive, Some(false));
    assert_eq!(collisions.verdict, ConvergenceVerdict::Converged);
    assert_eq!(collisions.refinements[0].materially_sensitive, Some(false));

    // The report declares the tolerance its verdicts used, and each metric
    // carries the one it was read against.
    assert_eq!(report.convergence_version, CONVERGENCE_VERSION);
    assert_eq!(report.tolerance.measure, TOLERANCE_MEASURE);
    assert_eq!(report.tolerance.rule, TOLERANCE_RULE);
    assert_eq!(report.tolerance.relative, CONVERGENCE_TOLERANCE);
    assert_eq!(report.tolerance.count_absolute, CONVERGENCE_COUNT_TOLERANCE);
    assert_eq!(report.tolerance.count_units, COUNT_UNITS);
    assert_eq!(report.tolerance.verdict_refinement, VERDICT_REFINEMENT);
    assert_eq!(
        separation.tolerance,
        MetricTolerance {
            class: CONTINUOUS_CLASS.to_owned(),
            relative: CONVERGENCE_TOLERANCE,
            absolute: 0.0,
        }
    );
    assert_eq!(
        collisions.tolerance,
        MetricTolerance {
            class: COUNT_CLASS.to_owned(),
            relative: CONVERGENCE_TOLERANCE,
            absolute: CONVERGENCE_COUNT_TOLERANCE,
        }
    );
}

/// The tolerance is per metric: a count metric adds one whole unit of absolute
/// tolerance, so one record of drift from any coarser value is noise while more
/// than that still has to clear the relative part; a continuous metric keeps the
/// relative part alone, so a zero coarser value stays unbounded there.
#[test]
fn the_tolerance_is_per_metric_because_a_count_has_an_absolute_part() {
    let scratch = Scratch::new("per-metric");
    let collisions = |collisions: u64| SyntheticSeed {
        collisions,
        ..SyntheticSeed::default()
    };
    let engineered = |name: &str, fast: u64, standard: u64, fine: u64| {
        fixture(
            &scratch,
            name,
            [
                &two(collisions(fast)),
                &two(collisions(standard)),
                &two(collisions(fine)),
            ],
        )
        .report()
        .expect("three fidelities over one bank converge")
    };

    // 0 -> 0 -> 1: one record of drift, inside the count class's absolute part.
    let noise = engineered("count-noise", 0, 0, 1);
    let collisions = &noise.metrics["event_counts.by_family.collisions"];
    assert_eq!(collisions.tolerance.class, COUNT_CLASS);
    assert_eq!(collisions.tolerance.absolute, CONVERGENCE_COUNT_TOLERANCE);
    assert_eq!(collisions.refinements[1].relative_change, None);
    assert_eq!(collisions.refinements[1].materially_sensitive, Some(false));
    assert_eq!(collisions.verdict, ConvergenceVerdict::Converged);

    // 0 -> 0 -> 3: three records is more than the absolute part, so the zero
    // reference is still a material change rather than an unbounded one.
    let material = engineered("count-material", 0, 0, 3);
    let collisions = &material.metrics["event_counts.by_family.collisions"];
    assert_eq!(collisions.refinements[1].relative_change, None);
    assert_eq!(collisions.refinements[1].materially_sensitive, Some(true));
    assert_eq!(collisions.verdict, ConvergenceVerdict::MateriallySensitive);

    // 20 -> 21 is one record inside the absolute part; 20 -> 23 is three, past
    // both the absolute part and 5% of twenty.
    let inside = engineered("count-relative-inside", 0, 20, 21);
    let collisions = &inside.metrics["event_counts.by_family.collisions"];
    assert_close(collisions.refinements[1].relative_change.unwrap(), 0.05);
    assert_eq!(collisions.refinements[1].materially_sensitive, Some(false));
    let outside = engineered("count-relative-outside", 0, 20, 23);
    let collisions = &outside.metrics["event_counts.by_family.collisions"];
    assert_close(collisions.refinements[1].relative_change.unwrap(), 0.15);
    assert_eq!(collisions.refinements[1].materially_sensitive, Some(true));

    // A continuous metric has no absolute part, so a zero coarser value remains
    // an unbounded relative change: any nonzero change there is material.
    let continuous = SyntheticSeed {
        ttc: reported(0.0),
        ..SyntheticSeed::default()
    };
    let mut fine = continuous.clone();
    fine.ttc = reported(0.5);
    let report = fixture(
        &scratch,
        "continuous-zero",
        [&two(continuous.clone()), &two(continuous), &two(fine)],
    )
    .report()
    .expect("three fidelities over one bank converge");
    let ttc = &report.metrics["minimum_ttc_s"];
    assert_eq!(ttc.tolerance.class, CONTINUOUS_CLASS);
    assert_eq!(ttc.tolerance.absolute, 0.0);
    assert_eq!(ttc.refinements[1].relative_change, None);
    assert_eq!(ttc.refinements[1].materially_sensitive, Some(true));

    // Every metric in every report states the tolerance it was read against.
    for (path, sensitivity) in every_sensitivity(&noise) {
        assert_eq!(sensitivity.tolerance.relative, CONVERGENCE_TOLERANCE);
        assert_eq!(
            sensitivity.tolerance.absolute,
            match sensitivity.tolerance.class.as_str() {
                COUNT_CLASS => CONVERGENCE_COUNT_TOLERANCE,
                CONTINUOUS_CLASS => 0.0,
                other => panic!("'{path}' reports an unknown tolerance class '{other}'"),
            }
        );
    }
}

/// A value that is not applicable or not observed at both steps is a status, not
/// a zero, and the change it cannot define is inconclusive rather than converged.
#[test]
fn a_status_is_reported_not_zeroed_and_an_unmeasurable_change_is_inconclusive() {
    let scratch = Scratch::new("statuses");
    // The default seed reports no time to collision and no post-encroachment
    // time, and observes a separation.
    let values = &separations(&[2.0, 3.0]);
    let fixture = fixture(&scratch, "statuses", [values, values, values]);
    let report = fixture
        .report()
        .expect("three fidelities over one bank converge");

    let ttc = &report.metrics["minimum_ttc_s"];
    assert_eq!(ttc.verdict, ConvergenceVerdict::Inconclusive);
    for value in &ttc.fidelities {
        let distribution = value
            .value
            .as_ref()
            .expect("the fixture reports the metric");
        assert_eq!(distribution.count, 0);
        assert_eq!(distribution.mean, None, "a status is never read as zero");
        assert_eq!(distribution.not_applicable_seeds, vec![0, 1]);
        assert!(distribution.manifests.is_empty());
    }
    for step in &ttc.refinements {
        let paired = step
            .paired
            .as_ref()
            .expect("both fidelities report the metric");
        assert_eq!(paired.count, 0);
        assert_eq!(paired.mean_difference, None);
        assert_eq!(paired.confidence_interval, None);
        assert_eq!(paired.unpaired.len(), 2);
        assert!(paired.unpaired.iter().all(|unpaired| {
            unpaired.a == MetricStatus::NotApplicable && unpaired.b == MetricStatus::NotApplicable
        }));
        assert_eq!(step.relative_change, None);
        assert_eq!(step.materially_sensitive, None);
    }

    // A not-observed metric is the other status, and it is reported the same way.
    let pet = &report.metrics["minimum_post_encroachment_s"];
    assert_eq!(pet.verdict, ConvergenceVerdict::Inconclusive);
    assert_eq!(
        pet.fidelities[1]
            .value
            .as_ref()
            .expect("the fixture reports the metric")
            .not_observed_seeds,
        vec![0, 1]
    );

    // A metric both fidelities report with the same value has a zero change: it
    // is converged on a defined relative change, not on a missing one.
    let separation = &report.metrics["minimum_separation_m"];
    assert_eq!(separation.verdict, ConvergenceVerdict::Converged);
    assert_eq!(separation.refinements[1].relative_change, Some(0.0));
    assert_eq!(separation.refinements[1].materially_sensitive, Some(false));
}

/// The report is disaggregated by mode pair and by movement, and a slice one
/// fidelity does not carry is an absent value rather than a smaller sample.
#[test]
fn the_report_is_disaggregated_by_mode_and_by_movement() {
    let scratch = Scratch::new("slices");
    let bucket = ["movement:through", "movement:through"];
    let only_fast = ["movement:crossing", "movement:crossing"];
    let fast = SyntheticSeed {
        mode_pairs: mode_pairs(
            reported(2.0),
            absent(MetricStatus::NotObserved),
            absent(MetricStatus::NotObserved),
        ),
        movements: BTreeMap::from([movement_bucket(
            only_fast,
            reported(8.0),
            absent(MetricStatus::NotApplicable),
            absent(MetricStatus::NotObserved),
        )]),
        ..SyntheticSeed::default()
    };

    let mut standard = fast.clone();
    standard.movements = BTreeMap::from([movement_bucket(
        bucket,
        reported(2.0),
        absent(MetricStatus::NotApplicable),
        absent(MetricStatus::NotObserved),
    )]);

    let mut fine = standard.clone();
    fine.separation = reported(1.0);
    fine.mode_pairs = mode_pairs(
        reported(4.0),
        absent(MetricStatus::NotObserved),
        absent(MetricStatus::NotObserved),
    );
    fine.movements = BTreeMap::from([movement_bucket(
        bucket,
        reported(4.0),
        absent(MetricStatus::NotApplicable),
        absent(MetricStatus::NotObserved),
    )]);

    let fixture = fixture(&scratch, "slices", [&two(fast), &two(standard), &two(fine)]);
    let report = fixture
        .report()
        .expect("three fidelities over one bank converge");

    // Mode: every label the artifacts carry is in the report, with the observed
    // pair flagged and the pairs no run observed inconclusive.
    let vehicle_vehicle = &report.mode_pair_slices["vehicle_vehicle"]["minimum_separation_m"];
    assert_eq!(
        vehicle_vehicle
            .fidelities
            .iter()
            .map(|value| value.value.as_ref().expect("reported").mean)
            .collect::<Vec<_>>(),
        vec![Some(2.0), Some(2.0), Some(4.0)]
    );
    assert_close(vehicle_vehicle.refinements[1].relative_change.unwrap(), 1.0);
    assert_eq!(
        vehicle_vehicle.verdict,
        ConvergenceVerdict::MateriallySensitive
    );
    let pedestrian = &report.mode_pair_slices["pedestrian_pedestrian"]["minimum_separation_m"];
    assert_eq!(pedestrian.verdict, ConvergenceVerdict::Inconclusive);
    assert_eq!(
        pedestrian.fidelities[0]
            .value
            .as_ref()
            .expect("the fixture reports the label")
            .mean,
        None
    );

    // Movement: the bucket's key and its three metrics are recorded, and the
    // fidelity that does not carry the bucket has no value at all.
    let slice = &report.movement_slices["movement:through|movement:through"];
    assert_eq!(slice.movement_keys, bucket.map(str::to_owned));
    assert_eq!(
        slice.metrics.keys().cloned().collect::<Vec<_>>(),
        vec![
            "minimum_post_encroachment_s",
            "minimum_separation_m",
            "minimum_ttc_s"
        ]
    );
    let separation = &slice.metrics["minimum_separation_m"];
    assert_eq!(separation.fidelities[0].fidelity, "fast");
    assert_eq!(
        separation.fidelities[0].value, None,
        "a bucket a fidelity does not carry is no observation, not a zero"
    );
    assert_eq!(
        separation.fidelities[1]
            .value
            .as_ref()
            .expect("reported")
            .mean,
        Some(2.0)
    );
    // The step that reaches the bucket pairs it; the side that does not carry it
    // counts every seed as no observation rather than pairing a zero.
    let reached = separation.refinements[0]
        .paired
        .as_ref()
        .expect("the finer side of the first step carries the bucket");
    assert_eq!(reached.count, 0);
    assert_eq!(reached.mean_difference, None);
    assert_eq!(reached.unpaired.len(), 2);
    assert!(
        reached
            .unpaired
            .iter()
            .all(|unpaired| unpaired.a == MetricStatus::Reported
                && unpaired.b == MetricStatus::NotObserved)
    );
    assert_eq!(separation.refinements[0].relative_change, None);
    assert_eq!(separation.refinements[0].materially_sensitive, None);
    assert_eq!(
        separation.refinements[1]
            .paired
            .as_ref()
            .expect("the finer two fidelities pair the bucket")
            .mean_difference,
        Some(2.0)
    );
    assert_eq!(separation.verdict, ConvergenceVerdict::MateriallySensitive);
    assert_eq!(
        slice.metrics["minimum_ttc_s"].verdict,
        ConvergenceVerdict::Inconclusive
    );

    // A bucket only the coarsest fidelity carries reaches the first step alone:
    // the step that cannot reach it at all has no paired distribution.
    let lone = &report.movement_slices["movement:crossing|movement:crossing"];
    assert_eq!(lone.movement_keys, only_fast.map(str::to_owned));
    let lone = &lone.metrics["minimum_separation_m"];
    assert_eq!(
        lone.fidelities
            .iter()
            .map(|value| value.value.as_ref().map(|value| value.mean))
            .collect::<Vec<_>>(),
        vec![Some(Some(8.0)), None, None]
    );
    assert_eq!(
        lone.refinements[0]
            .paired
            .as_ref()
            .expect("the coarser side carries the bucket")
            .count,
        0
    );
    assert_eq!(lone.refinements[1].paired, None);
    assert_eq!(lone.refinements[1].relative_change, None);
    assert_eq!(lone.verdict, ConvergenceVerdict::Inconclusive);
}

/// Every reported metric links to the manifests of every fidelity it reports and
/// to `metric_definition_version: 2`, in the declared ordering.
#[test]
fn every_metric_links_to_its_manifests_and_the_definition_version() {
    let scratch = Scratch::new("links");
    let bucket = ["movement:through", "movement:through"];
    let movement = BTreeMap::from([movement_bucket(
        bucket,
        reported(1.0),
        absent(MetricStatus::NotApplicable),
        absent(MetricStatus::NotObserved),
    )]);
    let with_movement = |values: &[f64]| -> Vec<(u64, SyntheticSeed)> {
        separations(values)
            .into_iter()
            .map(|(seed, mut values)| {
                values.movements = movement.clone();
                (seed, values)
            })
            .collect()
    };
    // Standard -> Fine is a 1% change of the separation mean, inside the
    // tolerance; Fast -> Standard is a large one.
    let fixture = fixture(
        &scratch,
        "links",
        [
            &with_movement(&[1.0, 2.0]),
            &with_movement(&[1.58, 2.58]),
            &with_movement(&[1.59, 2.59]),
        ],
    );
    let report = fixture
        .report()
        .expect("three fidelities over one bank converge");

    assert_eq!(report.metric_definition_version, METRIC_DEFINITION_VERSION);
    assert_eq!(
        report
            .fidelities
            .iter()
            .map(|fidelity| fidelity.fidelity.as_str())
            .collect::<Vec<_>>(),
        FIDELITIES
    );
    assert_eq!(report.scenario.id, "synthetic");
    assert_eq!(report.scenario.content_sha256, shared_scenario_sha256());
    assert_eq!(
        report.seed_bank.path,
        fixture.bank_path.display().to_string()
    );
    assert_eq!(report.seed_bank.content_sha256, fixture.bank.content_sha256);
    assert!(
        !report.movement_slices.is_empty(),
        "a movement slice is reported"
    );

    for (key, sensitivity) in every_sensitivity(&report) {
        assert_eq!(
            sensitivity.metric_definition_version, METRIC_DEFINITION_VERSION,
            "'{key}' reports another definition revision"
        );
        assert!(
            !sensitivity.unit.is_empty(),
            "'{key}' reports no unit for its values"
        );
        assert_eq!(
            sensitivity
                .fidelities
                .iter()
                .map(|value| value.fidelity.clone())
                .collect::<Vec<_>>(),
            FIDELITIES.to_vec(),
            "'{key}' orders its fidelities by the declared preset order"
        );
        assert_eq!(
            sensitivity
                .refinements
                .iter()
                .map(|step| (step.from.clone(), step.to.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("fast".to_owned(), "standard".to_owned()),
                ("standard".to_owned(), "fine".to_owned())
            ],
            "'{key}' orders its refinement steps Fast -> Standard -> Fine"
        );
        assert_eq!(
            sensitivity.verdict,
            ConvergenceVerdict::of(sensitivity.refinements[1].materially_sensitive),
            "'{key}'s verdict is its standard-to-fine reading"
        );

        for (index, value) in sensitivity.fidelities.iter().enumerate() {
            let fidelity = &report.fidelities[index];
            let Some(distribution) = &value.value else {
                continue;
            };
            assert_eq!(
                distribution.metric_definition_version, METRIC_DEFINITION_VERSION,
                "'{key}' at {} reports another definition revision",
                value.fidelity
            );
            assert_eq!(
                distribution.manifests.len(),
                distribution.reported_seeds.len(),
                "'{key}' at {} links every reported seed to a manifest",
                value.fidelity
            );
            let known: BTreeSet<&String> = fidelity
                .seeds
                .iter()
                .map(|seed| &seed.manifest_sha256)
                .collect();
            for manifest in &distribution.manifests {
                assert!(
                    known.contains(manifest),
                    "'{key}' at {} links to a manifest its batch does not hold",
                    value.fidelity
                );
            }
            assert!(
                distribution
                    .reported_seeds
                    .windows(2)
                    .all(|pair| pair[0] < pair[1]),
                "'{key}' at {} orders its reported seeds ascending",
                value.fidelity
            );
            assert_eq!(
                distribution.mean.is_none(),
                distribution.reported_seeds.is_empty(),
                "'{key}' at {} reports a mean exactly when it reports a seed",
                value.fidelity
            );
        }

        for (index, step) in sensitivity.refinements.iter().enumerate() {
            let coarser = &report.fidelities[index];
            let finer = &report.fidelities[index + 1];
            let Some(paired) = &step.paired else {
                continue;
            };
            assert_eq!(paired.metric_definition_version, METRIC_DEFINITION_VERSION);
            assert_eq!(paired.count, paired.paired_seeds.len());
            assert_eq!(paired.a_manifests.len(), paired.count);
            assert_eq!(paired.b_manifests.len(), paired.count);
            let finer_manifests: BTreeSet<&String> = finer
                .seeds
                .iter()
                .map(|seed| &seed.manifest_sha256)
                .collect();
            let coarser_manifests: BTreeSet<&String> = coarser
                .seeds
                .iter()
                .map(|seed| &seed.manifest_sha256)
                .collect();
            for manifest in &paired.a_manifests {
                assert!(
                    finer_manifests.contains(manifest),
                    "'{key}' links a {} manifest to the {} side of '{}' -> '{}'",
                    finer.fidelity,
                    step.to,
                    step.from,
                    step.to
                );
            }
            for manifest in &paired.b_manifests {
                assert!(
                    coarser_manifests.contains(manifest),
                    "'{key}' links a {} manifest to the {} side of '{}' -> '{}'",
                    coarser.fidelity,
                    step.from,
                    step.from,
                    step.to
                );
            }
            assert_eq!(
                paired.count + paired.unpaired.len(),
                report.fidelities[0].seeds.len(),
                "'{key}' accounts for every paired seed at '{}' -> '{}'",
                step.from,
                step.to
            );
        }
    }

    // The fixture's standard-to-fine change is inside the tolerance, so the
    // verdict is converged while the fast-to-standard change is not.
    let separation = &report.metrics["minimum_separation_m"];
    assert_eq!(separation.verdict, ConvergenceVerdict::Converged);
    assert_eq!(separation.refinements[0].materially_sensitive, Some(true));
    assert!(separation.refinements[0].relative_change.unwrap() > CONVERGENCE_TOLERANCE);
}

/// Building the report twice from the same batches produces the same bytes, and
/// a batch whose runs are listed in another order changes nothing but its own
/// manifest hash.
#[test]
fn the_ordering_is_deterministic() {
    let scratch = Scratch::new("ordering");
    let fixture = fixture(
        &scratch,
        "ordering",
        [
            &separations(&[1.0, 2.0]),
            &separations(&[1.5, 2.5]),
            &separations(&[1.6, 2.6]),
        ],
    );
    let report = fixture
        .report()
        .expect("three fidelities over one bank converge");
    assert_eq!(
        report,
        fixture.report().expect("the report is deterministic")
    );

    // The reported order is the declared one and every seed list ascends.
    assert_eq!(
        report
            .fidelities
            .iter()
            .map(|fidelity| fidelity.fidelity.clone())
            .collect::<Vec<_>>(),
        FIDELITIES.to_vec()
    );
    assert!(
        report
            .metrics
            .keys()
            .zip(report.metrics.keys().skip(1))
            .all(|(left, right)| left < right)
    );
    for (key, sensitivity) in every_sensitivity(&report) {
        for value in &sensitivity.fidelities {
            if let Some(distribution) = &value.value {
                assert!(
                    distribution
                        .not_observed_seeds
                        .windows(2)
                        .all(|pair| pair[0] < pair[1]),
                    "'{key}' orders its status seeds ascending"
                );
            }
        }
    }

    // The same three batches with one run list reversed differ in that batch
    // manifest's bytes alone.
    let reversed = fixture.side(
        "reversed",
        1,
        &separations(&[1.5, 2.5]),
        &Deviation {
            reverse_runs: true,
            ..Deviation::default()
        },
    );
    let mut normalized = converge_batches(
        &fixture.roots[0],
        &reversed,
        &fixture.roots[2],
        &fixture.bank_path,
        CONVERGENCE_TOLERANCE,
    )
    .expect("a reversed run list still converges");
    assert_ne!(
        normalized.fidelities[1].batch.sha256,
        report.fidelities[1].batch.sha256
    );
    // The root and the batch manifest's own bytes are the only inputs the
    // reversal changes; every reported number is the same.
    normalized.fidelities[1].root = report.fidelities[1].root.clone();
    normalized.fidelities[1].batch.sha256 = report.fidelities[1].batch.sha256.clone();
    assert_eq!(report, normalized);
}

/// Three batches that do not share one step, duration, or scenario are refused
/// rather than reported as a refinement.
#[test]
fn batches_that_do_not_share_one_step_duration_or_scenario_are_refused() {
    let scratch = Scratch::new("refusals");
    let fixture = fixture(
        &scratch,
        "refusals",
        [
            &separations(&[0.5, 1.5]),
            &separations(&[1.0, 2.0]),
            &separations(&[1.5, 2.5]),
        ],
    );
    assert!(fixture.report().is_ok(), "the well-formed triple converges");

    // A batch at another step is not the fidelity it was passed as.
    let wrong_step = fixture.side(
        "wrong-step",
        0,
        &separations(&[0.5, 1.5]),
        &Deviation {
            step_s: Some(0.05),
            ..Deviation::default()
        },
    );
    let error = refusal(converge_batches(
        &wrong_step,
        &fixture.roots[1],
        &fixture.roots[2],
        &fixture.bank_path,
        CONVERGENCE_TOLERANCE,
    ));
    assert!(
        matches!(
            error,
            ConvergenceError::FidelityStep {
                fidelity: "fast",
                ..
            }
        ),
        "unexpected refusal: {error}"
    );

    // A batch that covers another simulated duration is not a refinement of the
    // others: the declared rule holds the duration constant, not the tick count.
    let wrong_duration = fixture.side(
        "wrong-duration",
        0,
        &separations(&[0.5, 1.5]),
        &Deviation {
            ticks: Some(31),
            ..Deviation::default()
        },
    );
    let error = refusal(converge_batches(
        &wrong_duration,
        &fixture.roots[1],
        &fixture.roots[2],
        &fixture.bank_path,
        CONVERGENCE_TOLERANCE,
    ));
    match error {
        ConvergenceError::FidelityDuration {
            fidelity,
            ticks,
            expected_ticks,
            ..
        } => {
            assert_eq!(fidelity, "fast");
            assert_eq!(ticks, 31);
            assert_eq!(expected_ticks, 30);
        }
        other => panic!("unexpected refusal: {other}"),
    }

    // Two scenarios are two experiments, not one refinement.
    let wrong_scenario = fixture.side(
        "wrong-scenario",
        0,
        &separations(&[0.5, 1.5]),
        &Deviation {
            scenario_sha256: Some("b".repeat(64)),
            ..Deviation::default()
        },
    );
    let error = refusal(converge_batches(
        &wrong_scenario,
        &fixture.roots[1],
        &fixture.roots[2],
        &fixture.bank_path,
        CONVERGENCE_TOLERANCE,
    ));
    assert!(
        matches!(
            error,
            ConvergenceError::ScenarioMismatch {
                fidelity: "fast",
                ..
            }
        ),
        "unexpected refusal: {error}"
    );

    // Two banks cannot be paired, which the refinement step proves.
    let other_bank = fixture.side(
        "other-bank",
        2,
        &separations(&[1.5, 2.5]),
        &Deviation {
            bank_sha256: Some("f".repeat(64)),
            ..Deviation::default()
        },
    );
    let error = refusal(converge_batches(
        &fixture.roots[0],
        &fixture.roots[1],
        &other_bank,
        &fixture.bank_path,
        CONVERGENCE_TOLERANCE,
    ));
    assert!(
        matches!(error, ConvergenceError::Compare(_)),
        "unexpected refusal: {error}"
    );
}

/// The command runs one scenario at the three declared fidelities over one seed
/// bank, writes the report to its default path, and mutates no run artifact.
#[test]
fn the_converge_command_runs_three_fidelities_over_one_bank_and_mutates_nothing() {
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

    let scenario = repo_path(SCENARIO);
    let scenario_arg = scenario.to_str().expect("the scenario path is UTF-8");
    let ticks = STANDARD_TICKS.to_string();
    let output = scratch.cli(&[
        "converge",
        scenario_arg,
        "--seed-bank",
        "bank.json",
        "--out-root",
        "runs",
        "--ticks",
        &ticks,
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("convergence:"),
        "the command reports what it wrote: {}",
        stderr(&output)
    );

    let path = scratch.path(CONVERGENCE_FILE);
    let bytes = std::fs::read(&path).expect("the default output path is convergence.json");
    let report: ConvergenceReport = serde_json::from_slice(&bytes).expect("the report is JSON");

    // Each declared fidelity ran the declared rule's tick count over the one bank
    // and the one scenario.
    assert_eq!(
        report
            .fidelities
            .iter()
            .map(|fidelity| (fidelity.fidelity.as_str(), fidelity.step_s, fidelity.ticks))
            .collect::<Vec<_>>(),
        vec![
            ("fast", 0.1, 30),
            ("standard", 0.05, 60),
            ("fine", 0.02, 150)
        ]
    );
    assert!(report.fidelities.iter().all(|fidelity| {
        fidelity
            .seeds
            .iter()
            .map(|seed| seed.seed)
            .collect::<Vec<_>>()
            == vec![0, 1]
    }));
    assert_eq!(report.scenario.id, "car_following_v1");
    assert_eq!(report.seed_bank.path, "bank.json");
    assert_eq!(
        report.seed_bank.content_sha256,
        file_sha256(&scratch.path("bank.json"))
    );
    assert!(!report.metrics.is_empty());
    assert!(report.mode_pair_slices.contains_key("vehicle_vehicle"));
    assert!(
        report
            .movement_slices
            .contains_key("movement:through|movement:through"),
        "the benchmark's movement slice is reported"
    );

    // Every metric's verdict is its standard-to-fine reading, and a value no run
    // reported is a status rather than a zero.
    let mut sensitive = 0;
    let mut inconclusive = 0;
    for (key, sensitivity) in every_sensitivity(&report) {
        assert_eq!(
            sensitivity.verdict,
            ConvergenceVerdict::of(sensitivity.refinements[1].materially_sensitive),
            "'{key}'s verdict is its standard-to-fine reading"
        );
        match sensitivity.verdict {
            ConvergenceVerdict::MateriallySensitive => sensitive += 1,
            ConvergenceVerdict::Inconclusive => inconclusive += 1,
            ConvergenceVerdict::Converged => {}
        }
        for value in &sensitivity.fidelities {
            let Some(distribution) = &value.value else {
                continue;
            };
            assert_eq!(
                distribution.metric_definition_version,
                METRIC_DEFINITION_VERSION
            );
            assert_eq!(distribution.count, distribution.reported_seeds.len());
            if distribution.count == 0 {
                assert_eq!(
                    distribution.mean, None,
                    "'{key}' at {} reads a missing value as zero",
                    value.fidelity
                );
            }
        }
    }
    assert!(
        sensitive > 0,
        "the benchmark's refinement must flag a material sensitivity"
    );
    assert!(
        inconclusive > 0,
        "a metric no seed reports at both steps must be inconclusive"
    );

    // Re-invoking the command resumes the batches: no run artifact changes and
    // the report is byte-identical.
    let batch_roots: Vec<PathBuf> = report
        .fidelities
        .iter()
        .map(|fidelity| scratch.path(&fidelity.root))
        .collect();
    let before: Vec<Vec<(String, String)>> =
        batch_roots.iter().map(|root| tree_hashes(root)).collect();
    let again = scratch.cli(&[
        "converge",
        scenario_arg,
        "--seed-bank",
        "bank.json",
        "--out-root",
        "runs",
        "--ticks",
        &ticks,
    ]);
    assert_eq!(again.status.code(), Some(0), "stderr: {}", stderr(&again));
    assert_eq!(std::fs::read(&path).expect("the report is read"), bytes);
    assert_eq!(
        before,
        batch_roots
            .iter()
            .map(|root| tree_hashes(root))
            .collect::<Vec<_>>()
    );

    // Writing the report to stdout produces the same bytes.
    let to_stdout = scratch.cli(&[
        "converge",
        scenario_arg,
        "--seed-bank",
        "bank.json",
        "--out-root",
        "runs",
        "--ticks",
        &ticks,
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

    // A fidelity root that already holds another configuration is refused rather
    // than re-run into the wrong specification, and nothing is written.
    let refused = scratch.cli(&[
        "converge",
        scenario_arg,
        "--seed-bank",
        "bank.json",
        "--out-root",
        "runs",
        "--ticks",
        "61",
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
        "a refused convergence writes no report"
    );
    assert_eq!(
        before,
        batch_roots
            .iter()
            .map(|root| tree_hashes(root))
            .collect::<Vec<_>>()
    );
}
