//! The `aggregate` command: the across-seed distributions of one completed batch.
//!
//! A batch writes one immutable run directory per seed and links them in
//! `batch.json`; each run directory holds the versioned `metrics.json` that
//! reports metric definition v2's values for that seed
//! ([[DEF-005-metric-definition-v2]], which carries
//! [[DEF-004-metric-definition-v1]] forward). This module turns those files into
//! the batch-level experiment record, `aggregation.json`: for every metric the
//! runs report, the across-seed count, mean, spread, and confidence interval,
//! disaggregated by mode pair (`ModePair`) and by movement (the `MovementId` /
//! `PedestrianRouteId` union v1 chose), with every aggregated metric linked to
//! the run manifest(s) it came from and to `metric_definition_version: 2`.
//!
//! ## Metric keys and slices
//!
//! A whole-batch metric is keyed by the run artifact's own metric name:
//! `minimum_ttc_s`, `minimum_separation_m`, `minimum_post_encroachment_s`,
//! `event_counts.total`, `event_counts.by_family.<family>` for every counted
//! event family, and `event_counts.by_family_kind.<family>.<kind>` for the two
//! kinded families. The metric set is read from the artifacts, so a later
//! definition revision that adds a metric is aggregated without a change here.
//!
//! The slices hold the interaction metrics, the event families, and the
//! operational families:
//!
//! - [`Aggregation::mode_pair_slices`] is keyed by the `ModePair` label
//!   (`vehicle_vehicle`, `vehicle_pedestrian`, `pedestrian_pedestrian`) and
//!   holds that pair's `minimum_separation_m`, which is the only metric a run
//!   reports per mode pair;
//! - [`Aggregation::movement_slices`] is keyed by a bucket of two movement keys
//!   and holds the bucket's `minimum_separation_m`, `minimum_ttc_s`, and
//!   `minimum_post_encroachment_s`, exactly as the run artifact keys them;
//! - [`Aggregation::mode_event_slices`] is keyed by one `AgentMode` label and
//!   holds the counted event families that mode's own agents produced, keyed
//!   `event_counts.<family>`;
//! - [`Aggregation::agent_movement_slices`] is keyed by one movement key — the
//!   key of the agent that produced the record, not a pair of keys — and holds
//!   that movement's operational values plus its counted event families.
//!
//! The operational families per mode are already whole-batch metrics
//! (`operational.by_mode.<mode>.<metric>`), so the mode slice carries exactly
//! what the whole-batch map cannot: the event counts. The operational movement
//! level is deliberately not a whole-batch metric — a movement bucket is sparse
//! across seeds — so the agent-movement slice carries the operational values
//! there as well.
//!
//! A family a mode or movement recorded no record for is a reported `0`, not an
//! absent value: the run artifact's own `by_family` set always holds every
//! family, so a count of an absent family is an observation of zero. A movement
//! bucket a run's artifact does not carry at all is a different statement — the
//! run placed no agent on that movement — and is counted `not_observed` there,
//! like every other sparse slice.
//!
//! ## Reported, not applicable, not observed
//!
//! A value DEF-004 calls not applicable (no closing pair, so no time to
//! collision) or not observed (no pair came within the interaction range) is
//! never folded in as `0`. Each distribution reports the seeds behind each
//! status separately:
//!
//! - `count`, `mean`, `spread`, and `confidence_interval` use only the seeds
//!   that reported a value;
//! - `reported_seeds`, `not_applicable_seeds`, and `not_observed_seeds` list
//!   those seeds in ascending order, so a consumer reads a metric with three
//!   reported seeds of ten as `count: 3`, not as a mean diluted by seven zeros;
//! - `manifests` holds the `manifest_sha256` of the reported seeds, in the same
//!   order as `reported_seeds`, so every averaged number links to the runs it
//!   came from;
//! - a whole-batch metric whose count and status lists do not cover every seed
//!   is refused rather than silently under-counted, because a metric missing
//!   from one seed's artifact is a broken comparison;
//! - a movement bucket one run's artifact does not carry at all is counted
//!   `not_observed` for that run: mounting no value is making no observation.
//!   A slice's three status lists therefore always sum to the batch's seed
//!   count.
//!
//! ## The interval
//!
//! The interval is the two-sided Student-t interval on the sample mean at a 95%
//! confidence level: `mean ± critical_value * s / sqrt(n)`, where `s` is the
//! sample standard deviation with the `n - 1` denominator (Bessel's correction)
//! and the critical value is `t(0.975, n - 1)`. It is read from
//! [`T_CRITICAL_975`], the tabulated two-sided 95% critical values for 1 to
//! [`LARGEST_TABULATED_DEGREES_OF_FREEDOM`] degrees of freedom — the range a
//! seed-count experiment inhabits — and above the table from
//! [`NORMAL_CRITICAL_975`], the standard normal 97.5% quantile, which every
//! tabulated value above 30 degrees of freedom lies within 4% of. Both are
//! published constants, so an interval depends on the reported values alone and
//! never on wall time, randomness, or iteration order. A metric with fewer than
//! two reported seeds has no interval and reports `confidence_interval: null`,
//! because one observation measures no spread.
//!
//! The interval is the symmetric t interval and is never clamped to the
//! metric's physical range: a small sample of a non-negative metric can report
//! a lower bound below zero, which is the interval's honest statement about the
//! mean, not a claim that the metric can be negative.
//!
//! ## Determinism and immutability
//!
//! Every collection is ordered — metrics, slices, and seeds by ascending key,
//! each distribution's seed lists by ascending seed, and the mean summed in
//! ascending seed order — so aggregating one batch twice produces byte-identical
//! bytes and a batch whose seeds were requested out of order aggregates the
//! same as one whose seeds were requested in order. Reading is the whole
//! command: `aggregate` opens `batch.json` and each run's `manifest.json` and
//! `metrics.json` and writes exactly one file, the aggregation itself. No run
//! artifact is created, changed, or removed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::batch::{BATCH_MANIFEST_FILE, BatchManifest};
use crate::run_dir::MANIFEST_FILE;
use crate::run_metrics::{
    EVENT_FAMILY_LABELS, EventCounts, METRIC_DEFINITION_VERSION, METRICS_FILE, MetricStatus,
    MetricValue, MovementMinima, OperationalValues, RunMetricsArtifact,
};
use crate::trace::sha256_hex;

/// Version of the aggregation artifact format.
pub const AGGREGATION_VERSION: u32 = 1;

/// Default file name of the aggregation artifact inside the batch root.
pub const AGGREGATION_FILE: &str = "aggregation.json";

/// The interval method every [`ConfidenceInterval`] names.
pub const INTERVAL_METHOD: &str = "student_t";

/// Two-sided confidence level every interval is reported at.
pub const CONFIDENCE_LEVEL: f64 = 0.95;

/// Largest degrees of freedom [`T_CRITICAL_975`] tabulates.
pub const LARGEST_TABULATED_DEGREES_OF_FREEDOM: u32 = 30;

/// Least reported seeds that yield an interval.
pub const LEAST_INTERVAL_SEEDS: u32 = 2;

/// The two-sided 95% Student-t critical values `t(0.975, df)` for `df` from 1 to
/// [`LARGEST_TABULATED_DEGREES_OF_FREEDOM`], in ascending `df` order.
///
/// Published constants, so the interval is reproducible: 12.706 for one degree
/// of freedom down to 2.042 for thirty.
pub const T_CRITICAL_975: [f64; 30] = [
    12.706_204_736,
    4.302_652_730,
    3.182_446_305,
    2.776_445_105,
    2.570_581_836,
    2.446_911_851,
    2.364_624_252,
    2.306_004_135,
    2.262_157_163,
    2.228_138_852,
    2.200_985_160,
    2.178_812_830,
    2.160_368_656,
    2.144_786_688,
    2.131_449_546,
    2.119_905_299,
    2.109_815_578,
    2.100_922_040,
    2.093_024_054,
    2.085_963_447,
    2.079_613_845,
    2.073_873_068,
    2.068_657_610,
    2.063_898_562,
    2.059_538_553,
    2.055_529_439,
    2.051_830_516,
    2.048_407_142,
    2.045_229_642,
    2.042_272_456,
];

/// The standard normal 97.5% quantile, used above the tabulated range.
pub const NORMAL_CRITICAL_975: f64 = 1.959_963_984_540_054;

/// The metric a mode-pair slice reports.
pub(crate) const MODE_PAIR_METRIC: &str = "minimum_separation_m";

/// The unit of a time metric, as metric definition v2 fixes it.
const SECONDS: &str = "seconds";

/// The unit of a distance metric, as metric definition v2 fixes it.
pub(crate) const METRES: &str = "metres";

/// The countable event families' unit, as metric definition v2 fixes it.
pub(crate) const RECORDS: &str = "records";

/// The metric-key prefix a slice's counted event family carries.
pub(crate) const EVENT_COUNT_PREFIX: &str = "event_counts.";

/// The unit of a throughput metric, as metric definition v2 fixes it.
const AGENTS_PER_SECOND: &str = "agents_per_second";

/// The unit of a standing-agent count, as metric definition v2 fixes it.
pub(crate) const AGENTS: &str = "agents";

/// Failure to aggregate a completed batch.
#[derive(Debug, thiserror::Error)]
pub enum AggregateError {
    /// A batch or run artifact could not be read.
    #[error("cannot {action} '{path}': {source}")]
    Io {
        /// The path the operation targeted.
        path: PathBuf,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The batch manifest could not be parsed.
    #[error("batch manifest '{path}' is not readable JSON: {source}")]
    BatchManifest {
        /// The batch manifest that failed to parse.
        path: PathBuf,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// A run directory's metrics artifact could not be parsed.
    #[error("run directory '{path}' for seed {seed} holds no readable '{file}': {source}")]
    Metrics {
        /// The run directory whose metrics failed to parse.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The artifact file name.
        file: &'static str,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// The batch manifest names one seed twice.
    #[error("batch manifest '{path}' names seed {seed} more than once")]
    DuplicateSeed {
        /// The batch manifest that repeats the seed.
        path: PathBuf,
        /// The repeated seed.
        seed: u64,
    },
    /// A run directory's manifest bytes do not hash to the value the batch recorded.
    #[error(
        "run directory '{path}' for seed {seed} holds a manifest hashing to {observed}, but the batch recorded {recorded}"
    )]
    ManifestDigest {
        /// The run manifest that disagrees.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The hash the batch manifest recorded.
        recorded: String,
        /// The hash of the bytes on disk.
        observed: String,
    },
    /// A run's metrics artifact links to a different manifest than the batch recorded.
    #[error(
        "run directory '{path}' for seed {seed} reports manifest {observed}, but the batch recorded {recorded}"
    )]
    ManifestLink {
        /// The metrics artifact that disagrees.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The hash the batch manifest recorded.
        recorded: String,
        /// The hash the metrics artifact reports.
        observed: String,
    },
    /// A run's metrics artifact is from another definition revision.
    #[error(
        "run directory '{path}' for seed {seed} reports metric definition version {version}, but an aggregation reports version {expected}"
    )]
    DefinitionVersion {
        /// The metrics artifact that reports another revision.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The revision the artifact reports.
        version: u32,
        /// The revision this aggregation reports.
        expected: u32,
    },
    /// A metrics artifact claims a reported value without one.
    #[error(
        "run directory '{path}' for seed {seed} reports metric '{metric}' as reported with no value"
    )]
    ReportedWithoutValue {
        /// The metrics artifact that claims a value.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The metric that claims a value.
        metric: String,
    },
    /// A seed's metrics artifact does not report a metric another seed reports.
    #[error(
        "seed {seed} reports no status for '{metric}', which another seed of the batch reports; the seeds do not share one metric definition revision"
    )]
    MissingMetric {
        /// The metric the seed does not report.
        metric: String,
        /// The first seed that does not report it.
        seed: u64,
    },
}

/// The batch manifest an aggregation was computed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatedBatch {
    /// The batch manifest's path relative to the batch root.
    pub path: String,
    /// SHA-256 of the batch manifest's exact bytes.
    pub sha256: String,
    /// The batch manifest format version that was read.
    pub batch_manifest_version: u32,
}

/// One seed the aggregation read, with the links to the artifacts it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatedSeed {
    /// Root seed of the run.
    pub seed: u64,
    /// The run directory, relative to the batch root.
    pub directory: String,
    /// SHA-256 of the run's `manifest.json` bytes, as the batch recorded it.
    pub manifest_sha256: String,
    /// SHA-256 of the run's `metrics.json` bytes, as read.
    pub metrics_sha256: String,
}

/// The documented method behind every interval in the artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticsMethod {
    /// The interval method, [`INTERVAL_METHOD`].
    pub confidence_interval_method: String,
    /// The two-sided confidence level, [`CONFIDENCE_LEVEL`].
    pub confidence_level: f64,
    /// The sample variance denominator, `n - 1`.
    pub variance_denominator: String,
    /// The smallest tabulated degrees of freedom.
    pub smallest_tabulated_degrees_of_freedom: u32,
    /// The largest tabulated degrees of freedom.
    pub largest_tabulated_degrees_of_freedom: u32,
    /// The critical value used above the table: the standard normal 97.5%
    /// quantile.
    pub critical_value_above_the_table: f64,
    /// Least reported seeds that yield an interval.
    pub least_interval_seeds: u32,
}

impl StatisticsMethod {
    /// The method fixed by this version of the aggregation format.
    fn v1() -> Self {
        Self {
            confidence_interval_method: INTERVAL_METHOD.to_owned(),
            confidence_level: CONFIDENCE_LEVEL,
            variance_denominator: "n - 1".to_owned(),
            smallest_tabulated_degrees_of_freedom: 1,
            largest_tabulated_degrees_of_freedom: LARGEST_TABULATED_DEGREES_OF_FREEDOM,
            critical_value_above_the_table: NORMAL_CRITICAL_975,
            least_interval_seeds: LEAST_INTERVAL_SEEDS,
        }
    }
}

/// The spread of a metric over the seeds that reported it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    /// Least reported value.
    pub minimum: Option<f64>,
    /// Greatest reported value.
    pub maximum: Option<f64>,
    /// `maximum - minimum`.
    pub range: Option<f64>,
    /// The sample variance with the `n - 1` denominator; absent below two
    /// reported seeds.
    pub sample_variance: Option<f64>,
    /// The sample standard deviation, the square root of the sample variance.
    pub sample_standard_deviation: Option<f64>,
}

impl Spread {
    /// The spread of the reported values.
    pub(crate) fn of(values: &[f64], variance: Option<f64>) -> Self {
        let minimum = values.iter().copied().reduce(f64::min);
        let maximum = values.iter().copied().reduce(f64::max);
        Self {
            minimum,
            maximum,
            range: minimum
                .zip(maximum)
                .map(|(minimum, maximum)| maximum - minimum),
            sample_variance: variance,
            sample_standard_deviation: variance.map(f64::sqrt),
        }
    }
}

/// The two-sided Student-t interval of a metric over the seeds that reported it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    /// The interval method, [`INTERVAL_METHOD`].
    pub method: String,
    /// The two-sided confidence level.
    pub confidence_level: f64,
    /// Degrees of freedom, `count - 1`.
    pub degrees_of_freedom: u32,
    /// The critical value the interval used.
    pub critical_value: f64,
    /// The standard error of the mean, `s / sqrt(count)`.
    pub standard_error: f64,
    /// The half-width, `critical_value * standard_error`.
    pub half_width: f64,
    /// The lower bound, `mean - half_width`.
    pub lower: f64,
    /// The upper bound, `mean + half_width`.
    pub upper: f64,
}

impl ConfidenceInterval {
    /// The interval of the reported values, or `None` when fewer than
    /// [`LEAST_INTERVAL_SEEDS`] were reported.
    fn of(values: &[f64], mean: Option<f64>, variance: Option<f64>) -> Option<Self> {
        Self::labelled(INTERVAL_METHOD, values, mean, variance)
    }

    /// The same two-sided Student-t interval, labelled with `method`.
    ///
    /// The arithmetic is [`INTERVAL_METHOD`]'s in every case; only the method
    /// name a consumer reads differs, so `compare` reports the interval of its
    /// per-seed paired differences through this one construction instead of a
    /// second spelling of the critical-value lookup.
    pub(crate) fn labelled(
        method: &str,
        values: &[f64],
        mean: Option<f64>,
        variance: Option<f64>,
    ) -> Option<Self> {
        let (mean, variance) = mean.zip(variance)?;
        let count = values.len();
        let standard_error = variance.sqrt() / (count as f64).sqrt();
        let degrees_of_freedom = u32::try_from(count - 1).expect("a seed count fits in u32");
        let critical_value = critical_value(degrees_of_freedom);
        let half_width = critical_value * standard_error;
        Some(Self {
            method: method.to_owned(),
            confidence_level: CONFIDENCE_LEVEL,
            degrees_of_freedom,
            critical_value,
            standard_error,
            half_width,
            lower: mean - half_width,
            upper: mean + half_width,
        })
    }
}

/// One metric's across-seed distribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricDistribution {
    /// The metric definition revision these values are reported at.
    pub metric_definition_version: u32,
    /// The metric's unit, as metric definition v2 fixes it.
    pub unit: String,
    /// Reported seeds: the count every statistic here uses.
    pub count: usize,
    /// The mean over the reported seeds.
    pub mean: Option<f64>,
    /// The spread over the reported seeds.
    pub spread: Spread,
    /// The interval over the reported seeds; absent below
    /// [`LEAST_INTERVAL_SEEDS`] reported seeds.
    pub confidence_interval: Option<ConfidenceInterval>,
    /// The seeds that reported a value, ascending.
    pub reported_seeds: Vec<u64>,
    /// The seeds whose value was not applicable, ascending.
    pub not_applicable_seeds: Vec<u64>,
    /// The seeds that made no observation, ascending.
    pub not_observed_seeds: Vec<u64>,
    /// `manifest_sha256` of the reported seeds, in `reported_seeds` order.
    pub manifests: Vec<String>,
}

/// One movement bucket's metric distributions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementSlice {
    /// The bucket's two movement keys, sorted lexicographically.
    pub movement_keys: [String; 2],
    /// The bucket's distributions, keyed by metric name.
    pub metrics: BTreeMap<String, MetricDistribution>,
}

/// `aggregation.json`: the across-seed distributions of one completed batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aggregation {
    /// The aggregation format version.
    pub aggregation_version: u32,
    /// The metric definition revision every number here is reported at.
    pub metric_definition_version: u32,
    /// The batch manifest this aggregation read.
    pub batch: AggregatedBatch,
    /// The documented method behind every interval.
    pub statistics: StatisticsMethod,
    /// The seeds aggregated, ascending.
    pub seeds: Vec<AggregatedSeed>,
    /// The whole-batch distributions, keyed by metric name.
    pub metrics: BTreeMap<String, MetricDistribution>,
    /// The mode-pair slices, keyed by `ModePair` label.
    pub mode_pair_slices: BTreeMap<String, BTreeMap<String, MetricDistribution>>,
    /// The mode slices, keyed by one `AgentMode` label (`vehicle`,
    /// `pedestrian`), each holding `event_counts.<family>` for every counted
    /// family.
    pub mode_event_slices: BTreeMap<String, BTreeMap<String, MetricDistribution>>,
    /// The movement slices, keyed by the pair's two movement keys.
    pub movement_slices: BTreeMap<String, MovementSlice>,
    /// The agent-movement slices, keyed by one movement key
    /// (`movement:<name>` or `pedestrian_route:<name>`), each holding the ten
    /// operational metrics of one agent's own movement plus
    /// `event_counts.<family>` for every counted family.
    pub agent_movement_slices: BTreeMap<String, BTreeMap<String, MetricDistribution>>,
}

/// Aggregate a completed batch: read its manifest and every seed's metrics, and
/// compute the across-seed distribution of every metric the runs report.
///
/// The batch root must hold a `batch.json` and, for every run it names, the
/// run's `manifest.json` and `metrics.json`. Every artifact is read and
/// cross-checked against the batch manifest's record before it is aggregated,
/// so a run directory that changed after the batch finished is refused rather
/// than silently averaged. Nothing is written: the caller writes the returned
/// artifact.
pub fn aggregate_batch(root: &Path) -> Result<Aggregation, AggregateError> {
    let batch_path = root.join(BATCH_MANIFEST_FILE);
    let batch_bytes = read(&batch_path, "read batch manifest")?;
    let batch: BatchManifest =
        serde_json::from_slice(&batch_bytes).map_err(|source| AggregateError::BatchManifest {
            path: batch_path.clone(),
            source,
        })?;

    // The batch manifest already orders runs by ascending seed; sort again so
    // the aggregation's order is its own property, not its input's.
    let mut runs = batch.runs.clone();
    runs.sort_by_key(|run| run.seed);
    if let Some(seed) = runs
        .windows(2)
        .find(|pair| pair[0].seed == pair[1].seed)
        .map(|pair| pair[0].seed)
    {
        return Err(AggregateError::DuplicateSeed {
            path: batch_path,
            seed,
        });
    }
    let seeds: Vec<u64> = runs.iter().map(|run| run.seed).collect();

    let mut accumulation = Accumulation::default();
    let mut aggregated_seeds = Vec::with_capacity(runs.len());
    for run in &runs {
        let directory = root.join(&run.directory);
        let manifest_path = directory.join(MANIFEST_FILE);
        let manifest_sha256 = sha256_hex(&read(&manifest_path, "read run manifest")?);
        if manifest_sha256 != run.manifest_sha256 {
            return Err(AggregateError::ManifestDigest {
                path: manifest_path,
                seed: run.seed,
                recorded: run.manifest_sha256.clone(),
                observed: manifest_sha256,
            });
        }

        let metrics_path = directory.join(METRICS_FILE);
        let metrics_bytes = read(&metrics_path, "read run metrics")?;
        let metrics: RunMetricsArtifact =
            serde_json::from_slice(&metrics_bytes).map_err(|source| AggregateError::Metrics {
                path: directory.clone(),
                seed: run.seed,
                file: METRICS_FILE,
                source,
            })?;
        if metrics.metric_definition_version != METRIC_DEFINITION_VERSION {
            return Err(AggregateError::DefinitionVersion {
                path: metrics_path,
                seed: run.seed,
                version: metrics.metric_definition_version,
                expected: METRIC_DEFINITION_VERSION,
            });
        }
        if metrics.manifest_sha256 != run.manifest_sha256 {
            return Err(AggregateError::ManifestLink {
                path: metrics_path,
                seed: run.seed,
                recorded: run.manifest_sha256.clone(),
                observed: metrics.manifest_sha256.clone(),
            });
        }

        accumulation.accumulate(run.seed, &run.manifest_sha256, &metrics, &metrics_path)?;
        aggregated_seeds.push(AggregatedSeed {
            seed: run.seed,
            directory: run.directory.clone(),
            manifest_sha256: run.manifest_sha256.clone(),
            metrics_sha256: sha256_hex(&metrics_bytes),
        });
    }

    let batch = AggregatedBatch {
        path: BATCH_MANIFEST_FILE.to_owned(),
        sha256: sha256_hex(&batch_bytes),
        batch_manifest_version: batch.batch_manifest_version,
    };
    accumulation.finish(&seeds, aggregated_seeds, batch)
}

/// Read a file, or fail with the path and the operation that targeted it.
fn read(path: &Path, action: &'static str) -> Result<Vec<u8>, AggregateError> {
    fs::read(path).map_err(|source| AggregateError::Io {
        path: path.to_path_buf(),
        action,
        source,
    })
}

/// The two-sided 95% critical value for `degrees_of_freedom`.
///
/// Tabulated for 1 to [`LARGEST_TABULATED_DEGREES_OF_FREEDOM`] degrees of
/// freedom; above the table the standard normal 97.5% quantile stands in.
fn critical_value(degrees_of_freedom: u32) -> f64 {
    debug_assert!(
        degrees_of_freedom >= 1,
        "an interval has a degree of freedom"
    );
    T_CRITICAL_975
        .get(degrees_of_freedom as usize - 1)
        .copied()
        .unwrap_or(NORMAL_CRITICAL_975)
}

/// The mean of the reported values, `None` when none were reported.
pub(crate) fn sample_mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// The sample variance of the reported values with the `n - 1` denominator,
/// `None` below two reported values.
pub(crate) fn sample_variance(values: &[f64], mean: Option<f64>) -> Option<f64> {
    match (values.len(), mean) {
        (count, Some(mean)) if count >= 2 => Some(
            values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / (count as f64 - 1.0),
        ),
        _ => None,
    }
}

/// One seed's reading of a run-level metric: a status-bearing value, or a count
/// the run artifact always reports (zero when the family is absent).
pub(crate) enum Reading<'a> {
    /// A metric value with its reporting status.
    Value(&'a MetricValue),
    /// An always-reported count.
    Count(u64),
}

/// The run-level metrics one seed's artifact reports, each with the unit metric
/// definition v2 fixes for it.
///
/// The metric set is the artifact's own: the three interaction minima, the
/// total, every counted event family, every counted variant kind, and the
/// operational families of metric definition v2 — throughput, delay, and queues
/// — over the run and per mode. The operational movement level is deliberately
/// not read here: a movement bucket is sparse across seeds, and the aggregation
/// counts a whole-batch metric that a seed does not report at all as a broken
/// comparison rather than as an unobserved slice. That level is carried by
/// [`Aggregation::agent_movement_slices`] instead, and the event families per
/// mode by [`Aggregation::mode_event_slices`].
pub(crate) fn run_level_readings(
    artifact: &RunMetricsArtifact,
) -> Vec<(String, &'static str, Reading<'_>)> {
    let mut readings = vec![
        (
            "minimum_ttc_s".to_owned(),
            SECONDS,
            Reading::Value(&artifact.minimum_ttc_s),
        ),
        (
            "minimum_separation_m".to_owned(),
            METRES,
            Reading::Value(&artifact.minimum_separation_m),
        ),
        (
            "minimum_post_encroachment_s".to_owned(),
            SECONDS,
            Reading::Value(&artifact.minimum_post_encroachment_s),
        ),
        (
            "event_counts.total".to_owned(),
            RECORDS,
            Reading::Count(artifact.event_counts.total),
        ),
    ];
    for (family, count) in &artifact.event_counts.by_family {
        readings.push((
            format!("event_counts.by_family.{family}"),
            RECORDS,
            Reading::Count(*count),
        ));
    }
    for (family, kinds) in &artifact.event_counts.by_family_kind {
        for (kind, count) in kinds {
            readings.push((
                format!("event_counts.by_family_kind.{family}.{kind}"),
                RECORDS,
                Reading::Count(*count),
            ));
        }
    }
    for (name, unit, value) in operational_readings(&artifact.operational.run) {
        readings.push((
            format!("operational.run.{name}"),
            unit,
            Reading::Value(value),
        ));
    }
    for (mode, values) in &artifact.operational.by_mode {
        for (name, unit, value) in operational_readings(values) {
            readings.push((
                format!("operational.by_mode.{mode}.{name}"),
                unit,
                Reading::Value(value),
            ));
        }
    }
    readings
}

/// The metrics one operational bucket reports, each with its unit.
pub(crate) fn operational_readings(
    values: &OperationalValues,
) -> [(&'static str, &'static str, &MetricValue); 10] {
    [
        (
            "throughput_agents_per_s",
            AGENTS_PER_SECOND,
            &values.throughput_agents_per_s,
        ),
        ("mean_travel_time_s", SECONDS, &values.mean_travel_time_s),
        ("total_travel_time_s", SECONDS, &values.total_travel_time_s),
        (
            "mean_stopped_delay_s",
            SECONDS,
            &values.mean_stopped_delay_s,
        ),
        (
            "total_stopped_delay_s",
            SECONDS,
            &values.total_stopped_delay_s,
        ),
        (
            "mean_control_delay_s",
            SECONDS,
            &values.mean_control_delay_s,
        ),
        (
            "total_control_delay_s",
            SECONDS,
            &values.total_control_delay_s,
        ),
        (
            "maximum_queue_length_agents",
            AGENTS,
            &values.maximum_queue_length_agents,
        ),
        (
            "maximum_queue_duration_s",
            SECONDS,
            &values.maximum_queue_duration_s,
        ),
        (
            "mean_queue_duration_s",
            SECONDS,
            &values.mean_queue_duration_s,
        ),
    ]
}

/// The three metrics one movement bucket reports, each with its unit.
pub(crate) fn movement_readings(
    minima: &MovementMinima,
) -> [(&'static str, &'static str, &MetricValue); 3] {
    [
        ("minimum_separation_m", METRES, &minima.minimum_separation_m),
        ("minimum_ttc_s", SECONDS, &minima.minimum_ttc_s),
        (
            "minimum_post_encroachment_s",
            SECONDS,
            &minima.minimum_post_encroachment_s,
        ),
    ]
}

/// One mode's counted event families, keyed by the slice's metric name.
///
/// Every family the run counts is present, `0` when that mode produced no record
/// of it: the run artifact's `by_family` set always holds all ten, so a family
/// missing from the sparse `by_family_mode` slice is an observed zero rather
/// than an absent value.
pub(crate) fn mode_event_readings(
    counts: &EventCounts,
    mode: &str,
) -> Vec<(String, &'static str, u64)> {
    EVENT_FAMILY_LABELS
        .into_iter()
        .map(|family| {
            (
                format!("{EVENT_COUNT_PREFIX}{family}"),
                RECORDS,
                counts
                    .by_family_mode
                    .get(family)
                    .and_then(|modes| modes.get(mode))
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect()
}

/// One agent movement's counted event families, keyed by the slice's metric
/// name, under the same complete-set rule as [`mode_event_readings`].
pub(crate) fn agent_movement_event_readings(
    counts: &EventCounts,
    movement: &str,
) -> Vec<(String, &'static str, u64)> {
    EVENT_FAMILY_LABELS
        .into_iter()
        .map(|family| {
            (
                format!("{EVENT_COUNT_PREFIX}{family}"),
                RECORDS,
                counts
                    .by_family_movement
                    .get(family)
                    .and_then(|movements| movements.get(movement))
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect()
}

/// Every movement key the run attributed a counted record to.
pub(crate) fn recorded_movement_keys(counts: &EventCounts) -> BTreeSet<&str> {
    counts
        .by_family_movement
        .values()
        .flat_map(|movements| movements.keys().map(String::as_str))
        .collect()
}

/// The running per-seed readings of one metric.
#[derive(Debug)]
struct Accumulator {
    /// The metric's unit.
    unit: &'static str,
    /// The reported seeds with their values and their run manifest hashes.
    reported: Vec<(u64, f64, String)>,
    /// The seeds whose value was not applicable.
    not_applicable: Vec<u64>,
    /// The seeds that made no observation.
    not_observed: Vec<u64>,
}

impl Accumulator {
    /// An accumulator for a metric in `unit`.
    fn new(unit: &'static str) -> Self {
        Self {
            unit,
            reported: Vec::new(),
            not_applicable: Vec::new(),
            not_observed: Vec::new(),
        }
    }

    /// Record one seed's reading of a status-bearing metric value.
    fn reading(
        &mut self,
        seed: u64,
        manifest: &str,
        metric: &str,
        value: &MetricValue,
        path: &Path,
    ) -> Result<(), AggregateError> {
        match value.status {
            MetricStatus::Reported => match value.value {
                Some(value) => {
                    self.reported.push((seed, value, manifest.to_owned()));
                    Ok(())
                }
                None => Err(AggregateError::ReportedWithoutValue {
                    path: path.to_path_buf(),
                    seed,
                    metric: metric.to_owned(),
                }),
            },
            MetricStatus::NotApplicable => {
                self.not_applicable.push(seed);
                Ok(())
            }
            MetricStatus::NotObserved => {
                self.not_observed.push(seed);
                Ok(())
            }
        }
    }

    /// Record one seed's reading of an always-reported count, which is a value
    /// like any other: a reported zero is a measured zero.
    fn count(&mut self, seed: u64, manifest: &str, count: u64) {
        self.reported
            .push((seed, count as f64, manifest.to_owned()));
    }

    /// The seeds this metric holds no status for, ascending.
    fn seeds_without_status(&self, seeds: &[u64]) -> Vec<u64> {
        let known: BTreeSet<u64> = self
            .reported
            .iter()
            .map(|(seed, _, _)| *seed)
            .chain(self.not_applicable.iter().copied())
            .chain(self.not_observed.iter().copied())
            .collect();
        seeds
            .iter()
            .copied()
            .filter(|seed| !known.contains(seed))
            .collect()
    }

    /// Count every seed this metric holds no status for as not observed.
    fn fill_not_observed(&mut self, seeds: &[u64]) {
        let missing = self.seeds_without_status(seeds);
        self.not_observed.extend(missing);
    }

    /// Close the accumulator into the metric's across-seed distribution.
    ///
    /// The seed lists are sorted here, so a distribution's order is a property
    /// of the distribution and not of the order the seeds were read in.
    fn finish(mut self) -> MetricDistribution {
        self.reported.sort_by_key(|(seed, _, _)| *seed);
        self.not_applicable.sort_unstable();
        self.not_observed.sort_unstable();

        let values: Vec<f64> = self.reported.iter().map(|(_, value, _)| *value).collect();
        let mean = sample_mean(&values);
        let variance = sample_variance(&values, mean);
        let reported_seeds: Vec<u64> = self.reported.iter().map(|(seed, _, _)| *seed).collect();
        let manifests: Vec<String> = self
            .reported
            .into_iter()
            .map(|(_, _, manifest)| manifest)
            .collect();

        MetricDistribution {
            metric_definition_version: METRIC_DEFINITION_VERSION,
            unit: self.unit.to_owned(),
            count: values.len(),
            mean,
            spread: Spread::of(&values, variance),
            confidence_interval: ConfidenceInterval::of(&values, mean, variance),
            reported_seeds,
            not_applicable_seeds: self.not_applicable,
            not_observed_seeds: self.not_observed,
            manifests,
        }
    }
}

/// One slice's per-metric accumulators, filled seed by seed.
type SliceAccumulation = BTreeMap<String, Accumulator>;

/// One movement bucket's accumulators.
#[derive(Debug)]
struct MovementAccumulation {
    /// The bucket's two movement keys, as the run artifacts spell them.
    keys: [String; 2],
    /// The bucket's per-metric accumulators.
    metrics: BTreeMap<String, Accumulator>,
}

/// The per-metric accumulators of one aggregation, filled seed by seed.
#[derive(Debug, Default)]
struct Accumulation {
    /// The whole-batch accumulators, keyed by metric name.
    metrics: BTreeMap<String, Accumulator>,
    /// The mode-pair slices, keyed by `ModePair` label.
    mode_pairs: BTreeMap<String, Accumulator>,
    /// The mode event slices, keyed by `AgentMode` label.
    modes: BTreeMap<String, SliceAccumulation>,
    /// The pair movement slices, keyed by the pair's two movement keys.
    movements: BTreeMap<String, MovementAccumulation>,
    /// The agent movement slices, keyed by one movement key.
    agent_movements: BTreeMap<String, SliceAccumulation>,
}

impl Accumulation {
    /// Read one seed's artifact into every accumulator it belongs to.
    fn accumulate(
        &mut self,
        seed: u64,
        manifest: &str,
        artifact: &RunMetricsArtifact,
        path: &Path,
    ) -> Result<(), AggregateError> {
        for (key, unit, reading) in run_level_readings(artifact) {
            let accumulator = self
                .metrics
                .entry(key.clone())
                .or_insert_with(|| Accumulator::new(unit));
            match reading {
                Reading::Value(value) => accumulator.reading(seed, manifest, &key, value, path)?,
                Reading::Count(count) => accumulator.count(seed, manifest, count),
            }
        }

        for (label, value) in &artifact.mode_pair_minimum_separation_m {
            let accumulator = self
                .mode_pairs
                .entry(label.clone())
                .or_insert_with(|| Accumulator::new(METRES));
            accumulator.reading(seed, manifest, MODE_PAIR_METRIC, value, path)?;
        }

        // Every mode the artifact reports carries every counted family, `0` for
        // a family it produced no record of, so the mode slice is complete.
        for mode in artifact.operational.by_mode.keys() {
            for (key, unit, count) in mode_event_readings(&artifact.event_counts, mode) {
                self.modes
                    .entry(mode.clone())
                    .or_default()
                    .entry(key)
                    .or_insert_with(|| Accumulator::new(unit))
                    .count(seed, manifest, count);
            }
        }

        for (bucket, minima) in &artifact.movement_minima {
            let slice =
                self.movements
                    .entry(bucket.clone())
                    .or_insert_with(|| MovementAccumulation {
                        keys: minima.movement_keys.clone(),
                        metrics: BTreeMap::new(),
                    });
            for (name, unit, value) in movement_readings(minima) {
                let accumulator = slice
                    .metrics
                    .entry(name.to_owned())
                    .or_insert_with(|| Accumulator::new(unit));
                accumulator.reading(seed, manifest, name, value, path)?;
            }
        }

        // One agent movement's own slice: its operational values and its counted
        // event families. The bucket exists for a movement the run placed an
        // agent on, so a bucket a run does not carry is no observation there.
        let mut agent_buckets: BTreeSet<&str> = artifact
            .operational
            .by_movement
            .keys()
            .map(String::as_str)
            .collect();
        agent_buckets.extend(recorded_movement_keys(&artifact.event_counts));
        for bucket in agent_buckets {
            let slice = self.agent_movements.entry(bucket.to_owned()).or_default();
            if let Some(values) = artifact.operational.by_movement.get(bucket) {
                for (name, unit, value) in operational_readings(values) {
                    slice
                        .entry(name.to_owned())
                        .or_insert_with(|| Accumulator::new(unit))
                        .reading(seed, manifest, name, value, path)?;
                }
            }
            for (key, unit, count) in agent_movement_event_readings(&artifact.event_counts, bucket)
            {
                slice
                    .entry(key)
                    .or_insert_with(|| Accumulator::new(unit))
                    .count(seed, manifest, count);
            }
        }
        Ok(())
    }

    /// Close every accumulator into the aggregation artifact.
    ///
    /// A whole-batch metric must cover every seed: a metric one seed's artifact
    /// does not report at all would otherwise be silently averaged over fewer
    /// seeds than the batch ran. A slice bucket is sparse by nature — a
    /// movement pair only exists for a run whose agents occupy those movements —
    /// so a bucket a run does not carry is counted `not_observed` instead.
    fn finish(
        self,
        seeds: &[u64],
        seeds_read: Vec<AggregatedSeed>,
        batch: AggregatedBatch,
    ) -> Result<Aggregation, AggregateError> {
        let mut metrics = BTreeMap::new();
        for (key, accumulator) in self.metrics {
            if let Some(seed) = accumulator.seeds_without_status(seeds).first() {
                return Err(AggregateError::MissingMetric {
                    metric: key,
                    seed: *seed,
                });
            }
            metrics.insert(key, accumulator.finish());
        }

        let mode_pair_slices = self
            .mode_pairs
            .into_iter()
            .map(|(label, mut accumulator)| {
                accumulator.fill_not_observed(seeds);
                let slice = BTreeMap::from([(MODE_PAIR_METRIC.to_owned(), accumulator.finish())]);
                (label, slice)
            })
            .collect();

        let mode_event_slices = finish_slices(self.modes, seeds);

        let movement_slices = self
            .movements
            .into_iter()
            .map(|(bucket, accumulation)| {
                let MovementAccumulation { keys, metrics } = accumulation;
                let metrics = metrics
                    .into_iter()
                    .map(|(name, mut accumulator)| {
                        accumulator.fill_not_observed(seeds);
                        (name, accumulator.finish())
                    })
                    .collect();
                (
                    bucket,
                    MovementSlice {
                        movement_keys: keys,
                        metrics,
                    },
                )
            })
            .collect();

        Ok(Aggregation {
            aggregation_version: AGGREGATION_VERSION,
            metric_definition_version: METRIC_DEFINITION_VERSION,
            batch,
            statistics: StatisticsMethod::v1(),
            seeds: seeds_read,
            metrics,
            mode_pair_slices,
            mode_event_slices,
            movement_slices,
            agent_movement_slices: finish_slices(self.agent_movements, seeds),
        })
    }
}

/// Close every accumulator of a slice family into its distributions.
///
/// A slice bucket is sparse across seeds, so a seed that does not carry the
/// bucket is counted `not_observed` for every metric of that bucket rather than
/// shrinking the sample.
fn finish_slices(
    slices: BTreeMap<String, SliceAccumulation>,
    seeds: &[u64],
) -> BTreeMap<String, BTreeMap<String, MetricDistribution>> {
    slices
        .into_iter()
        .map(|(key, metrics)| {
            let metrics = metrics
                .into_iter()
                .map(|(name, mut accumulator)| {
                    accumulator.fill_not_observed(seeds);
                    (name, accumulator.finish())
                })
                .collect();
            (key, metrics)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The critical-value table is ascending, covers one to thirty degrees of
    /// freedom, and hands everything above it to the normal quantile.
    #[test]
    fn the_critical_value_table_covers_one_to_thirty_degrees_of_freedom() {
        assert_eq!(T_CRITICAL_975.len(), 30);
        assert!(T_CRITICAL_975.windows(2).all(|pair| pair[0] > pair[1]));
        assert_eq!(critical_value(1), T_CRITICAL_975[0]);
        assert_eq!(critical_value(30), T_CRITICAL_975[29]);
        assert_eq!(critical_value(31), NORMAL_CRITICAL_975);
        assert_eq!(critical_value(1_000), NORMAL_CRITICAL_975);
        assert!(critical_value(31) < critical_value(30));
    }

    /// The interval is the symmetric Student-t interval on the sample mean.
    #[test]
    fn the_interval_is_the_two_sided_student_t_interval() {
        let values = [1.0, 2.0, 3.0, 4.0];
        let mean = sample_mean(&values);
        let variance = sample_variance(&values, mean);
        assert_eq!(mean, Some(2.5));
        assert_eq!(variance, Some(5.0 / 3.0));

        let interval =
            ConfidenceInterval::of(&values, mean, variance).expect("four values have one");
        assert_eq!(interval.method, INTERVAL_METHOD);
        assert_eq!(interval.confidence_level, CONFIDENCE_LEVEL);
        assert_eq!(interval.degrees_of_freedom, 3);
        assert_eq!(interval.critical_value, T_CRITICAL_975[2]);
        let standard_error = (5.0_f64 / 3.0).sqrt() / 2.0;
        assert!((interval.standard_error - standard_error).abs() < 1e-12);
        let half_width = T_CRITICAL_975[2] * standard_error;
        assert!((interval.half_width - half_width).abs() < 1e-12);
        assert!((interval.lower - (2.5 - half_width)).abs() < 1e-12);
        assert!((interval.upper - (2.5 + half_width)).abs() < 1e-12);

        // One value measures no spread, so it has no interval and no variance.
        let single = [7.0];
        let mean = sample_mean(&single);
        assert_eq!(mean, Some(7.0));
        let variance = sample_variance(&single, mean);
        assert_eq!(variance, None);
        assert_eq!(ConfidenceInterval::of(&single, mean, variance), None);

        // No value at all is not a zero.
        assert_eq!(sample_mean(&[]), None);
        assert_eq!(sample_variance(&[], None), None);
    }

    /// The spread reports the least, the greatest, the range, and the
    /// Bessel-corrected dispersion.
    #[test]
    fn the_spread_is_the_range_and_the_sample_dispersion() {
        let spread = Spread::of(&[1.0, 3.0], Some(2.0));
        assert_eq!(spread.minimum, Some(1.0));
        assert_eq!(spread.maximum, Some(3.0));
        assert_eq!(spread.range, Some(2.0));
        assert_eq!(spread.sample_standard_deviation, Some(2.0_f64.sqrt()));

        let empty = Spread::of(&[], None);
        assert_eq!(empty.minimum, None);
        assert_eq!(empty.range, None);
        assert_eq!(empty.sample_variance, None);
    }

    /// An accumulator records the seeds behind each status separately.
    #[test]
    fn an_accumulator_keeps_the_seeds_behind_each_status() {
        let reported = MetricValue {
            status: MetricStatus::Reported,
            value: Some(2.0),
            agent: Some(0),
            other: Some(1),
            mode_pair: Some("vehicle_vehicle".to_owned()),
            tick: None,
        };
        let absent = |status| MetricValue {
            status,
            value: None,
            agent: None,
            other: None,
            mode_pair: None,
            tick: None,
        };
        let path = Path::new("metrics.json");
        let mut accumulator = Accumulator::new(SECONDS);
        accumulator
            .reading(1, "b", "metric", &reported, path)
            .unwrap();
        accumulator
            .reading(0, "a", "metric", &reported, path)
            .unwrap();
        accumulator
            .reading(2, "c", "metric", &absent(MetricStatus::NotApplicable), path)
            .unwrap();
        accumulator
            .reading(3, "d", "metric", &absent(MetricStatus::NotObserved), path)
            .unwrap();
        assert_eq!(accumulator.seeds_without_status(&[0, 1, 2, 3, 4]), vec![4]);

        let distribution = accumulator.finish();
        assert_eq!(distribution.count, 2);
        assert_eq!(distribution.mean, Some(2.0));
        assert_eq!(distribution.reported_seeds, vec![0, 1]);
        assert_eq!(distribution.not_applicable_seeds, vec![2]);
        assert_eq!(distribution.not_observed_seeds, vec![3]);
        assert_eq!(distribution.manifests, vec!["a".to_owned(), "b".to_owned()]);
    }
}
