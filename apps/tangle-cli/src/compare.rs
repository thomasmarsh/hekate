//! The `compare` command: the paired A/B comparison of two batches run from one
//! common-random-number (CRN) seed bank.
//!
//! A seed bank holds the ordered seeds two variants of one scenario both run
//! ([[TAS-039-phase-1-increment-5-crn-seed-bank]]), and each batch manifest
//! records the bank's path and content hash. This module reads both batches and
//! the bank, proves the two sides ran that one bank in that one order, pairs the
//! runs by seed, and for every reported metric computes the per-seed paired
//! difference
//!
//! ```text
//! d_i = value_A(seed_i) - value_B(seed_i)
//! ```
//!
//! with the mean difference and a documented paired confidence interval
//! ([[TAS-038-phase-1-increment-5-aggregation-command]] fixed the interval
//! method), disaggregated by mode pair (`ModePair`) and by movement — the
//! `MovementId` / `PedestrianRouteId` union [[DEF-004-metric-definition-v1]]
//! chose — exactly as the aggregation keys the same slices. Every number links
//! to both run manifests and to `metric_definition_version: 2`, and the artifact
//! writes `comparison.json` without touching any run artifact.
//!
//! ## Why the paired interval is narrower
//!
//! The aggregation's interval is the unpaired one: for one side alone it is
//! `mean ± t(0.975, n - 1) * s / sqrt(n)` over that side's per-seed values, and
//! its standard error carries the whole between-seed spread of that side,
//! including whatever the seeds themselves contribute. A comparison of two such
//! intervals would compare two means whose standard errors both contain that
//! shared seed-to-seed variation.
//!
//! The paired interval is computed on the differences instead, so the
//! seed-to-seed component the two variants have in common — the part that makes
//! seed 3 slower than seed 0 for both variants — cancels in `d_i` and does not
//! appear in `s_d / sqrt(n)`. What remains is the spread of the *paired* effect,
//! which is why a paired comparison over the same seeds is more sensitive than
//! comparing two unpaired intervals. The two intervals agree only when the
//! sides' per-seed values are uncorrelated, which is exactly the case the seed
//! bank exists to avoid.
//!
//! ## The interval
//!
//! The two-sided Student-t interval on the mean of the differences at a 95%
//! confidence level: `mean_difference ± t(0.975, n - 1) * s_d / sqrt(n)`, where
//! `s_d` is the sample standard deviation of the `n` paired differences with the
//! `n - 1` denominator. It is [`crate::aggregate::ConfidenceInterval`]'s
//! published critical-value table under the method name [`PAIRED_INTERVAL_METHOD`],
//! so an interval depends on the reported values alone and never on wall time,
//! randomness, or iteration order. Fewer than two paired seeds reports
//! `confidence_interval: null`, because one paired difference measures no
//! spread.
//!
//! ## Reported, unpaired, and refused
//!
//! A pair is usable only when both sides report a value for the metric at that
//! seed. A side whose value is not applicable or not observed leaves the pair
//! out of the statistic and is counted in
//! [`PairedDistribution::unpaired`] with each side's status, so a metric one
//! side never reports reads `count: 0` rather than a fabricated zero.
//!
//! Inputs that would silently mis-pair are refused instead:
//!
//! - a batch that names no seed bank, two batches that name different banks, or
//!   a bank file that does not hash to the value both batches recorded;
//! - a batch whose seed list is not its bank's seed list in the bank's order;
//! - a batch that names a seed twice, holds no run for a bank seed, or holds a
//!   run its own seed list does not pair;
//! - a run whose manifest hash, metrics manifest link, or metric definition
//!   version disagrees with what the batch recorded;
//! - a metric one side's artifacts report and the other's do not, which means
//!   the two sides were not measured under one metric definition.
//!
//! A *slice* bucket is sparse by nature — a variant may drive through a movement
//! the other never visits, and a run may not carry a bucket at every seed — so a
//! bucket present on one side only, or at some seeds only, is not a refusal: the
//! seeds where a side made no observation are counted unpaired on that side, and
//! a seed where neither side carried the bucket is counted unobserved on both.
//! Every slice distribution's paired and unpaired seeds therefore account for
//! every pair, exactly as the aggregation counts a sparse slice rather than
//! shrinking its sample. A counted event family is not sparse in that sense: a
//! mode's or a movement's family slice always holds all ten families, `0` when
//! the run recorded none of them, because the run artifact's own `by_family` set
//! already reports an absent family as a count of zero.
//!
//! ## Determinism and immutability
//!
//! Every collection is ordered: the pairs follow the bank's order, the metric
//! and slice maps are `BTreeMap`s, and each distribution's seed lists are
//! ascending. Reading is the whole command: `compare` opens the bank, both
//! `batch.json` manifests, and each paired run's `manifest.json` and
//! `metrics.json`, and writes exactly one file, the comparison itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::aggregate::{
    CONFIDENCE_LEVEL, ConfidenceInterval, LARGEST_TABULATED_DEGREES_OF_FREEDOM,
    LEAST_INTERVAL_SEEDS, METRES, MODE_PAIR_METRIC, NORMAL_CRITICAL_975, Reading, Spread,
    agent_movement_event_readings, mode_event_readings, movement_readings, operational_readings,
    recorded_movement_keys, run_level_readings, sample_mean, sample_variance,
};
use crate::batch::{BATCH_MANIFEST_FILE, BatchManifest, BatchRun};
use crate::run_dir::MANIFEST_FILE;
use crate::run_metrics::{
    METRIC_DEFINITION_VERSION, METRICS_FILE, MetricStatus, MetricValue, RunMetricsArtifact,
};
use crate::seed_bank::{SeedBankError, SeedBankReference, read_seed_bank};
use crate::trace::sha256_hex;

/// Version of the comparison artifact format.
pub const COMPARISON_VERSION: u32 = 1;

/// Default file name of the comparison artifact.
pub const COMPARISON_FILE: &str = "comparison.json";

/// The interval method every paired [`ConfidenceInterval`] names.
pub const PAIRED_INTERVAL_METHOD: &str = "paired_student_t";

/// The per-seed difference the paired statistic is computed over.
pub const PAIRED_DIFFERENCE: &str = "a - b";

/// Which side of a comparison an input belongs to.
///
/// [`Side::A`] is the batch the command was given as its first side and
/// [`Side::B`] the second; the difference is `A - B` in that same order, so a
/// positive mean difference means the first side's metric value is greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The batch given as side A.
    A,
    /// The batch given as side B.
    B,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::A => "A",
            Self::B => "B",
        })
    }
}

/// Failure to compare two batches.
#[derive(Debug, thiserror::Error)]
pub enum CompareError {
    /// The seed bank could not be read or is not a usable bank.
    #[error(transparent)]
    SeedBank(#[from] SeedBankError),
    /// A batch or run artifact could not be read.
    #[error("cannot {action} '{path}' on side {side}: {source}")]
    Io {
        /// The path the operation targeted.
        path: PathBuf,
        /// The side the path belongs to.
        side: Side,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// A batch manifest could not be parsed.
    #[error("batch manifest '{path}' on side {side} is not readable JSON: {source}")]
    BatchManifest {
        /// The batch manifest that failed to parse.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// A run directory's metrics artifact could not be parsed.
    #[error(
        "run directory '{path}' for seed {seed} on side {side} holds no readable '{file}': {source}"
    )]
    Metrics {
        /// The run directory whose metrics failed to parse.
        path: PathBuf,
        /// The side the run belongs to.
        side: Side,
        /// The seed whose directory it is.
        seed: u64,
        /// The artifact file name.
        file: &'static str,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// A batch manifest names no seed bank, so there is no pairing to prove.
    #[error(
        "batch manifest '{path}' on side {side} holds no seed bank; a paired comparison needs two batches that ran one seed bank"
    )]
    NoSeedBank {
        /// The batch manifest that names no bank.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
    },
    /// The two batches ran different seed banks.
    #[error(
        "the two batches ran different seed banks: side A records {a}, side B records {b}; a paired comparison needs one bank"
    )]
    SeedBankMismatch {
        /// The bank content hash side A recorded.
        a: String,
        /// The bank content hash side B recorded.
        b: String,
    },
    /// The bank file given does not hash to the value both batches recorded.
    #[error(
        "seed bank '{path}' hashes to {observed}, but both batches recorded {recorded}; the comparison needs the bank both sides ran"
    )]
    SeedBankFile {
        /// The bank file that was read.
        path: PathBuf,
        /// The hash both batches recorded.
        recorded: String,
        /// The hash of the file on disk.
        observed: String,
    },
    /// A batch manifest holds a different number of seeds than its bank.
    #[error(
        "batch manifest '{path}' on side {side} holds {count} seeds, but its seed bank holds {expected}; both sides must run one bank"
    )]
    SeedCount {
        /// The batch manifest that holds another count.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
        /// The count the manifest holds.
        count: usize,
        /// The count the bank holds.
        expected: usize,
    },
    /// A batch manifest's seeds are not its bank's seeds in the bank's order.
    #[error(
        "batch manifest '{path}' on side {side} holds seed {observed} at position {position}, but its seed bank holds {expected}; a comparison pairs one order"
    )]
    SeedOrder {
        /// The batch manifest that disagrees.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
        /// The zero-based position that disagrees.
        position: usize,
        /// The seed the bank holds there.
        expected: u64,
        /// The seed the batch holds there.
        observed: u64,
    },
    /// A batch manifest names one seed twice, so pairing it is ambiguous.
    #[error("batch manifest '{path}' on side {side} names seed {seed} more than once")]
    DuplicateSeed {
        /// The batch manifest that repeats the seed.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
        /// The repeated seed.
        seed: u64,
    },
    /// A batch manifest holds no run for a seed its bank pairs.
    #[error(
        "batch manifest '{path}' on side {side} holds no run for seed {seed}, which its seed bank pairs"
    )]
    MissingSeed {
        /// The batch manifest that holds no run.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
        /// The seed without a run.
        seed: u64,
    },
    /// A batch manifest holds a run its own seed list does not pair.
    #[error(
        "batch manifest '{path}' on side {side} holds a run for seed {seed}, which its own seed list does not pair"
    )]
    StrayRun {
        /// The batch manifest that holds the run.
        path: PathBuf,
        /// The side the manifest belongs to.
        side: Side,
        /// The seed of the unpaired run.
        seed: u64,
    },
    /// A run directory's manifest bytes do not hash to the value the batch recorded.
    #[error(
        "run directory '{path}' for seed {seed} on side {side} holds a manifest hashing to {observed}, but the batch recorded {recorded}"
    )]
    ManifestDigest {
        /// The run manifest that disagrees.
        path: PathBuf,
        /// The side the run belongs to.
        side: Side,
        /// The seed whose directory it is.
        seed: u64,
        /// The hash the batch manifest recorded.
        recorded: String,
        /// The hash of the bytes on disk.
        observed: String,
    },
    /// A run's metrics artifact links to a different manifest than the batch recorded.
    #[error(
        "run directory '{path}' for seed {seed} on side {side} reports manifest {observed}, but the batch recorded {recorded}"
    )]
    ManifestLink {
        /// The metrics artifact that disagrees.
        path: PathBuf,
        /// The side the run belongs to.
        side: Side,
        /// The seed whose directory it is.
        seed: u64,
        /// The hash the batch manifest recorded.
        recorded: String,
        /// The hash the metrics artifact reports.
        observed: String,
    },
    /// A run's metrics artifact is from another definition revision.
    #[error(
        "run directory '{path}' for seed {seed} on side {side} reports metric definition version {version}, but a comparison reports version {expected}"
    )]
    DefinitionVersion {
        /// The metrics artifact that reports another revision.
        path: PathBuf,
        /// The side the run belongs to.
        side: Side,
        /// The seed whose directory it is.
        seed: u64,
        /// The revision the artifact reports.
        version: u32,
        /// The revision this comparison reports.
        expected: u32,
    },
    /// A metrics artifact claims a reported value without one.
    #[error(
        "run directory '{path}' for seed {seed} on side {side} reports metric '{metric}' as reported with no value"
    )]
    ReportedWithoutValue {
        /// The metrics artifact that claims a value.
        path: PathBuf,
        /// The side the run belongs to.
        side: Side,
        /// The seed whose directory it is.
        seed: u64,
        /// The metric that claims a value.
        metric: String,
    },
    /// A metric one side reports is missing from the other side's artifacts.
    #[error(
        "seed {seed} on side {side} does not report metric '{metric}', which the other runs of the comparison do; the two sides were not measured under one metric definition"
    )]
    MetricAvailability {
        /// The side that does not report the metric.
        side: Side,
        /// The seed whose artifact does not report it.
        seed: u64,
        /// The metric that is missing.
        metric: String,
    },
}

/// The batch manifest a comparison read, with the bank identity it recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparedBatch {
    /// The batch manifest's path relative to its batch root.
    pub path: String,
    /// SHA-256 of the batch manifest's exact bytes.
    pub sha256: String,
    /// The batch manifest format version that was read.
    pub batch_manifest_version: u32,
    /// The seed bank path this batch's manifest recorded.
    pub seed_bank_path: String,
    /// The seed bank content hash this batch's manifest recorded; equal on both
    /// sides, because a comparison refuses two different banks.
    pub seed_bank_sha256: String,
}

/// One side's run for one paired seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparedRun {
    /// The run directory, relative to its batch root.
    pub directory: String,
    /// SHA-256 of the run's `manifest.json` bytes, as the batch recorded it.
    pub manifest_sha256: String,
    /// SHA-256 of the run's `metrics.json` bytes, as read.
    pub metrics_sha256: String,
}

/// One seed's run on both sides: the pairing the seed bank makes possible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparedPair {
    /// The paired seed.
    pub seed: u64,
    /// Side A's run at this seed.
    pub a: ComparedRun,
    /// Side B's run at this seed.
    pub b: ComparedRun,
}

/// The documented method behind every paired interval in the artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedMethod {
    /// The interval method, [`PAIRED_INTERVAL_METHOD`].
    pub confidence_interval_method: String,
    /// The two-sided confidence level.
    pub confidence_level: f64,
    /// The sample variance denominator, `n - 1`.
    pub variance_denominator: String,
    /// The per-seed difference the statistic is computed over,
    /// [`PAIRED_DIFFERENCE`].
    pub paired_difference: String,
    /// The smallest tabulated degrees of freedom.
    pub smallest_tabulated_degrees_of_freedom: u32,
    /// The largest tabulated degrees of freedom.
    pub largest_tabulated_degrees_of_freedom: u32,
    /// The critical value used above the table: the standard normal 97.5%
    /// quantile.
    pub critical_value_above_the_table: f64,
    /// Least paired seeds that yield an interval.
    pub least_interval_seeds: u32,
}

impl PairedMethod {
    /// The method fixed by this version of the comparison format.
    fn v1() -> Self {
        Self {
            confidence_interval_method: PAIRED_INTERVAL_METHOD.to_owned(),
            confidence_level: CONFIDENCE_LEVEL,
            variance_denominator: "n - 1".to_owned(),
            paired_difference: PAIRED_DIFFERENCE.to_owned(),
            smallest_tabulated_degrees_of_freedom: 1,
            largest_tabulated_degrees_of_freedom: LARGEST_TABULATED_DEGREES_OF_FREEDOM,
            critical_value_above_the_table: NORMAL_CRITICAL_975,
            least_interval_seeds: LEAST_INTERVAL_SEEDS,
        }
    }
}

/// A seed the paired statistic excludes, with the status each side reported.
///
/// A pair is usable only when both sides report a value, so at least one of the
/// two statuses here is not [`MetricStatus::Reported`]; the pair is counted
/// rather than read as a zero difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpairedSeed {
    /// The seed whose pair carries no value on at least one side.
    pub seed: u64,
    /// The status side A reported for this metric at this seed.
    pub a: MetricStatus,
    /// The status side B reported for this metric at this seed.
    pub b: MetricStatus,
}

/// One metric's paired comparison over the seeds both sides report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedDistribution {
    /// The metric definition revision these values are reported at.
    pub metric_definition_version: u32,
    /// The metric's unit, as metric definition v2 fixes it.
    pub unit: String,
    /// Paired seeds: the count every statistic here uses.
    pub count: usize,
    /// The mean of the per-seed differences `d_i = A_i - B_i`.
    pub mean_difference: Option<f64>,
    /// The spread of the per-seed differences.
    pub spread: Spread,
    /// The paired interval of the mean difference; absent below
    /// [`LEAST_INTERVAL_SEEDS`] paired seeds.
    pub confidence_interval: Option<ConfidenceInterval>,
    /// The paired seeds, ascending: the positions every statistic used.
    pub paired_seeds: Vec<u64>,
    /// The seeds the statistic excludes, ascending, with both sides' statuses.
    /// For a slice this also holds the seeds the slice itself does not cover.
    pub unpaired: Vec<UnpairedSeed>,
    /// Side A's `manifest_sha256` for the paired seeds, in `paired_seeds` order.
    pub a_manifests: Vec<String>,
    /// Side B's `manifest_sha256` for the paired seeds, in `paired_seeds` order.
    pub b_manifests: Vec<String>,
}

/// One movement bucket's paired distributions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparedMovementSlice {
    /// The bucket's two movement keys, sorted lexicographically.
    pub movement_keys: [String; 2],
    /// The bucket's distributions, keyed by metric name.
    pub metrics: BTreeMap<String, PairedDistribution>,
}

/// `comparison.json`: the paired A/B comparison of two batches run from one
/// common-random-number seed bank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// The comparison format version.
    pub comparison_version: u32,
    /// The metric definition revision every number here is reported at.
    pub metric_definition_version: u32,
    /// The documented method behind every paired interval.
    pub method: PairedMethod,
    /// The seed bank both batches ran and this comparison read, with the content
    /// hash both manifests recorded and the file produced.
    pub seed_bank: SeedBankReference,
    /// The batch manifest side A read. Its numbers are the minuend of every
    /// difference.
    pub a: ComparedBatch,
    /// The batch manifest side B read. Its numbers are the subtrahend.
    pub b: ComparedBatch,
    /// The paired runs, in the seed bank's order.
    pub pairs: Vec<ComparedPair>,
    /// The whole-batch paired distributions, keyed by metric name.
    pub metrics: BTreeMap<String, PairedDistribution>,
    /// The mode-pair slices, keyed by `ModePair` label.
    pub mode_pair_slices: BTreeMap<String, BTreeMap<String, PairedDistribution>>,
    /// The mode event slices, keyed by one `AgentMode` label, holding
    /// `event_counts.<family>` for every counted family.
    pub mode_event_slices: BTreeMap<String, BTreeMap<String, PairedDistribution>>,
    /// The movement slices, keyed by the pair's two movement keys.
    pub movement_slices: BTreeMap<String, ComparedMovementSlice>,
    /// The agent-movement slices, keyed by one movement key, holding the ten
    /// operational metrics of one agent's own movement plus
    /// `event_counts.<family>` for every counted family.
    pub agent_movement_slices: BTreeMap<String, BTreeMap<String, PairedDistribution>>,
}

/// Compare two batches run from one common-random-number seed bank.
///
/// Both batch roots must hold a `batch.json` naming the same bank file, and
/// every seed that bank pairs must have a completed run directory on both sides
/// holding its `manifest.json` and `metrics.json`. Each artifact is
/// cross-checked against the batch manifest's record before it is paired, so a
/// run directory that changed after its batch finished is refused rather than
/// compared. Nothing is written: the caller writes the returned artifact.
pub fn compare_batches(
    a_root: &Path,
    b_root: &Path,
    bank_path: &Path,
) -> Result<Comparison, CompareError> {
    let bank = read_seed_bank(bank_path)?;
    let a = read_side(Side::A, a_root)?;
    let b = read_side(Side::B, b_root)?;

    // One bank, the one the command was given: the file must hash to the value
    // both batches recorded, and the two batches must record one value.
    if b.link.seed_bank_sha256 != a.link.seed_bank_sha256 {
        return Err(CompareError::SeedBankMismatch {
            a: a.link.seed_bank_sha256.clone(),
            b: b.link.seed_bank_sha256.clone(),
        });
    }
    if bank.content_sha256 != a.link.seed_bank_sha256 {
        return Err(CompareError::SeedBankFile {
            path: bank_path.to_path_buf(),
            recorded: a.link.seed_bank_sha256.clone(),
            observed: bank.content_sha256,
        });
    }

    // One order: each batch's seed list is the bank's ordered seed list.
    let seeds = bank.bank.seeds.clone();
    check_seed_order(&a, &seeds)?;
    check_seed_order(&b, &seeds)?;
    let a_runs = index_runs(&a, &seeds)?;
    let b_runs = index_runs(&b, &seeds)?;

    let mut compared_seeds = Vec::with_capacity(seeds.len());
    let mut metrics: BTreeMap<String, PairedAccumulator> = BTreeMap::new();
    let mut mode_pairs: BTreeMap<String, PairedAccumulator> = BTreeMap::new();
    let mut mode_events: BTreeMap<String, SlicePairing> = BTreeMap::new();
    let mut movements: BTreeMap<String, MovementPairing> = BTreeMap::new();
    let mut agent_movements: BTreeMap<String, SlicePairing> = BTreeMap::new();
    let mut metric_set: Option<BTreeSet<String>> = None;

    for seed in &seeds {
        let a_run = check_run(
            &a,
            a_runs
                .get(seed)
                .expect("every bank seed has a run on side A"),
        )?;
        let b_run = check_run(
            &b,
            b_runs
                .get(seed)
                .expect("every bank seed has a run on side B"),
        )?;
        let a_readings = read_readings(
            SeedSide {
                side: Side::A,
                seed: *seed,
                path: &a_run.metrics_path,
            },
            &a_run.artifact,
        )?;
        let b_readings = read_readings(
            SeedSide {
                side: Side::B,
                seed: *seed,
                path: &b_run.metrics_path,
            },
            &b_run.artifact,
        )?;

        // The first seed read fixes the comparison's metric set; a metric any
        // other run reports or omits is a one-sided measurement and is refused.
        check_metric_set(
            &mut metric_set,
            a_readings.metrics.keys().cloned(),
            Side::A,
            *seed,
        )?;
        check_metric_set(
            &mut metric_set,
            b_readings.metrics.keys().cloned(),
            Side::B,
            *seed,
        )?;

        for (key, (unit, a_value)) in &a_readings.metrics {
            let (_, b_value) = b_readings
                .metrics
                .get(key)
                .expect("both sides report the comparison's metric set");
            metrics
                .entry(key.clone())
                .or_insert_with(|| PairedAccumulator::new(unit))
                .record(Pairing {
                    seed: *seed,
                    a: *a_value,
                    b: *b_value,
                    a_manifest: &a_run.link.manifest_sha256,
                    b_manifest: &b_run.link.manifest_sha256,
                });
        }

        // A mode pair or movement bucket one side does not carry is no
        // observation on that side, not a missing measurement.
        let labels: BTreeSet<&String> = a_readings
            .mode_pairs
            .keys()
            .chain(b_readings.mode_pairs.keys())
            .collect();
        for label in labels {
            let a_value = a_readings
                .mode_pairs
                .get(label)
                .copied()
                .unwrap_or(PairReading::NotObserved);
            let b_value = b_readings
                .mode_pairs
                .get(label)
                .copied()
                .unwrap_or(PairReading::NotObserved);
            mode_pairs
                .entry(label.clone())
                .or_insert_with(|| PairedAccumulator::new(METRES))
                .record(Pairing {
                    seed: *seed,
                    a: a_value,
                    b: b_value,
                    a_manifest: &a_run.link.manifest_sha256,
                    b_manifest: &b_run.link.manifest_sha256,
                });
        }

        // A mode one side does not carry made no observation on that side, and
        // a family a mode recorded no record of is a reported zero.
        let modes: BTreeSet<&String> = a_readings
            .mode_events
            .keys()
            .chain(b_readings.mode_events.keys())
            .collect();
        for mode in modes {
            let a_slice = a_readings.mode_events.get(mode);
            let b_slice = b_readings.mode_events.get(mode);
            let source = a_slice
                .or(b_slice)
                .expect("a mode slice belongs to at least one side");
            let pairing = mode_events
                .entry(mode.clone())
                .or_insert_with(|| SlicePairing::of(source));
            pair_slice_metrics(
                &mut pairing.metrics,
                &pairing.units,
                *seed,
                a_slice,
                b_slice,
                &a_run.link.manifest_sha256,
                &b_run.link.manifest_sha256,
            );
        }

        let buckets: BTreeSet<&String> = a_readings
            .movements
            .keys()
            .chain(b_readings.movements.keys())
            .collect();
        for bucket in buckets {
            let a_bucket = a_readings.movements.get(bucket);
            let b_bucket = b_readings.movements.get(bucket);
            let source = a_bucket
                .or(b_bucket)
                .expect("a bucket belongs to at least one side");
            let pairing = movements
                .entry(bucket.clone())
                .or_insert_with(|| MovementPairing::of(source));
            pair_slice_metrics(
                &mut pairing.metrics,
                &pairing.units,
                *seed,
                a_bucket.map(|bucket| &bucket.readings),
                b_bucket.map(|bucket| &bucket.readings),
                &a_run.link.manifest_sha256,
                &b_run.link.manifest_sha256,
            );
        }

        // One agent movement's own slice: its operational values and counted
        // families, paired the same way. A movement one side never placed an
        // agent on is no observation there.
        let agent_buckets: BTreeSet<&String> = a_readings
            .agent_movements
            .keys()
            .chain(b_readings.agent_movements.keys())
            .collect();
        for bucket in agent_buckets {
            let a_slice = a_readings.agent_movements.get(bucket);
            let b_slice = b_readings.agent_movements.get(bucket);
            let source = a_slice
                .or(b_slice)
                .expect("a movement slice belongs to at least one side");
            let pairing = agent_movements
                .entry(bucket.clone())
                .or_insert_with(|| SlicePairing::of(source));
            pair_slice_metrics(
                &mut pairing.metrics,
                &pairing.units,
                *seed,
                a_slice,
                b_slice,
                &a_run.link.manifest_sha256,
                &b_run.link.manifest_sha256,
            );
        }

        compared_seeds.push(ComparedPair {
            seed: *seed,
            a: a_run.link,
            b: b_run.link,
        });
    }

    Ok(Comparison {
        comparison_version: COMPARISON_VERSION,
        metric_definition_version: METRIC_DEFINITION_VERSION,
        method: PairedMethod::v1(),
        seed_bank: SeedBankReference {
            path: bank_path.display().to_string(),
            content_sha256: bank.content_sha256,
        },
        a: a.link,
        b: b.link,
        pairs: compared_seeds,
        metrics: finish_all(metrics),
        mode_pair_slices: mode_pairs
            .into_iter()
            .map(|(label, mut accumulator)| {
                // A slice covers the whole comparison: a seed the bucket does not
                // reach is no observation on either side, not a smaller sample.
                accumulator.fill_missing(&seeds);
                let slice = BTreeMap::from([(MODE_PAIR_METRIC.to_owned(), accumulator.finish())]);
                (label, slice)
            })
            .collect(),
        mode_event_slices: finish_slice_pairings(mode_events, &seeds),
        movement_slices: movements
            .into_iter()
            .map(|(bucket, mut pairing)| {
                for accumulator in pairing.metrics.values_mut() {
                    accumulator.fill_missing(&seeds);
                }
                (
                    bucket,
                    ComparedMovementSlice {
                        movement_keys: pairing.keys,
                        metrics: finish_all(pairing.metrics),
                    },
                )
            })
            .collect(),
        agent_movement_slices: finish_slice_pairings(agent_movements, &seeds),
    })
}

/// Close every bucket of a slice family, counting the seeds a bucket does not
/// reach as no observation on either side.
fn finish_slice_pairings(
    pairings: BTreeMap<String, SlicePairing>,
    seeds: &[u64],
) -> BTreeMap<String, BTreeMap<String, PairedDistribution>> {
    pairings
        .into_iter()
        .map(|(key, mut pairing)| {
            for accumulator in pairing.metrics.values_mut() {
                accumulator.fill_missing(seeds);
            }
            (key, finish_all(pairing.metrics))
        })
        .collect()
}

/// One side's batch manifest and the links a comparison records for it.
struct SideBatch {
    /// The side's batch root.
    root: PathBuf,
    /// The side's batch manifest path.
    manifest_path: PathBuf,
    /// The links the comparison records for this side's manifest.
    link: ComparedBatch,
    /// The seeds the manifest lists, in the order it lists them.
    seeds: Vec<u64>,
    /// The runs the manifest lists.
    runs: Vec<BatchRun>,
    /// The side, for a diagnostic.
    side: Side,
}

/// Read one side's batch manifest and the bank identity it records.
fn read_side(side: Side, root: &Path) -> Result<SideBatch, CompareError> {
    let manifest_path = root.join(BATCH_MANIFEST_FILE);
    let bytes = read(&manifest_path, side, "read batch manifest")?;
    let manifest: BatchManifest =
        serde_json::from_slice(&bytes).map_err(|source| CompareError::BatchManifest {
            path: manifest_path.clone(),
            side,
            source,
        })?;
    let seed_bank = manifest
        .seed_bank
        .clone()
        .ok_or_else(|| CompareError::NoSeedBank {
            path: manifest_path.clone(),
            side,
        })?;
    Ok(SideBatch {
        root: root.to_path_buf(),
        manifest_path: manifest_path.clone(),
        link: ComparedBatch {
            path: BATCH_MANIFEST_FILE.to_owned(),
            sha256: sha256_hex(&bytes),
            batch_manifest_version: manifest.batch_manifest_version,
            seed_bank_path: seed_bank.path,
            seed_bank_sha256: seed_bank.content_sha256,
        },
        seeds: manifest.seeds,
        runs: manifest.runs,
        side,
    })
}

/// Check that a batch manifest's seed list is the bank's ordered seed list.
fn check_seed_order(batch: &SideBatch, seeds: &[u64]) -> Result<(), CompareError> {
    if batch.seeds.len() != seeds.len() {
        return Err(CompareError::SeedCount {
            path: batch.manifest_path.clone(),
            side: batch.side,
            count: batch.seeds.len(),
            expected: seeds.len(),
        });
    }
    for (position, (expected, observed)) in seeds.iter().zip(&batch.seeds).enumerate() {
        if expected != observed {
            return Err(CompareError::SeedOrder {
                path: batch.manifest_path.clone(),
                side: batch.side,
                position,
                expected: *expected,
                observed: *observed,
            });
        }
    }
    Ok(())
}

/// Index a batch manifest's runs by seed, refusing a seed that cannot be paired.
///
/// A repeated seed is ambiguous, a bank seed without a run is unpairable, and a
/// run the manifest's own seed list does not hold is a batch that disagrees with
/// itself; all three are refused rather than skipped.
fn index_runs<'a>(
    batch: &'a SideBatch,
    seeds: &[u64],
) -> Result<BTreeMap<u64, &'a BatchRun>, CompareError> {
    let mut listed: Vec<u64> = batch.runs.iter().map(|run| run.seed).collect();
    listed.sort_unstable();
    if let Some(seed) = listed
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
    {
        return Err(CompareError::DuplicateSeed {
            path: batch.manifest_path.clone(),
            side: batch.side,
            seed,
        });
    }
    let indexed: BTreeMap<u64, &BatchRun> = batch.runs.iter().map(|run| (run.seed, run)).collect();

    let paired: BTreeSet<u64> = seeds.iter().copied().collect();
    if let Some(seed) = seeds.iter().find(|seed| !indexed.contains_key(seed)) {
        return Err(CompareError::MissingSeed {
            path: batch.manifest_path.clone(),
            side: batch.side,
            seed: *seed,
        });
    }
    if let Some(seed) = indexed.keys().find(|seed| !paired.contains(seed)) {
        return Err(CompareError::StrayRun {
            path: batch.manifest_path.clone(),
            side: batch.side,
            seed: *seed,
        });
    }
    Ok(indexed)
}

/// One run's checked artifacts.
struct CheckedRun {
    /// The links the comparison records for this run.
    link: ComparedRun,
    /// The `metrics.json` this run was read from, for a diagnostic.
    metrics_path: PathBuf,
    /// The parsed metric artifact.
    artifact: RunMetricsArtifact,
}

/// Read one run's manifest and metrics and check them against what the batch
/// recorded, so a run that changed after its batch finished is refused.
fn check_run(batch: &SideBatch, run: &BatchRun) -> Result<CheckedRun, CompareError> {
    let directory = batch.root.join(&run.directory);
    let manifest_path = directory.join(MANIFEST_FILE);
    let manifest_sha256 = sha256_hex(&read(&manifest_path, batch.side, "read run manifest")?);
    if manifest_sha256 != run.manifest_sha256 {
        return Err(CompareError::ManifestDigest {
            path: manifest_path,
            side: batch.side,
            seed: run.seed,
            recorded: run.manifest_sha256.clone(),
            observed: manifest_sha256,
        });
    }

    let metrics_path = directory.join(METRICS_FILE);
    let metrics_bytes = read(&metrics_path, batch.side, "read run metrics")?;
    let artifact: RunMetricsArtifact =
        serde_json::from_slice(&metrics_bytes).map_err(|source| CompareError::Metrics {
            path: directory,
            side: batch.side,
            seed: run.seed,
            file: METRICS_FILE,
            source,
        })?;
    if artifact.metric_definition_version != METRIC_DEFINITION_VERSION {
        return Err(CompareError::DefinitionVersion {
            path: metrics_path,
            side: batch.side,
            seed: run.seed,
            version: artifact.metric_definition_version,
            expected: METRIC_DEFINITION_VERSION,
        });
    }
    if artifact.manifest_sha256 != run.manifest_sha256 {
        return Err(CompareError::ManifestLink {
            path: metrics_path,
            side: batch.side,
            seed: run.seed,
            recorded: run.manifest_sha256.clone(),
            observed: artifact.manifest_sha256.clone(),
        });
    }

    Ok(CheckedRun {
        link: ComparedRun {
            directory: run.directory.clone(),
            manifest_sha256: run.manifest_sha256.clone(),
            metrics_sha256: sha256_hex(&metrics_bytes),
        },
        metrics_path,
        artifact,
    })
}

/// One side's reading of one metric at one seed.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PairReading {
    /// The side reported a value.
    Reported(f64),
    /// The metric's defining predicate was false here.
    NotApplicable,
    /// The side made no observation.
    NotObserved,
}

impl PairReading {
    /// The reporting status this reading stands for.
    fn status(self) -> MetricStatus {
        match self {
            Self::Reported(_) => MetricStatus::Reported,
            Self::NotApplicable => MetricStatus::NotApplicable,
            Self::NotObserved => MetricStatus::NotObserved,
        }
    }

    /// The value, when the metric was reported.
    fn value(self) -> Option<f64> {
        match self {
            Self::Reported(value) => Some(value),
            Self::NotApplicable | Self::NotObserved => None,
        }
    }
}

/// Where one seed's reading on one side came from, for a diagnostic.
#[derive(Debug, Clone, Copy)]
struct SeedSide<'a> {
    /// The side that produced the reading.
    side: Side,
    /// The seed the reading belongs to.
    seed: u64,
    /// The `metrics.json` the reading was read from.
    path: &'a Path,
}

impl SeedSide<'_> {
    /// Read one artifact metric value with its reporting status.
    fn reading(&self, metric: &str, value: &MetricValue) -> Result<PairReading, CompareError> {
        match value.status {
            MetricStatus::Reported => match value.value {
                Some(value) => Ok(PairReading::Reported(value)),
                None => Err(CompareError::ReportedWithoutValue {
                    path: self.path.to_path_buf(),
                    side: self.side,
                    seed: self.seed,
                    metric: metric.to_owned(),
                }),
            },
            MetricStatus::NotApplicable => Ok(PairReading::NotApplicable),
            MetricStatus::NotObserved => Ok(PairReading::NotObserved),
        }
    }
}

/// One slice bucket's readings on one side: a mode's counted families, an agent
/// movement's operational values and counted families, or a pair movement
/// bucket's interaction minima.
struct SliceReadings {
    /// The slice's metrics, each with its unit.
    units: BTreeMap<String, &'static str>,
    /// The slice's per-metric readings.
    readings: BTreeMap<String, PairReading>,
}

impl SliceReadings {
    /// The slice's readings, each with the unit it is reported in.
    fn of(readings: BTreeMap<String, (&'static str, PairReading)>) -> Self {
        let mut units = BTreeMap::new();
        let mut values = BTreeMap::new();
        for (name, (unit, reading)) in readings {
            units.insert(name.clone(), unit);
            values.insert(name, reading);
        }
        Self {
            units,
            readings: values,
        }
    }
}

/// One movement bucket's readings on one side.
struct Movements {
    /// The bucket's two movement keys, sorted lexicographically.
    keys: [String; 2],
    /// The bucket's readings.
    readings: SliceReadings,
}

/// One side's readings of one seed.
struct SeedReadings {
    /// The whole-batch metrics, keyed by metric name, each with its unit.
    metrics: BTreeMap<String, (&'static str, PairReading)>,
    /// The mode-pair values, keyed by `ModePair` label.
    mode_pairs: BTreeMap<String, PairReading>,
    /// The mode event slices, keyed by `AgentMode` label.
    mode_events: BTreeMap<String, SliceReadings>,
    /// The pair movement buckets, keyed by the pair's two movement keys.
    movements: BTreeMap<String, Movements>,
    /// The agent movement slices, keyed by one movement key.
    agent_movements: BTreeMap<String, SliceReadings>,
}

/// Read one artifact's metrics into the comparison's reading shape.
fn read_readings(
    side: SeedSide<'_>,
    artifact: &RunMetricsArtifact,
) -> Result<SeedReadings, CompareError> {
    let mut metrics = BTreeMap::new();
    for (key, unit, reading) in run_level_readings(artifact) {
        let reading = match reading {
            Reading::Value(value) => side.reading(&key, value)?,
            // An always-reported count is a value: zero events is an observation.
            Reading::Count(count) => PairReading::Reported(count as f64),
        };
        metrics.insert(key, (unit, reading));
    }

    let mut mode_pairs = BTreeMap::new();
    for (label, value) in &artifact.mode_pair_minimum_separation_m {
        mode_pairs.insert(label.clone(), side.reading(MODE_PAIR_METRIC, value)?);
    }

    // Every mode carries every counted family, `0` when the mode recorded none,
    // so a mode's slice is a complete observation rather than a sparse bucket.
    let mut mode_events = BTreeMap::new();
    for mode in artifact.operational.by_mode.keys() {
        let readings = mode_event_readings(&artifact.event_counts, mode)
            .into_iter()
            .map(|(key, unit, count)| (key, (unit, PairReading::Reported(count as f64))))
            .collect();
        mode_events.insert(mode.clone(), SliceReadings::of(readings));
    }

    let mut movements = BTreeMap::new();
    for (bucket, minima) in &artifact.movement_minima {
        let mut readings = BTreeMap::new();
        for (name, unit, value) in movement_readings(minima) {
            readings.insert(name.to_owned(), (unit, side.reading(name, value)?));
        }
        movements.insert(
            bucket.clone(),
            Movements {
                keys: minima.movement_keys.clone(),
                readings: SliceReadings::of(readings),
            },
        );
    }

    // One agent movement's own slice: the operational values of the movement it
    // was observed on plus that movement's counted families, which are complete
    // like a mode's.
    let mut agent_movements = BTreeMap::new();
    let mut buckets: BTreeSet<&str> = artifact
        .operational
        .by_movement
        .keys()
        .map(String::as_str)
        .collect();
    buckets.extend(recorded_movement_keys(&artifact.event_counts));
    for bucket in buckets {
        let mut readings: BTreeMap<String, (&'static str, PairReading)> = BTreeMap::new();
        if let Some(values) = artifact.operational.by_movement.get(bucket) {
            for (name, unit, value) in operational_readings(values) {
                readings.insert(name.to_owned(), (unit, side.reading(name, value)?));
            }
        }
        for (key, unit, count) in agent_movement_event_readings(&artifact.event_counts, bucket) {
            readings.insert(key, (unit, PairReading::Reported(count as f64)));
        }
        agent_movements.insert(bucket.to_owned(), SliceReadings::of(readings));
    }

    Ok(SeedReadings {
        metrics,
        mode_pairs,
        mode_events,
        movements,
        agent_movements,
    })
}

/// Fix the comparison's metric set on the first run read and refuse any run that
/// reports another set.
fn check_metric_set(
    expected: &mut Option<BTreeSet<String>>,
    observed: impl Iterator<Item = String>,
    side: Side,
    seed: u64,
) -> Result<(), CompareError> {
    let observed: BTreeSet<String> = observed.collect();
    match expected {
        None => {
            *expected = Some(observed);
            Ok(())
        }
        Some(expected) if *expected == observed => Ok(()),
        Some(expected) => Err(CompareError::MetricAvailability {
            side,
            seed,
            metric: expected
                .symmetric_difference(&observed)
                .next()
                .expect("two different sets differ by at least one key")
                .clone(),
        }),
    }
}

/// One bucket's reading on a side, or no observation when the side does not
/// carry the bucket at all.
fn slice_reading(bucket: Option<&SliceReadings>, metric: &str) -> PairReading {
    bucket
        .and_then(|bucket| bucket.readings.get(metric))
        .copied()
        .unwrap_or(PairReading::NotObserved)
}

/// Pair every metric of one slice bucket for one seed.
///
/// A metric a bucket carries on neither side is no observation on either side,
/// which is what a bucket a run does not reach contributes to the pair.
fn pair_slice_metrics(
    accumulators: &mut BTreeMap<String, PairedAccumulator>,
    units: &BTreeMap<String, &'static str>,
    seed: u64,
    a: Option<&SliceReadings>,
    b: Option<&SliceReadings>,
    a_manifest: &str,
    b_manifest: &str,
) {
    let bucket_metrics: Vec<(String, &'static str)> = units
        .iter()
        .map(|(name, unit)| (name.clone(), *unit))
        .collect();
    for (name, unit) in bucket_metrics {
        accumulators
            .entry(name.clone())
            .or_insert_with(|| PairedAccumulator::new(unit))
            .record(Pairing {
                seed,
                a: slice_reading(a, &name),
                b: slice_reading(b, &name),
                a_manifest,
                b_manifest,
            });
    }
}

/// One seed's paired reading of one metric.
struct Pairing<'a> {
    /// The paired seed.
    seed: u64,
    /// Side A's reading.
    a: PairReading,
    /// Side B's reading.
    b: PairReading,
    /// Side A's run manifest hash.
    a_manifest: &'a str,
    /// Side B's run manifest hash.
    b_manifest: &'a str,
}

/// One slice bucket's paired accumulators.
struct SlicePairing {
    /// The bucket's metrics with the unit each is reported in.
    units: BTreeMap<String, &'static str>,
    /// The bucket's per-metric accumulators.
    metrics: BTreeMap<String, PairedAccumulator>,
}

impl SlicePairing {
    /// The bucket's metrics and units, as the side that carries it spells them.
    fn of(source: &SliceReadings) -> Self {
        Self {
            units: source.units.clone(),
            metrics: BTreeMap::new(),
        }
    }
}

/// One movement bucket's paired accumulators.
struct MovementPairing {
    /// The bucket's two movement keys, sorted lexicographically.
    keys: [String; 2],
    /// The bucket's metrics with the unit each is reported in.
    units: BTreeMap<String, &'static str>,
    /// The bucket's per-metric accumulators.
    metrics: BTreeMap<String, PairedAccumulator>,
}

impl MovementPairing {
    /// The bucket's metrics and units, as the side that carries it spells them.
    ///
    /// The bucket key is the two movement keys sorted and joined, so both sides
    /// that carry the bucket name it identically; the keys are recorded from the
    /// first side that carries it.
    fn of(source: &Movements) -> Self {
        Self {
            keys: source.keys.clone(),
            units: source.readings.units.clone(),
            metrics: BTreeMap::new(),
        }
    }
}

/// The running paired readings of one metric.
#[derive(Debug)]
struct PairedAccumulator {
    /// The metric's unit.
    unit: &'static str,
    /// The paired seeds with their differences and both manifest hashes.
    differences: Vec<PairedDifference>,
    /// The seeds the statistic excludes, with both sides' statuses.
    unpaired: Vec<UnpairedSeed>,
}

/// One paired seed's difference and the manifests that produced it.
#[derive(Debug)]
struct PairedDifference {
    seed: u64,
    difference: f64,
    a_manifest: String,
    b_manifest: String,
}

impl PairedAccumulator {
    /// An accumulator for a metric in `unit`.
    fn new(unit: &'static str) -> Self {
        Self {
            unit,
            differences: Vec::new(),
            unpaired: Vec::new(),
        }
    }

    /// Record one seed's two readings.
    ///
    /// Both sides must report a value for the seed to enter the statistic; a
    /// pair with a not-applicable or not-observed side is counted instead.
    fn record(&mut self, pairing: Pairing<'_>) {
        match (pairing.a.value(), pairing.b.value()) {
            (Some(a), Some(b)) => self.differences.push(PairedDifference {
                seed: pairing.seed,
                difference: a - b,
                a_manifest: pairing.a_manifest.to_owned(),
                b_manifest: pairing.b_manifest.to_owned(),
            }),
            _ => self.unpaired.push(UnpairedSeed {
                seed: pairing.seed,
                a: pairing.a.status(),
                b: pairing.b.status(),
            }),
        }
    }

    /// Count every seed the accumulator holds no reading for as unobserved on
    /// both sides.
    ///
    /// A slice bucket exists only where a run carries it, so the seeds it does
    /// not reach would otherwise leave the distribution's statuses short of the
    /// comparison's pairs. The seeds are walked in the bank's order, but a seed
    /// the bucket does reach was already recorded by then, so the seeds this
    /// walk appends are not necessarily past it; [`Self::finish`] sorts the list
    /// into seed order before it is published.
    fn fill_missing(&mut self, seeds: &[u64]) {
        let known: BTreeSet<u64> = self
            .differences
            .iter()
            .map(|difference| difference.seed)
            .chain(self.unpaired.iter().map(|unpaired| unpaired.seed))
            .collect();
        for seed in seeds {
            if !known.contains(seed) {
                self.unpaired.push(UnpairedSeed {
                    seed: *seed,
                    a: MetricStatus::NotObserved,
                    b: MetricStatus::NotObserved,
                });
            }
        }
    }

    /// Close the accumulator into the metric's paired comparison.
    ///
    /// The paired seeds were recorded in the seed bank's order. The unpaired
    /// list is not in that order yet: a bucket first reached at seed `k` records
    /// `k` before [`Self::fill_missing`] appends the lower seeds it never
    /// reaches, so the list is sorted here and published in seed order whatever
    /// the call order was.
    fn finish(mut self) -> PairedDistribution {
        self.unpaired.sort_by_key(|unpaired| unpaired.seed);
        let values: Vec<f64> = self
            .differences
            .iter()
            .map(|difference| difference.difference)
            .collect();
        let mean = sample_mean(&values);
        let variance = sample_variance(&values, mean);
        PairedDistribution {
            metric_definition_version: METRIC_DEFINITION_VERSION,
            unit: self.unit.to_owned(),
            count: values.len(),
            mean_difference: mean,
            spread: Spread::of(&values, variance),
            confidence_interval: ConfidenceInterval::labelled(
                PAIRED_INTERVAL_METHOD,
                &values,
                mean,
                variance,
            ),
            paired_seeds: self
                .differences
                .iter()
                .map(|difference| difference.seed)
                .collect(),
            unpaired: self.unpaired,
            a_manifests: self
                .differences
                .iter()
                .map(|difference| difference.a_manifest.clone())
                .collect(),
            b_manifests: self
                .differences
                .iter()
                .map(|difference| difference.b_manifest.clone())
                .collect(),
        }
    }
}

/// Close every accumulator of one metric map into its paired distributions.
fn finish_all(
    accumulators: BTreeMap<String, PairedAccumulator>,
) -> BTreeMap<String, PairedDistribution> {
    accumulators
        .into_iter()
        .map(|(key, accumulator)| (key, accumulator.finish()))
        .collect()
}

/// Read a file, or fail with the side and the operation that targeted it.
fn read(path: &Path, side: Side, action: &'static str) -> Result<Vec<u8>, CompareError> {
    fs::read(path).map_err(|source| CompareError::Io {
        path: path.to_path_buf(),
        side,
        action,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pair enters the statistic only when both sides report a value.
    #[test]
    fn a_paired_accumulator_keeps_only_the_seeds_both_sides_report() {
        let mut accumulator = PairedAccumulator::new(METRES);
        let absorb = |accumulator: &mut PairedAccumulator, seed, a, b| {
            accumulator.record(Pairing {
                seed,
                a,
                b,
                a_manifest: "a",
                b_manifest: "b",
            });
        };
        absorb(
            &mut accumulator,
            0,
            PairReading::Reported(1.0),
            PairReading::Reported(0.5),
        );
        absorb(
            &mut accumulator,
            1,
            PairReading::Reported(2.0),
            PairReading::Reported(1.0),
        );
        absorb(
            &mut accumulator,
            2,
            PairReading::NotApplicable,
            PairReading::Reported(2.5),
        );
        absorb(
            &mut accumulator,
            3,
            PairReading::Reported(4.0),
            PairReading::NotObserved,
        );

        let distribution = accumulator.finish();
        assert_eq!(distribution.count, 2);
        assert_eq!(distribution.paired_seeds, vec![0, 1]);
        assert_eq!(distribution.a_manifests, vec!["a", "a"]);
        assert_eq!(distribution.b_manifests, vec!["b", "b"]);
        assert_eq!(
            distribution.unpaired,
            vec![
                UnpairedSeed {
                    seed: 2,
                    a: MetricStatus::NotApplicable,
                    b: MetricStatus::Reported,
                },
                UnpairedSeed {
                    seed: 3,
                    a: MetricStatus::Reported,
                    b: MetricStatus::NotObserved,
                },
            ]
        );
        assert_eq!(distribution.unit, METRES);
    }

    /// The paired interval is the Student-t interval of the differences.
    #[test]
    fn the_paired_interval_is_the_student_t_interval_of_the_differences() {
        let differences = [0.5, 1.0, 0.5, 0.0];
        let mean = sample_mean(&differences);
        let variance = sample_variance(&differences, mean);
        assert_eq!(mean, Some(0.5));
        assert!((variance.expect("four differences have a variance") - 0.5 / 3.0).abs() < 1e-12);

        let interval =
            ConfidenceInterval::labelled(PAIRED_INTERVAL_METHOD, &differences, mean, variance)
                .expect("four differences have an interval");
        assert_eq!(interval.method, PAIRED_INTERVAL_METHOD);
        assert_eq!(interval.degrees_of_freedom, 3);
        let standard_error = (0.5_f64 / 3.0).sqrt() / 2.0;
        assert!((interval.standard_error - standard_error).abs() < 1e-12);
        let half_width = crate::aggregate::T_CRITICAL_975[2] * standard_error;
        assert!((interval.half_width - half_width).abs() < 1e-12);
        assert!((interval.lower - (0.5 - half_width)).abs() < 1e-12);
        assert!((interval.upper - (0.5 + half_width)).abs() < 1e-12);

        // One paired difference measures no spread, so it reports no interval.
        assert_eq!(
            ConfidenceInterval::labelled(PAIRED_INTERVAL_METHOD, &[1.0], Some(1.0), None),
            None
        );
    }
}
