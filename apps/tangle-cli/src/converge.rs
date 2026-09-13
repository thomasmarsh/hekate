//! The `converge` command: the Fast/Standard/Fine sensitivity report.
//!
//! A convergence report answers one question for every metric a run reports:
//! does the value survive the fixed step refining? Three batches of one
//! scenario, one per Phase 1 fidelity preset ([`PRESETS`]: Fast 100 ms, Standard
//! 50 ms, Fine 20 ms), run one common-random-number seed bank over one simulated
//! duration, and this module turns their artifacts into `convergence.json`: for
//! every metric the across-seed value at each fidelity, the refinement change
//! from Fast to Standard and from Standard to Fine, and a convergence verdict
//! against one declared tolerance, disaggregated by mode pair (`ModePair`) and
//! by movement — the `MovementId` / `PedestrianRouteId` union
//! [[DEF-004-metric-definition-v1]] chose — wherever the metric supports it.
//!
//! ## The declared tolerance
//!
//! The tolerance is **per metric**, and each metric's record carries the one it
//! was read against ([`MetricSensitivity::tolerance`]). Every metric's tolerance
//! is the same shape — a relative part on the coarser value plus an absolute
//! part in the metric's own unit — and the part that differs is the metric's
//! class:
//!
//! - a **continuous** metric (`seconds`, `metres`, `agents_per_second`) is read
//!   against [`CONVERGENCE_TOLERANCE`] (5%) of its coarser across-seed value and
//!   no absolute part, so a change has to be a real fraction of the value to
//!   count;
//! - a **count** metric (`records`, `agents`) adds
//!   [`CONVERGENCE_COUNT_TOLERANCE`] (one unit) to that relative part, because a
//!   count's smallest possible change is one whole record or one whole agent.
//!
//! The absolute part is what the single global relative tolerance could not
//! express: a count metric whose coarser value is `0` has no relative scale at
//! all, so `0 -> 1` used to read as an unbounded relative change and a material
//! sensitivity. One record of drift is now noise, and the report states the
//! class, the relative part, and the absolute part it used rather than hiding
//! the rule.
//!
//! The measure is
//!
//! ```text
//! |value(fine) - value(coarse)| > absolute + relative x |value(coarse)|
//! ```
//!
//! where each value is the across-seed [mean](MetricDistribution::mean) over the
//! shared bank ([`TOLERANCE_REFERENCE`]) and the change itself is the paired mean
//! difference `fine - coarse` over that bank, so it carries the paired
//! confidence interval and the per-seed pairing the seed bank makes possible. The
//! relative change `|change| / |reference|` is reported beside it
//! ([`TOLERANCE_MEASURE`]), and is absent when the coarser value is zero and the
//! change is not, because that ratio has no scale: the absolute part decides
//! there.
//!
//! A metric whose **standard-to-fine** change exceeds its tolerance — strictly
//! greater, not equal ([`VERDICT_REFINEMENT`]) — is
//! [`ConvergenceVerdict::MateriallySensitive`]: the report names it rather than
//! hiding it behind an averaged value.
//!
//! Two cases are named rather than approximated:
//!
//! - a metric no seed reports at both steps of a refinement — not applicable,
//!   not observed, or the slice does not reach that fidelity — has no change to
//!   measure: the step reports `relative_change: null` and the metric is
//!   [`ConvergenceVerdict::Inconclusive`], never a fabricated zero;
//! - a metric that is not applicable or not observed at a fidelity is reported
//!   with that status and no value, exactly as the aggregation reports it
//!   ([[DEF-004-metric-definition-v1]]'s three statuses are never conflated with
//!   a value of `0`).
//!
//! ## Values, changes, and links
//!
//! The value at each fidelity is the aggregation's own
//! [`MetricDistribution`]: the across-seed count, mean, spread, confidence
//! interval, the seeds behind each reporting status, and the `manifest_sha256`
//! of every reported seed. The change at each refinement step is the paired
//! comparison's own [`PairedDistribution`]: the paired mean difference, its
//! interval, the paired seeds, the seeds the step excludes with both fidelities'
//! statuses, and both fidelities' `manifest_sha256` lists. Every record
//! therefore carries `metric_definition_version: 2` and resolves to the run
//! manifest at each fidelity, and each fidelity's batch manifest, run
//! directories, and artifact hashes are recorded once in
//! [`Convergence::fidelities`].
//!
//! The report reuses the landed machinery instead of restating it:
//! [`aggregate_batch`] reads each fidelity's seeds, [`compare_batches`] pairs
//! each refinement step's seeds (which is what proves all three batches ran the
//! one bank the command was given, in one seed order), and the `converge`
//! command's runs go through [`run_batch`](crate::batch::run_batch) with
//! [`PRESETS`], never a second copy of the step constants.
//!
//! ## The declared fidelity rule
//!
//! Fast, Standard, and Fine differ in step size, so the fidelity rule holds the
//! *simulated duration* constant, not the tick count: fixing ticks would compare
//! different simulated horizons. [`fidelity_ticks`] gives the whole number of
//! steps a fidelity runs for the duration the Standard fidelity's tick count
//! names, and [`converge_batches`] refuses a batch whose tick count is not that
//! number — a batch run at another duration is not a refinement of the others.
//!
//! ## Determinism and immutability
//!
//! Every collection is ordered: the fidelities and the refinement steps are in
//! the declared Fast → Standard → Fine order, the metric and slice maps are
//! `BTreeMap`s, each value's seed lists are ascending, and the report reads no
//! clock and no randomness. The underlying batches go through the batch
//! machinery's resume path, so a completed run directory is never rewritten and
//! re-invoking `converge` changes no run artifact: it re-reads the same bytes
//! and rewrites the same report.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::aggregate::{
    AGENTS, AggregateError, AggregatedBatch, AggregatedSeed, Aggregation, MetricDistribution,
    RECORDS, aggregate_batch,
};
use crate::baseline::{PRESETS, Preset, ScenarioProvenance};
use crate::batch::{BATCH_MANIFEST_FILE, BatchManifest, BatchSpec};
use crate::compare::{CompareError, PairedDistribution, compare_batches};
use crate::run_metrics::METRIC_DEFINITION_VERSION;
use crate::seed_bank::{SeedBankError, SeedBankReference, read_seed_bank};

/// Version of the convergence report format.
pub const CONVERGENCE_VERSION: u32 = 1;

/// Default file name of the convergence report.
pub const CONVERGENCE_FILE: &str = "convergence.json";

/// The measure the declared tolerance's relative part is read against.
pub const TOLERANCE_MEASURE: &str = "relative_change";

/// The rule every metric's tolerance applies.
pub const TOLERANCE_RULE: &str = "|fine - coarse| > absolute + relative x |coarse|";

/// The denominator the relative change uses.
pub const TOLERANCE_REFERENCE: &str = "the across-seed mean at the coarser fidelity of the step";

/// The refinement step every metric's verdict reads.
pub const VERDICT_REFINEMENT: &str = "standard_to_fine";

/// The class of a metric whose value is a continuous quantity.
pub const CONTINUOUS_CLASS: &str = "continuous";

/// The class of a metric whose value is a count of records or agents.
pub const COUNT_CLASS: &str = "count";

/// The units read as countable: a count's smallest change is one whole unit.
pub const COUNT_UNITS: [&str; 2] = [RECORDS, AGENTS];

/// The declared convergence tolerance's relative part: a metric whose
/// refinement change exceeds this fraction of its coarser across-seed value is
/// materially sensitive, before its class's absolute part is added.
///
/// 5% is a declared engineering default (the Phase 1 fidelity presets are
/// themselves "provisional engineering defaults, not calibrated scientific
/// claims"), chosen so a refinement change an experiment's conclusion would turn
/// on is reported rather than averaged away. It is recorded in every report's
/// [`Tolerance`] block and in every metric's own [`MetricTolerance`], so a reader
/// never has to guess the threshold a verdict used.
pub const CONVERGENCE_TOLERANCE: f64 = 0.05;

/// The declared absolute part of a countable metric's tolerance, in the metric's
/// own unit: one record or one agent.
///
/// A count's smallest possible change is one whole unit, so one unit of drift is
/// the count-metric noise floor the global relative tolerance could not express;
/// a change of more than one unit has to clear the relative part as well.
pub const CONVERGENCE_COUNT_TOLERANCE: f64 = 1.0;

/// The Standard fidelity preset's name, whose step fixes the one simulated
/// duration every fidelity covers.
const STANDARD_FIDELITY: &str = "standard";

/// The convergence verdict for one metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceVerdict {
    /// Every refinement step's change is within the metric's tolerance.
    Converged,
    /// The standard-to-fine change exceeds the metric's tolerance: the metric is
    /// materially sensitive to the fixed step.
    MateriallySensitive,
    /// No change could be measured: no seed reported a value at both steps of
    /// the refinement the verdict reads.
    Inconclusive,
}

impl ConvergenceVerdict {
    /// The verdict one refinement step's tolerance reading implies: a step that
    /// exceeds the metric's tolerance is materially sensitive, a step within it
    /// is converged, and a step with no measurable change is inconclusive.
    pub fn of(materially_sensitive: Option<bool>) -> Self {
        match materially_sensitive {
            Some(true) => Self::MateriallySensitive,
            Some(false) => Self::Converged,
            None => Self::Inconclusive,
        }
    }
}

/// The tolerance one metric's verdict was read against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricTolerance {
    /// The metric's class, [`CONTINUOUS_CLASS`] or [`COUNT_CLASS`].
    pub class: String,
    /// The relative part of the tolerance: the fraction of the coarser value.
    pub relative: f64,
    /// The absolute part of the tolerance, in the metric's own unit; `0` for a
    /// continuous metric.
    pub absolute: f64,
}

impl MetricTolerance {
    /// The tolerance a metric reported in `unit` is read against, with
    /// `relative` as the declared relative part.
    pub fn of(unit: &str, relative: f64) -> Self {
        let countable = COUNT_UNITS.contains(&unit);
        Self {
            class: match countable {
                true => COUNT_CLASS,
                false => CONTINUOUS_CLASS,
            }
            .to_owned(),
            relative,
            absolute: match countable {
                true => CONVERGENCE_COUNT_TOLERANCE,
                false => 0.0,
            },
        }
    }

    /// The change at or below which a metric with a coarser value of
    /// `reference` is converged.
    fn scale(&self, reference: f64) -> f64 {
        self.absolute + self.relative * reference.abs()
    }
}

/// The documented tolerance every verdict in the report was read against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    /// The measure of a step's change that is reported beside the verdict,
    /// [`TOLERANCE_MEASURE`].
    pub measure: String,
    /// The rule every metric's tolerance applies, [`TOLERANCE_RULE`].
    pub rule: String,
    /// The relative part every metric's tolerance carries,
    /// [`CONVERGENCE_TOLERANCE`] unless the caller declared another.
    pub relative: f64,
    /// The absolute part a countable metric's tolerance adds, in that metric's
    /// own unit, [`CONVERGENCE_COUNT_TOLERANCE`].
    pub count_absolute: f64,
    /// The units read as countable, [`COUNT_UNITS`].
    pub count_units: Vec<String>,
    /// The denominator of the relative change, [`TOLERANCE_REFERENCE`].
    pub reference: String,
    /// The refinement step the metric verdict reads, [`VERDICT_REFINEMENT`].
    pub verdict_refinement: String,
}

impl Tolerance {
    /// The tolerance block for a declared `relative` part.
    fn of(relative: f64) -> Self {
        Self {
            measure: TOLERANCE_MEASURE.to_owned(),
            rule: TOLERANCE_RULE.to_owned(),
            relative,
            count_absolute: CONVERGENCE_COUNT_TOLERANCE,
            count_units: COUNT_UNITS
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<String>>(),
            reference: TOLERANCE_REFERENCE.to_owned(),
            verdict_refinement: VERDICT_REFINEMENT.to_owned(),
        }
    }
}

/// One fidelity's completed batch, with every link the report records for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityBatch {
    /// The fidelity preset's name: `fast`, `standard`, or `fine`.
    pub fidelity: String,
    /// The preset's fixed physics step in seconds.
    pub step_s: f64,
    /// Fixed steps every run of this fidelity advanced.
    pub ticks: u64,
    /// The batch root as the caller named it.
    pub root: String,
    /// The batch manifest this fidelity's aggregation read.
    pub batch: AggregatedBatch,
    /// The seeds aggregated, ascending, with each run's directory and the
    /// `manifest_sha256` and `metrics_sha256` of its artifacts.
    pub seeds: Vec<AggregatedSeed>,
}

/// One fidelity's value of one metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityValue {
    /// The fidelity preset's name.
    pub fidelity: String,
    /// The across-seed distribution at this fidelity, or `None` when this
    /// fidelity's aggregation carries no such slice at all: the fidelity made no
    /// observation of it, which is a status, never a zero.
    pub value: Option<MetricDistribution>,
}

/// One refinement step: the paired change from a coarser fidelity to a finer one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefinementStep {
    /// The coarser fidelity's name.
    pub from: String,
    /// The finer fidelity's name.
    pub to: String,
    /// The paired distribution of the per-seed differences `to - from` over the
    /// shared seed bank, or `None` when neither fidelity carries the slice. Its
    /// `a_manifests` are the finer fidelity's and its `b_manifests` the coarser
    /// fidelity's, because side A is the minuend.
    pub paired: Option<PairedDistribution>,
    /// The relative change `|mean(to) - mean(from)| / |mean(from)|`, absent when
    /// no seed is paired or when the coarser value is zero and the change is not,
    /// because that ratio has no scale there; the metric's tolerance decides that
    /// case by its absolute part alone.
    pub relative_change: Option<f64>,
    /// Whether this step's change exceeds this metric's tolerance: `None` when no
    /// change is measurable.
    pub materially_sensitive: Option<bool>,
}

/// One metric's value at each fidelity and its convergence across the steps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSensitivity {
    /// The metric definition revision every number here is reported at.
    pub metric_definition_version: u32,
    /// The metric's unit, as metric definition v2 fixes it.
    pub unit: String,
    /// The tolerance this metric's verdict was read against: its class, the
    /// relative part, and the absolute part in the metric's own unit.
    pub tolerance: MetricTolerance,
    /// The declared fidelity order: Fast, Standard, Fine.
    pub fidelities: Vec<FidelityValue>,
    /// The declared refinement order: Fast → Standard, Standard → Fine.
    pub refinements: Vec<RefinementStep>,
    /// The verdict on the refinement [`VERDICT_REFINEMENT`] names.
    pub verdict: ConvergenceVerdict,
}

/// One movement bucket's metric sensitivities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementSensitivity {
    /// The bucket's two movement keys, sorted lexicographically.
    pub movement_keys: [String; 2],
    /// The bucket's sensitivities, keyed by metric name.
    pub metrics: BTreeMap<String, MetricSensitivity>,
}

/// `convergence.json`: the Fast/Standard/Fine sensitivity report of one scenario
/// run at three fidelities over one seed bank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvergenceReport {
    /// The report format version.
    pub convergence_version: u32,
    /// The metric definition revision every number here is reported at.
    pub metric_definition_version: u32,
    /// The tolerance every verdict was read against.
    pub tolerance: Tolerance,
    /// The one scenario all three fidelities ran.
    pub scenario: ScenarioProvenance,
    /// The one seed bank all three fidelities ran.
    pub seed_bank: SeedBankReference,
    /// The fidelities, in the declared Fast, Standard, Fine order.
    pub fidelities: Vec<FidelityBatch>,
    /// The whole-batch sensitivities, keyed by metric name.
    pub metrics: BTreeMap<String, MetricSensitivity>,
    /// The mode slices, keyed by `ModePair` label.
    pub mode_pair_slices: BTreeMap<String, BTreeMap<String, MetricSensitivity>>,
    /// The movement slices, keyed by the pair's two movement keys.
    pub movement_slices: BTreeMap<String, MovementSensitivity>,
}

/// Failure to build a convergence report.
#[derive(Debug, thiserror::Error)]
pub enum ConvergenceError {
    /// The seed bank could not be read or is not a usable bank.
    #[error(transparent)]
    SeedBank(#[from] SeedBankError),
    /// A fidelity's batch could not be aggregated.
    #[error(transparent)]
    Aggregate(#[from] AggregateError),
    /// A refinement step's two fidelities could not be paired.
    #[error(transparent)]
    Compare(#[from] CompareError),
    /// A fidelity's batch manifest could not be read.
    #[error("cannot {action} '{path}' of the {fidelity} fidelity: {source}")]
    Io {
        /// The path the operation targeted.
        path: PathBuf,
        /// The fidelity the path belongs to.
        fidelity: &'static str,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// A fidelity's batch manifest could not be parsed.
    #[error("batch manifest '{path}' of the {fidelity} fidelity is not readable JSON: {source}")]
    BatchManifest {
        /// The batch manifest that failed to parse.
        path: PathBuf,
        /// The fidelity the manifest belongs to.
        fidelity: &'static str,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// A fidelity's batch ran another fixed step than its preset declares.
    #[error(
        "batch '{path}' of the {fidelity} fidelity runs a {observed_step_s} s step, but the {fidelity} preset's declared step is {expected_step_s} s; a convergence report compares the declared fidelities"
    )]
    FidelityStep {
        /// The fidelity the batch was expected to run.
        fidelity: &'static str,
        /// The batch manifest that disagrees.
        path: PathBuf,
        /// The step the batch recorded.
        observed_step_s: f64,
        /// The preset's declared step.
        expected_step_s: f64,
    },
    /// A fidelity's batch covers another simulated duration than the Standard
    /// batch.
    #[error(
        "batch '{path}' of the {fidelity} fidelity runs {ticks} steps of {step_s} s, but the declared fidelity rule holds the simulated duration constant: {expected_ticks} steps cover it; a convergence report compares one duration"
    )]
    FidelityDuration {
        /// The fidelity whose duration disagrees.
        fidelity: &'static str,
        /// The batch manifest that disagrees.
        path: PathBuf,
        /// The fixed step the batch recorded.
        step_s: f64,
        /// The steps the batch advanced.
        ticks: u64,
        /// The steps that cover the Standard batch's duration at this step.
        expected_ticks: u64,
    },
    /// The three fidelities did not run one scenario.
    #[error(
        "batch '{path}' of the {fidelity} fidelity ran scenario {observed}, but the {STANDARD_FIDELITY} fidelity ran {expected}; a convergence report compares one scenario"
    )]
    ScenarioMismatch {
        /// The fidelity whose scenario disagrees.
        fidelity: &'static str,
        /// The batch manifest that disagrees.
        path: PathBuf,
        /// The scenario content hash the batch recorded.
        observed: String,
        /// The scenario content hash the Standard fidelity recorded.
        expected: String,
    },
}

/// The whole number of fixed steps the declared fidelity rule runs at `step_s`
/// when the Standard fidelity's step runs `standard_ticks` steps:
/// `round(standard_ticks x standard_step / step_s)`.
///
/// The rule holds the simulated duration constant rather than the tick count,
/// because the presets differ in step size: Fast, Standard, and Fine therefore
/// cover the same simulated seconds with different step counts.
/// `baseline::capture` applies the same rule to a declared duration.
pub fn fidelity_ticks(step_s: f64, standard_ticks: u64) -> u64 {
    (standard_ticks as f64 * standard_step_s() / step_s).round() as u64
}

/// The Standard preset's fixed step, the one simulated duration every fidelity
/// covers.
fn standard_step_s() -> f64 {
    PRESETS
        .iter()
        .find(|preset| preset.name == STANDARD_FIDELITY)
        .expect("PRESETS declares the standard fidelity")
        .step_s
}

/// Read three completed fidelity batches run over one seed bank into a
/// sensitivity report.
///
/// Each root must hold a batch run at its preset's declared step, at the
/// tick count the declared fidelity rule gives it, over the one scenario and the
/// one seed bank the others ran, with every run directory holding its
/// `manifest.json` and `metrics.json`. The fidelities are read through the
/// aggregation and paired through the comparison, so a run directory that
/// changed after its batch finished is refused rather than reported. Nothing is
/// written: the caller writes the returned report.
pub fn converge_batches(
    fast_root: &Path,
    standard_root: &Path,
    fine_root: &Path,
    bank_path: &Path,
    tolerance: f64,
) -> Result<ConvergenceReport, ConvergenceError> {
    let bank = read_seed_bank(bank_path)?;
    let readings = [
        read_fidelity(PRESETS[0], fast_root)?,
        read_fidelity(PRESETS[1], standard_root)?,
        read_fidelity(PRESETS[2], fine_root)?,
    ];

    // One scenario, one duration: the three batches must differ in fidelity
    // alone. The one seed bank and the one seed order are proven by the two
    // refinement steps below, which pair all three roots through the bank.
    let standard = &readings[1];
    for reading in &readings {
        if reading.spec.scenario.content_sha256 != standard.spec.scenario.content_sha256 {
            return Err(ConvergenceError::ScenarioMismatch {
                fidelity: reading.preset.name,
                path: reading.manifest_path(),
                observed: reading.spec.scenario.content_sha256.clone(),
                expected: standard.spec.scenario.content_sha256.clone(),
            });
        }
        let expected_ticks = fidelity_ticks(reading.preset.step_s, standard.spec.ticks);
        if reading.spec.ticks != expected_ticks {
            return Err(ConvergenceError::FidelityDuration {
                fidelity: reading.preset.name,
                path: reading.manifest_path(),
                step_s: reading.preset.step_s,
                ticks: reading.spec.ticks,
                expected_ticks,
            });
        }
    }

    // Each step is the paired comparison of its finer fidelity as side A and its
    // coarser as side B, so its mean difference is `to - from`.
    let fast_to_standard = compare_batches(standard_root, fast_root, bank_path)?;
    let standard_to_fine = compare_batches(fine_root, standard_root, bank_path)?;
    let steps = [&fast_to_standard, &standard_to_fine];

    let mut metrics = BTreeMap::new();
    let mut keys: BTreeSet<&String> = BTreeSet::new();
    for reading in &readings {
        keys.extend(reading.aggregation.metrics.keys());
    }
    for key in keys {
        let values = [
            readings[0].aggregation.metrics.get(key),
            readings[1].aggregation.metrics.get(key),
            readings[2].aggregation.metrics.get(key),
        ];
        let paired = [steps[0].metrics.get(key), steps[1].metrics.get(key)];
        metrics.insert(key.clone(), sensitivity(values, paired, tolerance));
    }

    let mut mode_pair_slices = BTreeMap::new();
    let mut labels: BTreeSet<&String> = BTreeSet::new();
    for reading in &readings {
        labels.extend(reading.aggregation.mode_pair_slices.keys());
    }
    for label in labels {
        let mut slice_keys: BTreeSet<&String> = BTreeSet::new();
        for reading in &readings {
            if let Some(slice) = reading.aggregation.mode_pair_slices.get(label) {
                slice_keys.extend(slice.keys());
            }
        }
        let mut slice = BTreeMap::new();
        for key in slice_keys {
            let values = [
                readings[0]
                    .aggregation
                    .mode_pair_slices
                    .get(label)
                    .and_then(|slice| slice.get(key)),
                readings[1]
                    .aggregation
                    .mode_pair_slices
                    .get(label)
                    .and_then(|slice| slice.get(key)),
                readings[2]
                    .aggregation
                    .mode_pair_slices
                    .get(label)
                    .and_then(|slice| slice.get(key)),
            ];
            let paired = [
                steps[0]
                    .mode_pair_slices
                    .get(label)
                    .and_then(|slice| slice.get(key)),
                steps[1]
                    .mode_pair_slices
                    .get(label)
                    .and_then(|slice| slice.get(key)),
            ];
            slice.insert(key.clone(), sensitivity(values, paired, tolerance));
        }
        mode_pair_slices.insert(label.clone(), slice);
    }

    let mut movement_slices = BTreeMap::new();
    let mut buckets: BTreeSet<&String> = BTreeSet::new();
    for reading in &readings {
        buckets.extend(reading.aggregation.movement_slices.keys());
    }
    for bucket in buckets {
        let mut slice_keys: BTreeSet<&String> = BTreeSet::new();
        let mut movement_keys: Option<[String; 2]> = None;
        for reading in &readings {
            if let Some(slice) = reading.aggregation.movement_slices.get(bucket) {
                slice_keys.extend(slice.metrics.keys());
                movement_keys.get_or_insert_with(|| slice.movement_keys.clone());
            }
        }
        let mut slice = BTreeMap::new();
        for key in slice_keys {
            let values = [
                readings[0]
                    .aggregation
                    .movement_slices
                    .get(bucket)
                    .and_then(|slice| slice.metrics.get(key)),
                readings[1]
                    .aggregation
                    .movement_slices
                    .get(bucket)
                    .and_then(|slice| slice.metrics.get(key)),
                readings[2]
                    .aggregation
                    .movement_slices
                    .get(bucket)
                    .and_then(|slice| slice.metrics.get(key)),
            ];
            let paired = [
                steps[0]
                    .movement_slices
                    .get(bucket)
                    .and_then(|slice| slice.metrics.get(key)),
                steps[1]
                    .movement_slices
                    .get(bucket)
                    .and_then(|slice| slice.metrics.get(key)),
            ];
            slice.insert(key.clone(), sensitivity(values, paired, tolerance));
        }
        movement_slices.insert(
            bucket.clone(),
            MovementSensitivity {
                movement_keys: movement_keys
                    .expect("a slice key is read from the fidelity that carries it"),
                metrics: slice,
            },
        );
    }

    Ok(ConvergenceReport {
        convergence_version: CONVERGENCE_VERSION,
        metric_definition_version: METRIC_DEFINITION_VERSION,
        tolerance: Tolerance::of(tolerance),
        scenario: standard.spec.scenario.clone(),
        seed_bank: SeedBankReference {
            path: bank_path.display().to_string(),
            content_sha256: bank.content_sha256,
        },
        fidelities: readings
            .into_iter()
            .map(|reading| FidelityBatch {
                fidelity: reading.preset.name.to_owned(),
                step_s: reading.preset.step_s,
                ticks: reading.spec.ticks,
                root: reading.root.display().to_string(),
                batch: reading.aggregation.batch,
                seeds: reading.aggregation.seeds,
            })
            .collect(),
        metrics,
        mode_pair_slices,
        movement_slices,
    })
}

/// One fidelity's batch as read: its preset, its aggregation, and the
/// specification the declared fidelity rule is checked against.
struct FidelityReading {
    /// The preset this fidelity ran.
    preset: Preset,
    /// The batch root as the caller named it.
    root: PathBuf,
    /// The batch's across-seed aggregation.
    aggregation: Aggregation,
    /// The batch specification the batch manifest recorded.
    spec: BatchSpec,
}

impl FidelityReading {
    /// The batch manifest path, for a diagnostic.
    fn manifest_path(&self) -> PathBuf {
        self.root.join(BATCH_MANIFEST_FILE)
    }
}

/// Read one fidelity's batch: its aggregation and the specification it recorded.
fn read_fidelity(preset: Preset, root: &Path) -> Result<FidelityReading, ConvergenceError> {
    let aggregation = aggregate_batch(root)?;
    let manifest_path = root.join(BATCH_MANIFEST_FILE);
    let bytes = fs::read(&manifest_path).map_err(|source| ConvergenceError::Io {
        path: manifest_path.clone(),
        fidelity: preset.name,
        action: "read batch manifest",
        source,
    })?;
    let manifest: BatchManifest =
        serde_json::from_slice(&bytes).map_err(|source| ConvergenceError::BatchManifest {
            path: manifest_path.clone(),
            fidelity: preset.name,
            source,
        })?;
    if manifest.spec.step_s != preset.step_s {
        return Err(ConvergenceError::FidelityStep {
            fidelity: preset.name,
            path: manifest_path,
            observed_step_s: manifest.spec.step_s,
            expected_step_s: preset.step_s,
        });
    }
    Ok(FidelityReading {
        preset,
        root: root.to_path_buf(),
        aggregation,
        spec: manifest.spec,
    })
}

/// Build one metric's sensitivity record.
///
/// `values` are the three fidelities' distributions in the declared fidelity
/// order and `paired` the two refinement steps' paired distributions in the
/// declared step order; either is absent where the slice does not reach that
/// fidelity or that step.
fn sensitivity(
    values: [Option<&MetricDistribution>; 3],
    paired: [Option<&PairedDistribution>; 2],
    relative: f64,
) -> MetricSensitivity {
    let unit = values
        .iter()
        .flatten()
        .next()
        .map(|distribution| distribution.unit.clone())
        .expect("a metric key is read from a fidelity that reports it");
    let tolerance = MetricTolerance::of(&unit, relative);
    let fidelities = PRESETS
        .iter()
        .zip(values)
        .map(|(preset, value)| FidelityValue {
            fidelity: preset.name.to_owned(),
            value: value.cloned(),
        })
        .collect();
    let refinements = vec![
        refinement(PRESETS[0], PRESETS[1], values[0], paired[0], &tolerance),
        refinement(PRESETS[1], PRESETS[2], values[1], paired[1], &tolerance),
    ];
    let verdict = ConvergenceVerdict::of(refinements[1].materially_sensitive);

    MetricSensitivity {
        metric_definition_version: METRIC_DEFINITION_VERSION,
        unit,
        tolerance,
        fidelities,
        refinements,
        verdict,
    }
}

/// Build one refinement step: its paired change and the metric's tolerance's
/// reading of it.
fn refinement(
    from: Preset,
    to: Preset,
    from_value: Option<&MetricDistribution>,
    paired: Option<&PairedDistribution>,
    tolerance: &MetricTolerance,
) -> RefinementStep {
    let change = paired.and_then(|distribution| distribution.mean_difference);
    let reference = from_value.and_then(|distribution| distribution.mean);
    let (relative_change, materially_sensitive) = assess(change, reference, tolerance);

    RefinementStep {
        from: from.name.to_owned(),
        to: to.name.to_owned(),
        paired: paired.cloned(),
        relative_change,
        materially_sensitive,
    }
}

/// The metric's tolerance reading of one refinement step's change.
///
/// The change is the paired mean difference `to - from` over the shared seed
/// bank and the reference is the coarser fidelity's across-seed mean. The metric
/// is materially sensitive when the change exceeds its tolerance: the absolute
/// part plus the relative part times the reference's magnitude. The reported
/// relative change `|change| / |reference|` is absent against a zero reference,
/// where that ratio has no scale, and the absolute part alone decides. A step
/// with no change at all — no seed reported a value at both fidelities — is not
/// comparable.
fn assess(
    change: Option<f64>,
    reference: Option<f64>,
    tolerance: &MetricTolerance,
) -> (Option<f64>, Option<bool>) {
    match (change, reference) {
        (Some(change), Some(reference)) => {
            let relative_change = (reference != 0.0).then(|| change.abs() / reference.abs());
            (
                relative_change,
                Some(change.abs() > tolerance.scale(reference)),
            )
        }
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declared order this module indexes and iterates is Fast, Standard,
    /// Fine, and the Standard preset is the one whose step fixes every
    /// fidelity's tick count.
    #[test]
    fn the_declared_fidelity_order_is_fast_standard_fine() {
        assert_eq!(
            PRESETS.map(|preset| preset.name),
            ["fast", "standard", "fine"]
        );
        assert_eq!(PRESETS.map(|preset| preset.step_s), [0.1, 0.05, 0.02]);
        assert_eq!(standard_step_s(), 0.05);
    }

    /// Each fidelity's tick count is the whole number of its steps that covers
    /// the duration the Standard tick count names.
    #[test]
    fn the_fidelity_tick_counts_cover_one_duration() {
        assert_eq!(fidelity_ticks(PRESETS[0].step_s, 250), 125);
        assert_eq!(fidelity_ticks(PRESETS[1].step_s, 250), 250);
        assert_eq!(fidelity_ticks(PRESETS[2].step_s, 250), 625);
        assert_eq!(fidelity_ticks(PRESETS[0].step_s, 60), 30);
        assert_eq!(fidelity_ticks(PRESETS[1].step_s, 60), 60);
        assert_eq!(fidelity_ticks(PRESETS[2].step_s, 60), 150);
    }

    /// The per-metric tolerance is a strict threshold on the change: the
    /// absolute part plus the relative part times the coarser value's magnitude.
    /// A count metric's absolute part is what lets a zero reference judge a
    /// one-record change as noise rather than as an unbounded relative change.
    #[test]
    fn the_tolerance_is_a_strict_absolute_plus_relative_threshold() {
        let continuous = MetricTolerance::of("seconds", 0.25);
        assert_eq!(continuous.class, CONTINUOUS_CLASS);
        assert_eq!(continuous.absolute, 0.0);

        // A change of exactly the tolerance is converged: the rule is "exceeds".
        assert_eq!(
            assess(Some(0.25), Some(1.0), &continuous),
            (Some(0.25), Some(false))
        );
        assert_eq!(
            assess(Some(0.250_000_1), Some(1.0), &continuous),
            (Some(0.250_000_1), Some(true))
        );
        assert_eq!(
            assess(Some(-0.5), Some(1.0), &continuous),
            (Some(0.5), Some(true)),
            "the measure is the magnitude of the change"
        );
        // A negative coarser value is a scale, not a sign: the reference is its
        // magnitude, like the change.
        assert_eq!(
            assess(Some(0.05), Some(-1.0), &continuous),
            (Some(0.05), Some(false))
        );

        // A zero reference has no relative scale, so the absolute part decides:
        // a continuous metric has none, and a count metric has one unit.
        assert_eq!(
            assess(Some(0.0), Some(0.0), &continuous),
            (None, Some(false))
        );
        assert_eq!(
            assess(Some(2.0), Some(0.0), &continuous),
            (None, Some(true))
        );
        assert_eq!(
            assess(Some(-2.0), Some(0.0), &continuous),
            (None, Some(true))
        );

        let count = MetricTolerance::of("records", 0.05);
        assert_eq!(count.class, COUNT_CLASS);
        assert_eq!(count.absolute, CONVERGENCE_COUNT_TOLERANCE);
        assert_eq!(count.scale(0.0), 1.0);
        // One record of drift is noise, from zero or from a value.
        assert_eq!(assess(Some(1.0), Some(0.0), &count), (None, Some(false)));
        assert_eq!(assess(Some(0.0), Some(0.0), &count), (None, Some(false)));
        assert_eq!(
            assess(Some(1.0), Some(10.0), &count),
            (Some(0.1), Some(false))
        );
        // More than the absolute part has to clear the relative part too.
        assert_eq!(assess(Some(2.0), Some(0.0), &count), (None, Some(true)));
        assert_eq!(
            assess(Some(2.0), Some(10.0), &count),
            (Some(0.2), Some(true))
        );
        assert_eq!(assess(Some(-2.0), Some(0.0), &count), (None, Some(true)));
        // Agents per second is a rate, not a count of agents.
        assert_eq!(
            MetricTolerance::of("agents_per_second", 0.05).class,
            CONTINUOUS_CLASS
        );
        assert_eq!(MetricTolerance::of("agents", 0.05).class, COUNT_CLASS);
        assert_eq!(COUNT_UNITS, ["records", "agents"]);

        // No change to measure is not a converged change.
        assert_eq!(assess(None, Some(1.0), &continuous), (None, None));
        assert_eq!(assess(Some(1.0), None, &continuous), (None, None));
        assert_eq!(assess(None, None, &continuous), (None, None));

        assert_eq!(
            ConvergenceVerdict::of(Some(true)),
            ConvergenceVerdict::MateriallySensitive
        );
        assert_eq!(
            ConvergenceVerdict::of(Some(false)),
            ConvergenceVerdict::Converged
        );
        assert_eq!(
            ConvergenceVerdict::of(None),
            ConvergenceVerdict::Inconclusive
        );
    }
}
