//! Contract tests for the `experiment` command and the checked-in comparison
//! report.
//!
//! The Increment 6 comparison is only evidence if the report it produces covers
//! every gate item by mode and by movement and every number in it resolves to
//! the run manifests that produced it. These tests hold that contract from the
//! outside: the checked-in report is pinned to its checked-in spec, bank, and
//! scenarios; the command is run end to end over a scratch spec and its numbers
//! are re-derived independently from the run artifacts on disk; a parallel run
//! is shown to produce the same per-run trace hashes and the same report bytes
//! as a serial one; and a spec that disagrees with itself is refused.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};
use hekate_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, CONTINUOUS_CLASS, CONVERGENCE_EVIDENCE_FILE,
    CONVERGENCE_EVIDENCE_VERSION, CONVERGENCE_RUN_DIR, CONVERGENCE_SUMMARY_FILE, COUNT_CLASS,
    ConvergenceVerdict, ExperimentConvergence, ExperimentError, ExperimentReport, ExperimentSpec,
    FINDING_DIRECTION_RULE, FindingDirection, MANIFEST_FILE, METRIC_DEFINITION_VERSION,
    METRICS_FILE, PRESETS, REPORT_FILE, REPORT_VERSION, RUN_SLICE, SECTION_ORDER, SLICE_FAMILIES,
    SamplingPolicy, SliceKind, TOLERANCE_RULE, fidelity_ticks, render_convergence_summary,
    run_experiment, run_experiment_convergence,
};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

const EXPERIMENT: &str = "experiments/increment6_signal_timing_v1/experiment.json";
const SEED_BANK: &str = "experiments/increment6_signal_timing_v1/seed_bank.json";
const REPORT: &str = "experiments/increment6_signal_timing_v1/comparison_report.json";
const EVIDENCE: &str = "experiments/increment6_signal_timing_v1/convergence_evidence.json";
const SUMMARY: &str = "experiments/increment6_signal_timing_v1/convergence_summary.md";
/// The run root the checked-in report was produced with, relative to the
/// repository root. The run directories themselves are not checked in.
const RUN_ROOT: &str = "experiments/increment6_signal_timing_v1/runs";

/// Ticks a scratch experiment runs: 3 s at the Standard step, enough for a
/// vehicle to reach the conflict region and for both variants to behave
/// differently, and short enough to keep the suite fast.
const SCRATCH_TICKS: u64 = 60;

/// The two variants' names, in the spec's declared order.
const VARIANTS_VARIANT_NAMES: [&str; 2] = ["ew_priority", "ns_priority"];

/// The two checked-in variants, in the spec's declared order.
const VARIANTS: [&str; 2] = [
    "scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5",
    "scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5",
];

/// A scratch bank of two seeds: enough to pair and to show a spread, and small
/// enough that a scratch experiment stays fast in a debug build.
const SCRATCH_SEEDS: [u64; 2] = [1, 2];

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-cli-experiment-command-{name}-{}",
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

fn read(relative: &str) -> String {
    let path = repo_path(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()))
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

fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    let mut json = serde_json::to_string_pretty(value).expect("artifact serializes");
    json.push('\n');
    std::fs::write(path, json)
        .unwrap_or_else(|error| panic!("cannot write '{}': {error}", path.display()));
}

/// The checked-in experiment spec, as the library reads it.
fn checked_in_spec() -> ExperimentSpec {
    serde_json::from_str(&read(EXPERIMENT)).expect("the experiment spec is valid JSON")
}

/// The checked-in comparison report, as the library reads it.
fn checked_in_report() -> ExperimentReport {
    serde_json::from_str(&read(REPORT)).expect("the comparison report is valid JSON")
}

/// The seeds a run table names, ascending.
fn run_seeds(report: &ExperimentReport, variant: usize) -> Vec<u64> {
    report.variants[variant]
        .runs
        .iter()
        .map(|run| run.seed)
        .collect()
}

/// Every record's seeds must account for every seed of the bank exactly once,
/// and every record must state the definition revision and a unit.
fn assert_records_are_attributed(report: &ExperimentReport, seeds: usize) {
    let mut checked = 0;
    for (section, slice, metric, record) in report.records() {
        assert_eq!(
            record.metric_definition_version, METRIC_DEFINITION_VERSION,
            "'{section}/{slice}/{metric}'"
        );
        assert!(!record.unit.is_empty(), "'{section}/{slice}/{metric}'");
        assert!(
            SECTION_ORDER.contains(&record.section.as_str()),
            "'{metric}' names the unknown section '{}'",
            record.section
        );
        for (side, distribution) in [("a", &record.a), ("b", &record.b)] {
            let Some(distribution) = distribution else {
                continue;
            };
            assert_eq!(
                distribution.count
                    + distribution.not_applicable_seeds.len()
                    + distribution.not_observed_seeds.len(),
                seeds,
                "side {side} of '{section}/{slice}/{metric}' must account for every seed"
            );
            assert_eq!(distribution.count, distribution.reported_seeds.len());
            for lists in [
                &distribution.reported_seeds,
                &distribution.not_applicable_seeds,
                &distribution.not_observed_seeds,
            ] {
                assert!(
                    lists.windows(2).all(|pair| pair[0] < pair[1]),
                    "side {side} of '{section}/{slice}/{metric}' is not ascending"
                );
            }
            assert!(
                distribution.count >= 2 || distribution.confidence_interval.is_none(),
                "a below-two-seed distribution reports no interval"
            );
        }
        if let Some(paired) = &record.paired {
            assert_eq!(paired.count, paired.paired_seeds.len());
            assert!(
                paired
                    .paired_seeds
                    .iter()
                    .all(|seed| !paired.unpaired_seeds.contains(seed))
            );
            assert_eq!(
                paired.count + paired.unpaired_seeds.len(),
                seeds,
                "the paired and unpaired seeds of '{section}/{slice}/{metric}' must cover the bank"
            );
        }
        checked += 1;
    }
    assert!(checked > 100, "only {checked} records were checked");
}

/// Both variants' run tables must resolve to the manifests and metrics artifacts
/// on disk, which is what makes every number attributable.
fn assert_run_tables_resolve(report: &ExperimentReport, roots: [&Path; 2]) {
    for (index, variant) in report.variants.iter().enumerate() {
        let root = roots[index];
        assert_eq!(variant.root, root.display().to_string());
        for run in &variant.runs {
            let directory = root.join(&run.directory);
            assert_eq!(
                run.manifest_sha256,
                file_sha256(&directory.join(MANIFEST_FILE)),
                "seed {} of '{}'",
                run.seed,
                variant.variant
            );
            assert_eq!(
                run.metrics_sha256,
                file_sha256(&directory.join(METRICS_FILE)),
                "seed {} of '{}'",
                run.seed,
                variant.variant
            );
        }
    }
}

/// The gate item names the report must carry, whatever the experiment's outcome.
fn assert_every_gate_item_is_covered(report: &ExperimentReport) {
    let sections: Vec<&str> = report
        .sections
        .iter()
        .map(|section| section.section.as_str())
        .collect();
    for item in [
        "throughput",
        "delay",
        "queues",
        "violations",
        "collisions",
        "minimum_separation",
    ] {
        assert!(
            sections.contains(&item),
            "the report must report '{item}', and does not: {sections:?}"
        );
    }
    // Time to collision and post-encroachment time are interaction metrics whose
    // every value may be not observed or not applicable; that is still a
    // reported measurement, so the item must be present.
    for item in [
        "time_to_collision",
        "post_encroachment_time",
        "near_misses",
        "events",
    ] {
        assert!(sections.contains(&item), "the report must report '{item}'");
    }
    assert_eq!(sections.first(), Some(&"throughput"));
}

/// The spec, the bank, and the report must agree on every input they name.
#[test]
fn the_checked_in_report_matches_the_checked_in_inputs() {
    let spec = checked_in_spec();
    let report = checked_in_report();
    let bank: serde_json::Value =
        serde_json::from_str(&read(SEED_BANK)).expect("the seed bank is valid JSON");
    let seeds: Vec<u64> = bank["seeds"]
        .as_array()
        .expect("the bank holds seeds")
        .iter()
        .map(|seed| seed.as_u64().expect("a seed is a number"))
        .collect();

    // The report names the spec it was built from, by path and by content hash.
    assert_eq!(report.report_version, REPORT_VERSION);
    assert_eq!(report.metric_definition_version, METRIC_DEFINITION_VERSION);
    assert_eq!(report.experiment.path, EXPERIMENT);
    assert_eq!(report.experiment.id, spec.id);
    assert_eq!(
        report.experiment.experiment_version,
        spec.experiment_version
    );
    assert_eq!(
        report.experiment.content_sha256,
        file_sha256(&repo_path(EXPERIMENT))
    );
    assert_eq!(report.experiment.content_sha256.len(), 64);

    // The run policy is the spec's, and the bank is the checked-in bank.
    assert_eq!(report.fidelity.fidelity, spec.fidelity);
    assert_eq!(report.fidelity.step_s, spec.step_s);
    assert_eq!(report.fidelity.ticks, spec.ticks);
    assert_eq!(report.fidelity.duration_s, spec.duration_s);
    assert_eq!(report.sampling, spec.sampling);
    assert_eq!(report.sampling, SamplingPolicy::default());
    assert_eq!(report.seed_bank.path, spec.seed_bank);
    assert_eq!(report.seed_bank.path, SEED_BANK);
    assert_eq!(
        report.seed_bank.content_sha256,
        file_sha256(&repo_path(SEED_BANK))
    );

    // The two sides are the spec's variants in the declared order, and the
    // first is the minuend.
    assert_eq!(report.side_a, spec.variants[0].variant);
    assert_eq!(report.side_b, spec.variants[1].variant);
    assert_eq!(report.variants.len(), 2);
    for (index, variant) in report.variants.iter().enumerate() {
        assert_eq!(variant.variant, spec.variants[index].variant);
        assert_eq!(variant.scenario.source_path, spec.variants[index].scenario);
        assert_eq!(variant.scenario.source_path, VARIANTS[index]);
        assert_eq!(
            variant.scenario.content_sha256,
            file_sha256(&repo_path(VARIANTS[index]))
        );
        assert_eq!(
            variant.root,
            format!("{RUN_ROOT}/{}", variant.variant),
            "a variant's run root is <run-root>/<variant>"
        );
        assert_eq!(variant.batch.path, BATCH_MANIFEST_FILE);
        assert_eq!(variant.batch.sha256.len(), 64);
        assert!(variant.batch.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(run_seeds(&report, index), seeds);
        for run in &variant.runs {
            for hash in [&run.manifest_sha256, &run.metrics_sha256] {
                assert_eq!(hash.len(), 64);
                assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
            }
        }
    }
    assert_ne!(
        report.variants[0].batch.sha256,
        report.variants[1].batch.sha256
    );

    // Every gate item is covered, every record is attributed, and both
    // dimensions the gate names are present.
    assert_every_gate_item_is_covered(&report);
    assert_records_are_attributed(&report, seeds.len());

    let slices = report.slices();
    let modes: BTreeSet<String> = slices
        .iter()
        .filter(|(kind, _)| *kind == SliceKind::Mode)
        .map(|(_, key)| key.clone())
        .collect();
    assert_eq!(
        modes,
        BTreeSet::from(["pedestrian".to_owned(), "vehicle".to_owned()]),
        "the report must disaggregate by mode"
    );
    assert!(slices.iter().any(|(kind, _)| *kind == SliceKind::Run));
    assert!(slices.iter().any(|(kind, _)| *kind == SliceKind::ModePair));
    assert!(
        slices
            .iter()
            .any(|(kind, _)| *kind == SliceKind::MovementPair)
    );
    assert!(
        slices
            .iter()
            .filter(|(kind, _)| *kind == SliceKind::Movement)
            .count()
            >= 2,
        "the report must disaggregate by movement"
    );

    // Every counted family is reported per mode and per movement, so the
    // collision, violation, and queue families the gate names are disaggregated.
    for family in [
        "collisions",
        "near_misses",
        "violations",
        "queue_events",
        "region_entries",
        "region_exits",
        "control_transitions",
        "yields",
        "spawns",
        "despawns",
    ] {
        let key = format!("event_counts.{family}");
        for mode in ["vehicle", "pedestrian"] {
            assert!(
                report.record(mode, &key).is_some(),
                "mode '{mode}' must report '{key}'"
            );
        }
        assert!(
            slices
                .iter()
                .filter(|(kind, _)| *kind == SliceKind::Movement)
                .any(|(_, movement)| report.record(movement, &key).is_some()),
            "a movement must report '{key}'"
        );
    }

    // The run slice carries the whole-run metric set, and one metric is
    // reported exactly once.
    let run = report
        .record(RUN_SLICE, "minimum_separation_m")
        .expect("a run");
    assert_eq!(run.section, "minimum_separation");
    assert_eq!(run.unit, "metres");
    let records = report.records();
    let mut keys = BTreeSet::new();
    for (_, slice, metric, _) in &records {
        assert!(
            keys.insert((slice.clone(), metric.clone())),
            "'{slice}/{metric}' is reported twice"
        );
    }
    let control = report
        .record("movement:ew_through", "mean_control_delay_s")
        .expect("the east-west movement's control delay");
    assert_eq!(control.section, "delay");
    assert_eq!(control.unit, "seconds");
}

/// Run the `experiment` command with `arguments` from the repository root.
fn experiment_command(spec: &Path, run_root: &Path, jobs: u64, output: &Path) -> Output {
    Command::new(CLI)
        .args([
            "experiment",
            &spec.display().to_string(),
            "--run-root",
            &run_root.display().to_string(),
            "--jobs",
            &jobs.to_string(),
            "--output",
            &output.display().to_string(),
        ])
        .current_dir(repo_path(""))
        .output()
        .expect("hekate-cli runs")
}

/// A scratch seed bank of `seeds`, written as the `seed-bank` command writes one.
fn write_scratch_bank(scratch: &Scratch, name: &str, seeds: &[u64]) -> PathBuf {
    let bank =
        hekate_cli::SeedBank::from_seeds(seeds).expect("the seeds are distinct and ascending");
    let path = scratch.path(&format!("{name}-bank.json"));
    write_json(&path, &bank);
    path
}

/// A scratch spec over the checked-in variants, a scratch bank, and a short
/// horizon, written with absolute paths so the test does not depend on its
/// working directory.
fn write_scratch_spec(scratch: &Scratch, name: &str, ticks: u64, bank: &Path) -> PathBuf {
    let path = scratch.path(&format!("{name}.json"));
    let text = format!(
        r#"{{
  "experiment_version": 1,
  "id": "scratch_{name}",
  "summary": "A scratch experiment over the checked-in variants.",
  "fidelity": "standard",
  "step_s": 0.05,
  "ticks": {ticks},
  "duration_s": {},
  "seed_bank": "{}",
  "sampling": {},
  "controlled": ["everything but the signal plan"],
  "independent_variable": "the fixed-time signal plan",
  "variants": [
    {{ "variant": "ew_priority", "scenario": "{}" }},
    {{ "variant": "ns_priority", "scenario": "{}" }}
  ]
}}
"#,
        ticks as f64 * 0.05,
        bank.display(),
        serde_json::to_string(&SamplingPolicy::default()).expect("the policy serializes"),
        repo_path(VARIANTS[0]).display(),
        repo_path(VARIANTS[1]).display(),
    );
    std::fs::write(&path, text).expect("the scratch spec is written");
    path
}

/// The command's own contract: both variants run into their own immutable run
/// directories over one bank, and the report's numbers re-derive from the run
/// artifacts on disk, disaggregated by mode and by movement.
#[test]
fn the_experiment_runs_both_variants_over_one_bank_and_reports_by_mode_and_movement() {
    let scratch = Scratch::new("run");
    let bank = write_scratch_bank(&scratch, "run", &SCRATCH_SEEDS);
    let spec = write_scratch_spec(&scratch, "run", SCRATCH_TICKS, &bank);
    let run_root = scratch.path("runs");

    let report = run_experiment(&spec, &run_root, 2).expect("the experiment runs");

    // Both variants ran, each with the bank's seeds, and the report links them.
    assert_eq!(report.variants.len(), 2);
    assert_eq!(report.side_a, "ew_priority");
    assert_eq!(report.side_b, "ns_priority");
    assert_eq!(run_seeds(&report, 0), SCRATCH_SEEDS.to_vec());
    assert_eq!(run_seeds(&report, 1), SCRATCH_SEEDS.to_vec());
    assert_eq!(report.metric_definition_version, METRIC_DEFINITION_VERSION);
    assert_eq!(
        report.seed_bank.content_sha256,
        file_sha256(&bank),
        "the report links the bank both variants ran"
    );
    assert_eq!(
        report.fidelity.ticks, SCRATCH_TICKS,
        "the report states the run policy it was produced under"
    );

    // Every number resolves to the manifests on disk, and every record is
    // attributed to the seeds behind it.
    assert_run_tables_resolve(
        &report,
        [&run_root.join("ew_priority"), &run_root.join("ns_priority")],
    );
    assert_every_gate_item_is_covered(&report);
    assert_records_are_attributed(&report, SCRATCH_SEEDS.len());

    // Both batch roots hold one immutable run directory per seed.
    for variant in ["ew_priority", "ns_priority"] {
        let root = run_root.join(variant);
        let manifest: BatchManifest = serde_json::from_str(
            &std::fs::read_to_string(root.join(BATCH_MANIFEST_FILE)).expect("readable"),
        )
        .expect("the batch manifest is JSON");
        assert_eq!(manifest.seeds, SCRATCH_SEEDS.to_vec());
        assert_eq!(manifest.runs.len(), SCRATCH_SEEDS.len());
        assert_eq!(
            manifest.seed_bank.as_ref().expect("a bank").content_sha256,
            report.seed_bank.content_sha256
        );
        for run in &manifest.runs {
            assert!(root.join(&run.directory).join(MANIFEST_FILE).exists());
        }
    }

    // Independently re-derive three numbers from the run artifacts: a run-level
    // operational value, one mode's collision count, and one movement's queue
    // events. All three are reported by mode and by movement.
    for (slice, metric, reading) in [
        (
            RUN_SLICE,
            "operational.run.throughput_agents_per_s",
            Reading::RunOperational("throughput_agents_per_s"),
        ),
        (
            "vehicle",
            "event_counts.collisions",
            Reading::ModeFamily("collisions", "vehicle"),
        ),
        (
            "movement:ew_through",
            "event_counts.queue_events",
            Reading::MovementFamily("queue_events", "movement:ew_through"),
        ),
        (
            "movement:ew_through",
            "throughput_agents_per_s",
            Reading::MovementOperational("movement:ew_through", "throughput_agents_per_s"),
        ),
    ] {
        let side_a: Vec<Option<f64>> = report.variants[0]
            .runs
            .iter()
            .map(|run| reading.value(&run_root.join("ew_priority").join(&run.directory)))
            .collect();
        let side_b: Vec<Option<f64>> = report.variants[1]
            .runs
            .iter()
            .map(|run| reading.value(&run_root.join("ns_priority").join(&run.directory)))
            .collect();
        let record = report
            .record(slice, metric)
            .unwrap_or_else(|| panic!("'{slice}' must report '{metric}'"));
        let a = record.a.as_ref().expect("side A reports it");
        let reported_a: Vec<f64> = side_a.iter().flatten().copied().collect();
        assert_eq!(
            a.reported_seeds,
            SCRATCH_SEEDS
                .iter()
                .zip(&side_a)
                .filter(|(_, value)| value.is_some())
                .map(|(seed, _)| *seed)
                .collect::<Vec<_>>(),
            "'{slice}/{metric}' reports exactly the seeds whose run reports the metric"
        );
        assert_eq!(a.count, reported_a.len());
        if !reported_a.is_empty() {
            assert!(
                (a.mean.expect("a mean") - mean(&reported_a)).abs() < 1e-9,
                "'{slice}/{metric}' reports the mean of each seed's own value"
            );
        }
        let paired = record.paired.as_ref().expect("the slices are paired");
        let differences: Vec<f64> = side_a
            .iter()
            .zip(&side_b)
            .filter_map(|(a, b)| a.zip(*b))
            .map(|(a, b)| a - b)
            .collect();
        assert_eq!(paired.count, differences.len());
        if !differences.is_empty() {
            assert!(
                (paired.mean_difference.expect("a mean difference") - mean(&differences)).abs()
                    < 1e-9,
                "'{slice}/{metric}' reports the paired difference side_a - side_b"
            );
        }
    }
}

/// One number read out of one run's `metrics.json`, for the independent
/// re-derivation. Every variant is the metric's absence-aware reading: `None`
/// when the run did not report a value.
enum Reading {
    /// One run-level operational value.
    RunOperational(&'static str),
    /// One movement's operational value.
    MovementOperational(&'static str, &'static str),
    /// One movement's counted event family.
    MovementFamily(&'static str, &'static str),
    /// One mode's counted event family.
    ModeFamily(&'static str, &'static str),
}

impl Reading {
    /// The value the metric reports for the run whose directory is `directory`,
    /// or `None` when the run reports a status rather than a value.
    fn value(&self, directory: &Path) -> Option<f64> {
        let artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(METRICS_FILE)).expect("readable"),
        )
        .expect("the run metrics are JSON");
        // A count of an absent family is the observed zero the run artifact's
        // own `by_family` set already reports, so only a bucket the run does not
        // carry at all is no observation.
        let counted = |family: &str, key: &str, slice: &str| -> Option<f64> {
            Some(
                artifact["event_counts"][slice]
                    .get(family)
                    .and_then(|counts| counts.get(key))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
            )
        };
        match self {
            Self::RunOperational(metric) => {
                artifact["operational"]["run"][*metric]["value"].as_f64()
            }
            Self::MovementOperational(movement, metric) => artifact["operational"]["by_movement"]
                .get(*movement)?
                .get(*metric)?["value"]
                .as_f64(),
            Self::MovementFamily(family, movement) => {
                artifact["operational"]["by_movement"].get(*movement)?;
                counted(family, movement, "by_family_movement")
            }
            Self::ModeFamily(family, mode) => {
                artifact["operational"]["by_mode"].get(*mode)?;
                counted(family, mode, "by_family_mode")
            }
        }
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// The command is resumable and its report is deterministic: a parallel run
/// produces the same per-run trace hashes and the same report bytes as a serial
/// one, and re-running changes no run artifact.
#[test]
fn a_parallel_experiment_matches_a_serial_one_and_rewrites_nothing() {
    let scratch = Scratch::new("parallel");
    let bank = write_scratch_bank(&scratch, "parallel", &SCRATCH_SEEDS);
    let spec = write_scratch_spec(&scratch, "parallel-spec", SCRATCH_TICKS, &bank);
    let serial_root = scratch.path("serial");
    let parallel_root = scratch.path("parallel");
    let serial_report = scratch.path("serial-report.json");
    let parallel_report = scratch.path("parallel-report.json");

    for (root, jobs, output) in [
        (&serial_root, 1, &serial_report),
        (&parallel_root, 8, &parallel_report),
    ] {
        let run = experiment_command(&spec, root, jobs, output);
        assert_eq!(
            run.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }

    // The two batch manifests are byte-identical: a batch manifest is a function
    // of the specification and the runs, never of how many workers ran them.
    let mut trace_hashes = Vec::new();
    for variant in ["ew_priority", "ns_priority"] {
        let serial =
            std::fs::read(serial_root.join(variant).join(BATCH_MANIFEST_FILE)).expect("readable");
        let parallel =
            std::fs::read(parallel_root.join(variant).join(BATCH_MANIFEST_FILE)).expect("readable");
        assert_eq!(
            serial, parallel,
            "variant '{variant}' differs between a serial and a parallel run"
        );
        let manifest: BatchManifest = serde_json::from_slice(&serial).expect("batch JSON");
        trace_hashes.push(
            manifest
                .runs
                .iter()
                .map(|run| run.trace_sha256.clone())
                .collect::<Vec<_>>(),
        );
    }
    // The two variants differ at every seed: the independent variable is
    // observable in the recorded trace hashes.
    assert_eq!(trace_hashes[0].len(), SCRATCH_SEEDS.len());
    assert!(
        trace_hashes[0]
            .iter()
            .zip(&trace_hashes[1])
            .all(|(a, b)| a != b),
        "the two variants must not share a per-run trace hash"
    );

    // The two reports differ in nothing but the run root they name: everything
    // derived from the runs is identical, field for field.
    let serial: ExperimentReport =
        serde_json::from_str(&std::fs::read_to_string(&serial_report).expect("readable"))
            .expect("the report is JSON");
    let parallel: ExperimentReport =
        serde_json::from_str(&std::fs::read_to_string(&parallel_report).expect("readable"))
            .expect("the report is JSON");
    assert_eq!(serial.sections, parallel.sections);
    for index in 0..2 {
        assert_eq!(serial.variants[index].batch, parallel.variants[index].batch);
        assert_eq!(serial.variants[index].runs, parallel.variants[index].runs);
        assert_eq!(
            serial.variants[index].scenario,
            parallel.variants[index].scenario
        );
    }
    assert_ne!(serial.variants[0].root, parallel.variants[0].root);

    // Re-running a completed experiment rewrites the report byte for byte and
    // changes no run artifact: a completed run directory is never rewritten.
    let before = tree_hashes(&serial_root);
    let report_before = std::fs::read(&serial_report).expect("readable");
    let rerun = experiment_command(&spec, &serial_root, 8, &serial_report);
    assert_eq!(
        rerun.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&rerun.stderr)
    );
    assert_eq!(tree_hashes(&serial_root), before);
    assert_eq!(
        std::fs::read(&serial_report).expect("readable"),
        report_before
    );
}

/// A spec that disagrees with itself, or names fewer than two variants, is
/// refused rather than run.
#[test]
fn a_spec_that_cannot_be_compared_is_refused() {
    let scratch = Scratch::new("refused");
    let run_root = scratch.path("runs");
    let bank = write_scratch_bank(&scratch, "refused", &SCRATCH_SEEDS);
    let spec = write_scratch_spec(&scratch, "refused", SCRATCH_TICKS, &bank);
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&spec).expect("readable"))
            .expect("the spec is JSON");

    // One variant: there is nothing to compare against.
    let mut one = document.clone();
    one["variants"].as_array_mut().expect("variants").pop();
    let path = scratch.path("one.json");
    write_json(&path, &one);
    assert!(matches!(
        run_experiment(&path, &run_root, 1),
        Err(ExperimentError::Variants { variants: 1, .. })
    ));

    // A repeated variant name is ambiguous, not a second experiment.
    let mut repeated = document.clone();
    repeated["variants"][1]["variant"] = serde_json::Value::from("ew_priority");
    let path = scratch.path("repeated.json");
    write_json(&path, &repeated);
    assert!(matches!(
        run_experiment(&path, &run_root, 1),
        Err(ExperimentError::DuplicateVariant { variant, .. }) if variant == "ew_priority"
    ));

    // The spec's own tick count, step, and duration must agree.
    let mut inconsistent = document.clone();
    inconsistent["duration_s"] = serde_json::Value::from(99.0);
    let path = scratch.path("inconsistent.json");
    write_json(&path, &inconsistent);
    assert!(matches!(
        run_experiment(&path, &run_root, 1),
        Err(ExperimentError::SpecDuration { .. })
    ));

    // A spec that is not JSON, and one whose scenario is missing, fail with the
    // path that caused it.
    let path = scratch.path("malformed.json");
    std::fs::write(&path, "{ not json").expect("the malformed spec is written");
    assert!(matches!(
        run_experiment(&path, &run_root, 1),
        Err(ExperimentError::Spec { .. })
    ));
    let mut absent = document.clone();
    absent["variants"][0]["scenario"] = serde_json::Value::from("/nonexistent/scenario.json5");
    let path = scratch.path("absent.json");
    write_json(&path, &absent);
    assert!(matches!(
        run_experiment(&path, &run_root, 1),
        Err(ExperimentError::Scenario { variant, .. }) if variant == "ew_priority"
    ));
    assert!(matches!(
        run_experiment(&scratch.path("no-spec.json"), &run_root, 1),
        Err(ExperimentError::Io { .. })
    ));

    // Every refusal happens before a run starts, so no run directory exists.
    assert!(!run_root.exists());
}

/// The command writes exactly the library's bytes, and the checked-in report is
/// the artifact under the declared default name.
#[test]
fn the_command_writes_the_reports_library_bytes() {
    let scratch = Scratch::new("bytes");
    let bank = write_scratch_bank(&scratch, "bytes", &SCRATCH_SEEDS);
    let spec = write_scratch_spec(&scratch, "bytes", SCRATCH_TICKS, &bank);
    let run_root = scratch.path("runs");
    let output = scratch.path(REPORT_FILE);
    let report = run_experiment(&spec, &run_root, 1).expect("the experiment runs");

    let written = experiment_command(&spec, &run_root, 1, &output);
    assert_eq!(
        written.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&written.stderr)
    );
    let mut expected = serde_json::to_string_pretty(&report).expect("the report serializes");
    expected.push('\n');
    assert_eq!(
        std::fs::read_to_string(&output).expect("readable"),
        expected
    );

    // The checked-in report is that same artifact under the default name, and
    // the run directories it was produced into are not checked in.
    assert_eq!(Path::new(REPORT).file_name().expect("a name"), REPORT_FILE);
    assert_eq!(
        read("experiments/increment6_signal_timing_v1/.gitignore"),
        "runs/\n",
        "the run directories are large and regenerable, so they stay untracked"
    );
}

/// The checked-in convergence evidence, as the library reads it.
fn checked_in_evidence() -> ExperimentConvergence {
    serde_json::from_str(&read(EVIDENCE)).expect("the convergence evidence is valid JSON")
}

/// A selected finding naming the run as a whole, which every run reports.
const RUN_FINDING: &str = r#"[
    {
      "label": "Control delay over the whole run",
      "family": "run",
      "slice": "*",
      "metric": "operational.run.mean_control_delay_s"
    }
  ]"#;

/// A scratch spec with the selected findings and the run policy a test
/// declares, written with absolute paths so the test does not depend on its
/// working directory.
fn write_scratch_evidence_spec(
    scratch: &Scratch,
    name: &str,
    bank: &Path,
    selected_findings: &str,
    fidelity: &str,
    step_s: f64,
    ticks: u64,
) -> PathBuf {
    let path = scratch.path(&format!("{name}.json"));
    let text = format!(
        r#"{{
  "experiment_version": 1,
  "id": "scratch_{name}",
  "summary": "A scratch experiment over the checked-in variants.",
  "fidelity": "{fidelity}",
  "step_s": {step_s},
  "ticks": {ticks},
  "duration_s": {},
  "seed_bank": "{}",
  "sampling": {},
  "controlled": ["everything but the signal plan"],
  "independent_variable": "the fixed-time signal plan",
  "variants": [
    {{ "variant": "ew_priority", "scenario": "{}" }},
    {{ "variant": "ns_priority", "scenario": "{}" }}
  ],
  "selected_findings": {selected_findings}
}}
"#,
        ticks as f64 * step_s,
        bank.display(),
        serde_json::to_string(&SamplingPolicy::default()).expect("the policy serializes"),
        repo_path(VARIANTS[0]).display(),
        repo_path(VARIANTS[1]).display(),
    );
    std::fs::write(&path, text).expect("the scratch spec is written");
    path
}

/// Run the `experiment` command with the convergence flags from the repository
/// root.
fn experiment_convergence_command(
    spec: &Path,
    run_root: &Path,
    jobs: u64,
    output: &Path,
    convergence: &Path,
    summary: &Path,
) -> Output {
    Command::new(CLI)
        .args([
            "experiment",
            &spec.display().to_string(),
            "--run-root",
            &run_root.display().to_string(),
            "--jobs",
            &jobs.to_string(),
            "--output",
            &output.display().to_string(),
            "--convergence",
            &convergence.display().to_string(),
            "--summary",
            &summary.display().to_string(),
        ])
        .current_dir(repo_path(""))
        .output()
        .expect("hekate-cli runs")
}

/// The variant's section of the summary: its per-metric tables, up to the next
/// section.
fn summary_block<'a>(summary: &'a str, variant: &str) -> &'a str {
    let start = summary
        .find(&format!("### {variant}\n"))
        .unwrap_or_else(|| panic!("the summary has no '{variant}' section"));
    let rest = &summary[start..];
    let end = ["\n### ", "\n## "]
        .into_iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .expect("the summary ends a section");
    &rest[..end]
}

/// Every verdict count of one variant's evidence, in the declared verdict
/// order.
fn verdict_counts(
    evidence: &ExperimentConvergence,
    variant: usize,
) -> (usize, usize, usize, usize) {
    let metrics: Vec<&hekate_cli::MetricConvergence> = evidence.variants[variant]
        .slices
        .iter()
        .flat_map(|slice| slice.metrics.values())
        .collect();
    let count = |verdict: ConvergenceVerdict| {
        metrics
            .iter()
            .filter(|record| record.verdict == verdict)
            .count()
    };
    (
        metrics.len(),
        count(ConvergenceVerdict::Converged),
        count(ConvergenceVerdict::MateriallySensitive),
        count(ConvergenceVerdict::Inconclusive),
    )
}

/// The checked-in convergence evidence is a function of the checked-in spec,
/// bank, and variants: it judges every metric of every slice family at each
/// fidelity, it states each selected finding's direction, and the checked-in
/// summary is that artifact's own rendering with no metric omitted.
#[test]
fn the_checked_in_convergence_evidence_matches_the_checked_in_inputs() {
    let spec = checked_in_spec();
    let evidence = checked_in_evidence();
    let bank: serde_json::Value =
        serde_json::from_str(&read(SEED_BANK)).expect("the seed bank is valid JSON");
    let seeds: Vec<u64> = bank["seeds"]
        .as_array()
        .expect("the bank holds seeds")
        .iter()
        .map(|seed| seed.as_u64().expect("a seed is a number"))
        .collect();

    assert_eq!(evidence.evidence_version, CONVERGENCE_EVIDENCE_VERSION);
    assert_eq!(
        evidence.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(evidence.experiment.path, EXPERIMENT);
    assert_eq!(evidence.experiment.id, spec.id);
    assert_eq!(
        evidence.experiment.content_sha256,
        file_sha256(&repo_path(EXPERIMENT))
    );
    assert_eq!(evidence.variants.len(), 2);
    assert!(
        !spec.selected_findings.is_empty(),
        "the experiment must declare the findings its conclusion rests on"
    );
    assert_eq!(spec.selected_findings.len(), evidence.findings.len());

    for (index, variant) in evidence.variants.iter().enumerate() {
        assert_eq!(variant.variant, spec.variants[index].variant);
        assert_eq!(variant.scenario.source_path, spec.variants[index].scenario);
        assert_eq!(
            variant.scenario.content_sha256,
            file_sha256(&repo_path(VARIANTS[index]))
        );
        assert_eq!(variant.seed_bank.path, spec.seed_bank);
        assert_eq!(variant.seed_bank.path, SEED_BANK);
        assert_eq!(
            variant.seed_bank.content_sha256,
            file_sha256(&repo_path(SEED_BANK))
        );
        assert_eq!(variant.tolerance.rule, TOLERANCE_RULE);

        // The three declared fidelities over the one bank, at the tick counts
        // the declared fidelity rule gives the spec's Standard duration. The
        // Standard fidelity is the variant's own batch root — the batch the
        // comparison report read — and the Fast and Fine batches are the
        // convergence runs beside it.
        assert_eq!(variant.fidelities.len(), PRESETS.len());
        for (position, fidelity) in variant.fidelities.iter().enumerate() {
            let preset = PRESETS[position];
            assert_eq!(fidelity.fidelity, preset.name);
            assert_eq!(fidelity.step_s, preset.step_s);
            assert_eq!(fidelity.ticks, fidelity_ticks(preset.step_s, spec.ticks));
            assert_eq!(
                fidelity
                    .seeds
                    .iter()
                    .map(|seed| seed.seed)
                    .collect::<Vec<_>>(),
                seeds
            );
            let expected_root = match preset.name == "standard" {
                true => format!("{RUN_ROOT}/{}", variant.variant),
                false => format!(
                    "{RUN_ROOT}/{CONVERGENCE_RUN_DIR}/{}/{preset}",
                    variant.variant,
                    preset = preset.name
                ),
            };
            assert_eq!(fidelity.root, expected_root);
        }

        // Every declared family is present, in the declared order, and every
        // metric of every slice is judged at every fidelity.
        let mut families: Vec<&str> = Vec::new();
        for slice in &variant.slices {
            if families.last() != Some(&slice.family.label()) {
                families.push(slice.family.label());
            }
            assert!(!slice.metrics.is_empty(), "an empty slice is not a slice");
        }
        assert_eq!(
            families,
            SLICE_FAMILIES
                .into_iter()
                .map(hekate_cli::SliceFamily::label)
                .collect::<Vec<_>>()
        );
        let mut judged = 0;
        for slice in &variant.slices {
            for (metric, record) in &slice.metrics {
                let path = format!("{}/{metric}", slice.slice);
                assert_eq!(record.metric_definition_version, METRIC_DEFINITION_VERSION);
                assert_eq!(record.means.len(), PRESETS.len(), "{path}");
                for counts in [
                    &record.reported_seeds,
                    &record.not_applicable_seeds,
                    &record.not_observed_seeds,
                ] {
                    assert_eq!(counts.len(), PRESETS.len(), "{path}");
                }
                // Every seed is reported, not applicable, or not observed at
                // each fidelity, and a mean exists exactly when a seed reported.
                for position in 0..PRESETS.len() {
                    assert_eq!(
                        record.reported_seeds[position]
                            + record.not_applicable_seeds[position]
                            + record.not_observed_seeds[position],
                        seeds.len(),
                        "{path} must account for every seed at each fidelity"
                    );
                    assert_eq!(
                        record.means[position].is_some(),
                        record.reported_seeds[position] > 0,
                        "{path} reports a mean exactly when a seed reported a value"
                    );
                }
                assert_eq!(record.refinements.len(), PRESETS.len() - 1);
                for (position, step) in record.refinements.iter().enumerate() {
                    assert_eq!(step.from, PRESETS[position].name);
                    assert_eq!(step.to, PRESETS[position + 1].name);
                    assert_eq!(
                        step.paired_seeds + step.unpaired_seeds,
                        seeds.len(),
                        "{path} accounts for every paired seed at each step"
                    );
                }
                assert_eq!(
                    record.verdict,
                    ConvergenceVerdict::of(record.refinements[1].materially_sensitive),
                    "{path}'s verdict is its standard-to-fine reading"
                );
                // The tolerance's class is the unit's, exactly as the
                // convergence report reads it.
                let countable = matches!(record.unit.as_str(), "records" | "agents");
                assert_eq!(
                    record.tolerance.class,
                    match countable {
                        true => COUNT_CLASS,
                        false => CONTINUOUS_CLASS,
                    },
                    "{path} is judged against the wrong tolerance class"
                );
                judged += 1;
            }
        }
        assert!(
            judged > 200,
            "the evidence must judge every family's metrics, and judged {judged} for '{}'",
            variant.variant
        );
    }

    // Every selected finding resolves in both variants, its sides are those
    // variants' own means, and the direction is the one the rule gives.
    assert_eq!(evidence.findings_method.rule, FINDING_DIRECTION_RULE);
    assert_eq!(
        evidence.findings_method.fidelity,
        PRESETS[PRESETS.len() - 1].name
    );
    assert_eq!(evidence.findings_method.reference_fidelity, PRESETS[0].name);
    assert_eq!(evidence.variants[0].variant, spec.variants[0].variant);
    for (finding, selected) in evidence.findings.iter().zip(&spec.selected_findings) {
        assert_eq!(&finding.finding, selected);
        for (index, variant) in evidence.variants.iter().enumerate() {
            let metric = variant
                .slices
                .iter()
                .filter(|slice| slice.family == selected.family && slice.slice == selected.slice)
                .find_map(|slice| slice.metrics.get(&selected.metric))
                .unwrap_or_else(|| {
                    panic!(
                        "'{}' must report '{}' under the {} slice '{}'",
                        variant.variant,
                        selected.metric,
                        selected.family.label(),
                        selected.slice
                    )
                });
            let side = match index {
                0 => &finding.side_a,
                _ => &finding.side_b,
            };
            assert_eq!(side, &metric.means);
        }
        let difference: Vec<Option<f64>> = finding
            .side_a
            .iter()
            .zip(&finding.side_b)
            .map(|(a, b)| a.zip(*b).map(|(a, b)| a - b))
            .collect();
        for (recorded, expected) in finding.difference.iter().zip(&difference) {
            match (recorded, expected) {
                (Some(recorded), Some(expected)) => assert!(
                    (recorded - expected).abs() <= 1e-9 * expected.abs().max(1.0),
                    "{recorded} differs from the difference {expected}"
                ),
                (None, None) => {}
                other => panic!("the difference is read from the two sides: {other:?}"),
            }
        }
        let expected = match (difference[0], difference[PRESETS.len() - 1]) {
            (Some(reference), Some(judged)) if reference != 0.0 && judged != 0.0 => {
                match reference.signum() == judged.signum() {
                    true => FindingDirection::Stable,
                    false => FindingDirection::Flipped,
                }
            }
            _ => FindingDirection::Inconclusive,
        };
        assert_eq!(finding.direction, expected, "'{}'", selected.label);
    }
    // The comparison's headline findings are the ones whose direction the gate
    // reads, so at least one of them must survive the step refining.
    assert!(
        evidence
            .findings
            .iter()
            .any(|finding| finding.direction == FindingDirection::Stable),
        "no selected finding is directionally stable at Fine"
    );

    // The checked-in summary is the artifact's own rendering, it names every
    // metric of every family of every variant exactly once, and its counts are
    // the artifact's.
    let summary = read(SUMMARY);
    assert_eq!(summary, render_convergence_summary(&evidence));
    for (index, variant) in evidence.variants.iter().enumerate() {
        let block = summary_block(&summary, &variant.variant);
        let mut rows = 0;
        for slice in &variant.slices {
            for metric in slice.metrics.keys() {
                let row = format!("| `{}` | `{metric}` |", slice.slice);
                assert_eq!(
                    block.matches(&row).count(),
                    1,
                    "'{}/{metric}' must be one row of the '{}' summary",
                    slice.slice,
                    variant.variant
                );
                rows += 1;
            }
        }
        let (metrics, converged, sensitive, inconclusive) = verdict_counts(&evidence, index);
        assert_eq!(rows, metrics);
        assert!(
            converged + sensitive + inconclusive == metrics,
            "every metric reaches exactly one verdict"
        );
        assert!(
            sensitive > 0,
            "the evidence must report material sensitivity rather than hide it"
        );
        assert!(
            summary.contains(&format!(
                "| `{}` | {metrics} | {converged} | {sensitive} | {inconclusive} |",
                variant.variant
            )),
            "the summary's counts must be the artifact's"
        );
    }
}

/// The command runs each variant's Fast and Fine batches beside its Standard
/// one and writes the convergence evidence and its summary: the Standard
/// fidelity is the variant's own batch root — the comparison's own batch — the
/// evidence and the summary are a function of the runs, and re-invoking the
/// command changes no run artifact and rewrites both byte for byte.
#[test]
fn the_experiment_command_runs_the_convergence_evidence_and_summary() {
    let scratch = Scratch::new("convergence");
    let bank = write_scratch_bank(&scratch, "convergence", &SCRATCH_SEEDS);
    let spec = write_scratch_evidence_spec(
        &scratch,
        "convergence",
        &bank,
        RUN_FINDING,
        "standard",
        0.05,
        SCRATCH_TICKS,
    );
    let run_root = scratch.path("runs");
    let report_path = scratch.path("report.json");
    let evidence_path = scratch.path(CONVERGENCE_EVIDENCE_FILE);
    let summary_path = scratch.path(CONVERGENCE_SUMMARY_FILE);

    let written = experiment_convergence_command(
        &spec,
        &run_root,
        2,
        &report_path,
        &evidence_path,
        &summary_path,
    );
    assert_eq!(
        written.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&written.stderr)
    );

    let evidence: ExperimentConvergence = serde_json::from_str(
        &std::fs::read_to_string(&evidence_path).expect("the evidence is written"),
    )
    .expect("the evidence is JSON");
    let report: ExperimentReport = serde_json::from_str(
        &std::fs::read_to_string(&report_path).expect("the report is written"),
    )
    .expect("the report is JSON");

    assert_eq!(evidence.evidence_version, CONVERGENCE_EVIDENCE_VERSION);
    assert_eq!(
        evidence.metric_definition_version,
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(evidence.experiment.id, "scratch_convergence");
    assert_eq!(
        evidence.experiment.content_sha256,
        file_sha256(&spec),
        "the evidence names the spec it was built from"
    );
    assert_eq!(evidence.findings.len(), 1);
    assert_eq!(
        evidence.findings[0].finding.label,
        "Control delay over the whole run"
    );

    for (index, variant) in evidence.variants.iter().enumerate() {
        assert_eq!(variant.variant, VARIANTS_VARIANT_NAMES[index]);
        assert_eq!(variant.seed_bank.content_sha256, file_sha256(&bank));
        assert_eq!(
            variant
                .fidelities
                .iter()
                .map(|fidelity| (
                    fidelity.fidelity.as_str(),
                    fidelity.step_s,
                    fidelity.ticks,
                    fidelity.root.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "fast",
                    0.1,
                    fidelity_ticks(0.1, SCRATCH_TICKS),
                    run_root
                        .join(CONVERGENCE_RUN_DIR)
                        .join(VARIANTS_VARIANT_NAMES[index])
                        .join("fast")
                        .display()
                        .to_string(),
                ),
                (
                    "standard",
                    0.05,
                    SCRATCH_TICKS,
                    run_root
                        .join(VARIANTS_VARIANT_NAMES[index])
                        .display()
                        .to_string(),
                ),
                (
                    "fine",
                    0.02,
                    fidelity_ticks(0.02, SCRATCH_TICKS),
                    run_root
                        .join(CONVERGENCE_RUN_DIR)
                        .join(VARIANTS_VARIANT_NAMES[index])
                        .join("fine")
                        .display()
                        .to_string(),
                ),
            ]
        );
        // The Standard fidelity is the batch the comparison report read, so the
        // two artifacts cannot disagree about it.
        assert_eq!(
            variant.fidelities[1].batch, report.variants[index].batch,
            "the Standard fidelity is the comparison's own batch"
        );
        assert!(
            !variant.slices.is_empty(),
            "a variant with no slices judges nothing"
        );
    }

    // The summary is the evidence's own rendering, and it lists every metric.
    let summary = std::fs::read_to_string(&summary_path).expect("the summary is written");
    assert_eq!(summary, render_convergence_summary(&evidence));
    for variant in &evidence.variants {
        let block = summary_block(&summary, &variant.variant);
        for slice in &variant.slices {
            for metric in slice.metrics.keys() {
                assert!(
                    block.contains(&format!("| `{}` | `{metric}` |", slice.slice)),
                    "'{}/{metric}' must be a row of the summary",
                    slice.slice
                );
            }
        }
    }

    // Re-invoking the command resumes every batch: no run artifact changes and
    // both artifacts are byte-identical.
    let before = tree_hashes(&run_root);
    let evidence_before = std::fs::read(&evidence_path).expect("readable");
    let summary_before = std::fs::read(&summary_path).expect("readable");
    let again = experiment_convergence_command(
        &spec,
        &run_root,
        8,
        &report_path,
        &evidence_path,
        &summary_path,
    );
    assert_eq!(
        again.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(tree_hashes(&run_root), before);
    assert_eq!(
        std::fs::read(&evidence_path).expect("readable"),
        evidence_before
    );
    assert_eq!(
        std::fs::read(&summary_path).expect("readable"),
        summary_before
    );
}

/// A spec that declares another fidelity than the one convergence refines, or
/// selects a finding no variant reports, is refused rather than reported as an
/// absent value.
#[test]
fn convergence_evidence_refuses_another_fidelity_or_an_absent_finding() {
    let scratch = Scratch::new("convergence-refused");
    let bank = write_scratch_bank(&scratch, "convergence-refused", &SCRATCH_SEEDS);
    let run_root = scratch.path("runs");

    // The evidence refines the Standard fidelity and reads the variant's own
    // batch as that fidelity, so a spec at another fidelity has nothing to read.
    let fine = write_scratch_evidence_spec(
        &scratch,
        "fine",
        &bank,
        RUN_FINDING,
        "fine",
        0.02,
        SCRATCH_TICKS * 5,
    );
    match run_experiment_convergence(&fine, &run_root, 1) {
        Err(ExperimentError::ConvergenceFidelity {
            fidelity,
            step_s,
            standard,
            ..
        }) => {
            assert_eq!(fidelity, "fine");
            assert_eq!(step_s, 0.02);
            assert_eq!(standard, "standard");
        }
        other => panic!("unexpected result: {other:?}"),
    }

    // A finding that names no slice and metric of a variant is a spec error, not
    // an inconclusive finding.
    let absent = write_scratch_evidence_spec(
        &scratch,
        "absent",
        &bank,
        r#"[{ "label": "A finding nothing reports", "family": "agent_movement", "slice": "movement:nowhere", "metric": "mean_control_delay_s" }]"#,
        "standard",
        0.05,
        SCRATCH_TICKS,
    );
    match run_experiment_convergence(&absent, &run_root, 1) {
        Err(ExperimentError::Finding {
            variant,
            label,
            detail,
            ..
        }) => {
            assert_eq!(variant, "ew_priority");
            assert_eq!(label, "A finding nothing reports");
            assert!(detail.contains("agent_movement"), "{detail}");
            assert!(detail.contains("movement:nowhere"), "{detail}");
            assert!(detail.contains("mean_control_delay_s"), "{detail}");
        }
        other => panic!("unexpected result: {other:?}"),
    }

    // The command exits non-zero and writes no evidence for the refused spec.
    let evidence_path = scratch.path(CONVERGENCE_EVIDENCE_FILE);
    let refused = experiment_convergence_command(
        &absent,
        &run_root,
        1,
        &scratch.path("absent-report.json"),
        &evidence_path,
        &scratch.path(CONVERGENCE_SUMMARY_FILE),
    );
    assert_eq!(refused.status.code(), Some(1), "a refusal exits 1");
    assert!(
        !evidence_path.exists(),
        "a refused convergence writes no evidence"
    );
}

/// The summary is the evidence rendered, so asking for it without asking for the
/// evidence is a usage error rather than a summary of nothing.
#[test]
fn the_summary_flag_requires_the_convergence_flag() {
    let scratch = Scratch::new("summary-requires");
    let refused = Command::new(CLI)
        .args([
            "experiment",
            "spec.json",
            "--run-root",
            "runs",
            "--summary",
            "summary.md",
        ])
        .current_dir(&scratch.dir)
        .output()
        .expect("hekate-cli runs");

    assert_eq!(refused.status.code(), Some(2), "clap rejects the usage");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("--convergence"),
        "the diagnostic names the missing flag: {stderr}"
    );
}
