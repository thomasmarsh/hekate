//! The `experiment` command: run a checked-in experiment and write its
//! comparison report.
//!
//! An experiment spec names two scenario-only variants of one scenario, the
//! common-random-number seed bank both run, and the fidelity, tick count, and
//! sampling policy every run applies. This module reads that spec, runs each
//! variant into its own immutable run directories through
//! [`run_batch`](crate::batch::run_batch), aggregates each variant's batch
//! through [`aggregate_batch`], pairs the two variants by seed through
//! [`compare_batches`], and writes one `comparison_report.json`: the concise,
//! machine-readable record of what the variants did, disaggregated by mode and
//! by movement and linked to every run manifest behind each number.
//!
//! ## What the report reports
//!
//! Every metric the two variants' run artifacts report appears exactly once, in
//! the one gate section its name belongs to ([`SECTION_ORDER`]): throughput,
//! delay, queues, violations, collisions, near misses, time to collision,
//! post-encroachment time, minimum separation, and the remaining counted event
//! families. Within a section the metrics are grouped by the slice they
//! disaggregate, in the declared [`SLICE_ORDER`]: the run as a whole, one
//! [`AgentMode`](tangle_sim::AgentMode) label, one `ModePair` label, one pair of
//! movement keys, or one agent's own movement key. A metric name whose leaf is a
//! counted family and whose family has no section of its own lands in `events`,
//! so no reported metric can be silently dropped from the report.
//!
//! A slice names a metric the way the slice reads: the run slice carries the
//! aggregation's own whole-batch keys (`operational.run.<metric>`,
//! `event_counts.by_family.<family>`, and the three interaction minima), a mode
//! slice names an operational metric by its leaf because the slice's own key
//! names the mode and an event family as `event_counts.<family>`, and the
//! interaction slices carry the aggregation's own keys. Each record states the
//! section it belongs to, so a reader that flattens the report keeps the
//! grouping.
//!
//! Each record holds both variants' across-seed distributions and, where the
//! comparison paired the slice, the paired difference. The two sides are named
//! ([`ExperimentReport::side_a`] and [`ExperimentReport::side_b`]) and the
//! difference is `side_a - side_b`, exactly as [`PAIRED_DIFFERENCE`] fixes it,
//! so no reader has to guess the sign.
//!
//! ## Every number links to its manifest
//!
//! [`ExperimentReport::links`] states the rule, and the report keeps the table
//! it resolves against: each variant's batch root, its `batch.json` link, and
//! the seed table of [`AggregatedSeed`]s — the seed, its run directory, and the
//! `manifest_sha256` and `metrics_sha256` of that run. A record's
//! `reported_seeds` therefore resolves to the manifests that produced its
//! numbers, and a missing value is a status (`not_applicable_seeds`,
//! `not_observed_seeds`) rather than a fabricated zero. Manifests are linked by
//! hash rather than copied per metric, which is what keeps the report bounded as
//! the metric set grows.
//!
//! ## Determinism and immutability
//!
//! The report is a deterministic function of the checked-in spec, the checked-in
//! scenario sources, the checked-in seed bank, and the run artifacts the batch
//! machinery writes. Every collection is ordered: variants in the spec's
//! declared order, slices in [`SLICE_ORDER`], metrics by ascending key, seeds
//! ascending, and sections in [`SECTION_ORDER`]. The batches go through the
//! batch machinery's resume path, so a completed run directory is never
//! rewritten and re-invoking the command changes nothing but the report bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::aggregate::{
    AggregateError, AggregatedBatch, AggregatedSeed, Aggregation, ConfidenceInterval,
    MetricDistribution, aggregate_batch,
};
use crate::baseline::ScenarioProvenance;
use crate::batch::{BatchError, BatchRequest, run_batch};
use crate::compare::{
    CompareError, Comparison, PAIRED_DIFFERENCE, PairedDistribution, UnpairedSeed, compare_batches,
};
use crate::run_dir::SamplingPolicy;
use crate::run_metrics::{EVENT_FAMILY_LABELS, METRIC_DEFINITION_VERSION};
use crate::seed_bank::{SeedBankError, SeedBankReference, read_seed_bank};
use crate::trace::sha256_hex;
use crate::{LoadError, load_scenario_hashed};

/// Version of the experiment spec format this command reads.
pub const EXPERIMENT_VERSION: u32 = 1;

/// Version of the comparison report format.
pub const REPORT_VERSION: u32 = 1;

/// Default file name of the comparison report.
pub const REPORT_FILE: &str = "comparison_report.json";

/// The experiment spec's fixed-step tolerance when it checks that its own ticks
/// and duration agree: half a microsecond, far below any authored duration.
const DURATION_EPSILON_S: f64 = 1e-6;

/// The rule [`ReportLinks::manifests`] states.
const MANIFEST_LINK_RULE: &str = "a record's reported_seeds name the seeds behind its numbers; that variant's runs table maps each seed to its run directory, manifest_sha256, and metrics_sha256, and every manifest lives at <root>/<directory>/manifest.json";

/// The gate items the report groups its metrics by, in the order it writes them.
///
/// `collisions` is metric definition v1's contacting collision family, which is
/// the contact-event family; `queues` holds the queue statistics and the counted
/// queue events; `events` holds every remaining counted family (region entries
/// and exits, control transitions, yields, spawns, and despawns) and is the
/// catch-all, so a metric the report does not otherwise name is still reported.
pub const SECTION_ORDER: [&str; 10] = [
    "throughput",
    "delay",
    "queues",
    "violations",
    "collisions",
    "near_misses",
    "time_to_collision",
    "post_encroachment_time",
    "minimum_separation",
    "events",
];

/// The kind of slice a group of metrics disaggregates by, in the order the
/// report writes the slice families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SliceKind {
    /// The run as a whole.
    Run,
    /// One `AgentMode` label.
    Mode,
    /// One `ModePair` label.
    ModePair,
    /// One pair of movement keys.
    MovementPair,
    /// One agent's own movement key.
    Movement,
}

impl SliceKind {
    /// The kind's stable artifact key.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Mode => "mode",
            Self::ModePair => "mode_pair",
            Self::MovementPair => "movement_pair",
            Self::Movement => "movement",
        }
    }
}

/// The declared slice order the report writes.
pub const SLICE_ORDER: [SliceKind; 5] = [
    SliceKind::Run,
    SliceKind::Mode,
    SliceKind::ModePair,
    SliceKind::MovementPair,
    SliceKind::Movement,
];

/// The slice key of the run as a whole.
pub const RUN_SLICE: &str = "*";

/// The checked-in experiment spec: the variants, the bank, and the run policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentSpec {
    /// Spec format version, [`EXPERIMENT_VERSION`].
    pub experiment_version: u32,
    /// The experiment's identifier.
    pub id: String,
    /// What the experiment compares.
    pub summary: String,
    /// The Phase 1 fidelity preset every run uses.
    pub fidelity: String,
    /// The preset's fixed physics step in seconds.
    pub step_s: f64,
    /// Fixed steps every run advances.
    pub ticks: u64,
    /// The simulated seconds `ticks` of `step_s` cover.
    pub duration_s: f64,
    /// The seed bank both variants run, relative to the working directory.
    pub seed_bank: String,
    /// The sampling policy every run applies.
    pub sampling: SamplingPolicy,
    /// The data the experiment holds constant.
    pub controlled: Vec<String>,
    /// The one scenario datum the variants differ in.
    pub independent_variable: String,
    /// The variants, in the declared order. The first is side A of every paired
    /// difference.
    pub variants: Vec<ExperimentVariant>,
}

/// One side of an experiment: its name and its scenario source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentVariant {
    /// The variant's name, as the report and the run root spell it.
    pub variant: String,
    /// The variant's scenario source, relative to the working directory.
    pub scenario: String,
}

/// The experiment spec a report was built from, with its content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentReference {
    /// Spec format version that was read.
    pub experiment_version: u32,
    /// The experiment's identifier.
    pub id: String,
    /// The path the spec was read from.
    pub path: String,
    /// SHA-256 of the spec's exact bytes.
    pub content_sha256: String,
}

/// One run of the report's seed table: where a seed's artifacts live.
///
/// The report keeps the aggregation's own seed table, so a record's
/// `reported_seeds` resolve to the run directories and manifest hashes that
/// produced its numbers.
pub type ReportRun = AggregatedSeed;

/// One side of the experiment as run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportVariant {
    /// The variant's name.
    pub variant: String,
    /// The variant's scenario provenance, including its content hash.
    pub scenario: ScenarioProvenance,
    /// The batch root the variant ran into.
    pub root: String,
    /// The variant's `batch.json`, hashed.
    pub batch: AggregatedBatch,
    /// The seeds run, ascending, with each run's directory and artifact hashes.
    pub runs: Vec<ReportRun>,
}

/// One metric's across-seed distribution on one side.
///
/// The distribution's manifest links are its seeds: the variant's `runs` table
/// maps each `reported_seeds` entry to its manifest hash, so the report states
/// the link once per variant instead of once per metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDistribution {
    /// Reported seeds: the count every statistic here uses.
    pub count: usize,
    /// The mean over the reported seeds.
    pub mean: Option<f64>,
    /// Least reported value.
    pub minimum: Option<f64>,
    /// Greatest reported value.
    pub maximum: Option<f64>,
    /// The two-sided 95% Student-t interval; absent below two reported seeds.
    pub confidence_interval: Option<ConfidenceInterval>,
    /// The seeds that reported a value, ascending.
    pub reported_seeds: Vec<u64>,
    /// The seeds whose value was not applicable, ascending.
    pub not_applicable_seeds: Vec<u64>,
    /// The seeds that made no observation, ascending.
    pub not_observed_seeds: Vec<u64>,
}

impl ReportDistribution {
    /// The report's view of one across-seed distribution.
    fn of(distribution: &MetricDistribution) -> Self {
        Self {
            count: distribution.count,
            mean: distribution.mean,
            minimum: distribution.spread.minimum,
            maximum: distribution.spread.maximum,
            confidence_interval: distribution.confidence_interval.clone(),
            reported_seeds: distribution.reported_seeds.clone(),
            not_applicable_seeds: distribution.not_applicable_seeds.clone(),
            not_observed_seeds: distribution.not_observed_seeds.clone(),
        }
    }
}

/// One metric's paired difference between the two variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportPaired {
    /// Paired seeds: the count every statistic here uses.
    pub count: usize,
    /// The mean of the per-seed differences `side_a - side_b`.
    pub mean_difference: Option<f64>,
    /// The paired two-sided 95% Student-t interval of that mean difference.
    pub confidence_interval: Option<ConfidenceInterval>,
    /// The paired seeds, ascending: the seeds the statistic used.
    pub paired_seeds: Vec<u64>,
    /// The seeds the statistic excludes, ascending. Each side's status for such
    /// a seed is in that side's distribution above.
    pub unpaired_seeds: Vec<u64>,
}

impl ReportPaired {
    /// The report's view of one paired distribution.
    fn of(distribution: &PairedDistribution) -> Self {
        Self {
            count: distribution.count,
            mean_difference: distribution.mean_difference,
            confidence_interval: distribution.confidence_interval.clone(),
            paired_seeds: distribution.paired_seeds.clone(),
            unpaired_seeds: distribution
                .unpaired
                .iter()
                .map(|unpaired: &UnpairedSeed| unpaired.seed)
                .collect(),
        }
    }
}

/// One metric's report record: both variants' distributions and their pairing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportMetric {
    /// The metric definition revision every number here is reported at.
    pub metric_definition_version: u32,
    /// The metric's unit, as metric definition v2 fixes it.
    pub unit: String,
    /// The gate section this metric belongs to.
    pub section: String,
    /// Side A's across-seed distribution, absent when side A's aggregation
    /// carries no such slice at all.
    pub a: Option<ReportDistribution>,
    /// Side B's across-seed distribution, absent when side B's aggregation
    /// carries no such slice at all.
    pub b: Option<ReportDistribution>,
    /// The paired difference, absent when the comparison pairs no such slice.
    pub paired: Option<ReportPaired>,
}

/// One slice's metrics: what they disaggregate by and the records themselves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSlice {
    /// The slice's kind, one of [`SLICE_ORDER`].
    pub slice_kind: SliceKind,
    /// The slice key: [`RUN_SLICE`], a mode label, a `ModePair` label, a pair of
    /// movement keys, or one movement key.
    pub slice: String,
    /// The slice's records, keyed by metric name.
    pub metrics: BTreeMap<String, ReportMetric>,
}

/// One gate section of the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSection {
    /// The gate item: one of [`SECTION_ORDER`].
    pub section: String,
    /// The slices that carry a metric of this section, in [`SLICE_ORDER`].
    pub slices: Vec<ReportSlice>,
}

/// How every number in the report resolves to the runs behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportLinks {
    /// The metric definition revision every number is reported at.
    pub metric_definition_version: u32,
    /// The rule that resolves a record's seeds to its manifests.
    pub manifests: String,
    /// The difference the paired numbers are.
    pub paired_difference: String,
}

/// The run policy the report was produced under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportFidelity {
    /// The Phase 1 fidelity preset every run used.
    pub fidelity: String,
    /// The preset's fixed physics step in seconds.
    pub step_s: f64,
    /// Fixed steps every run advanced.
    pub ticks: u64,
    /// The simulated seconds the runs covered.
    pub duration_s: f64,
}

/// `comparison_report.json`: the concise comparison of one experiment's variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentReport {
    /// The report format version.
    pub report_version: u32,
    /// The metric definition revision every number here is reported at.
    pub metric_definition_version: u32,
    /// The experiment spec this report was built from.
    pub experiment: ExperimentReference,
    /// The run policy every variant ran under.
    pub fidelity: ReportFidelity,
    /// The sampling policy every run applied.
    pub sampling: SamplingPolicy,
    /// The seed bank both variants ran.
    pub seed_bank: SeedBankReference,
    /// The variant whose numbers are the minuend of every paired difference.
    pub side_a: String,
    /// The variant whose numbers are the subtrahend.
    pub side_b: String,
    /// How every number resolves to its manifests.
    pub links: ReportLinks,
    /// The variants, in the spec's declared order.
    pub variants: Vec<ReportVariant>,
    /// The report's sections, in [`SECTION_ORDER`].
    pub sections: Vec<ReportSection>,
}

impl ExperimentReport {
    /// Every metric record, with the section, slice, and metric name it lives
    /// under.
    pub fn records(&self) -> Vec<(String, String, String, &ReportMetric)> {
        let mut all = Vec::new();
        for section in &self.sections {
            for slice in &section.slices {
                for (metric, record) in &slice.metrics {
                    all.push((
                        section.section.clone(),
                        slice.slice.clone(),
                        metric.clone(),
                        record,
                    ));
                }
            }
        }
        all
    }

    /// The record for `metric` under the slice keyed `slice`.
    ///
    /// A slice appears in every section that carries one of its metrics, so the
    /// lookup searches all of them rather than the first slice key found.
    pub fn record(&self, slice: &str, metric: &str) -> Option<&ReportMetric> {
        self.sections
            .iter()
            .flat_map(|section| &section.slices)
            .filter(|candidate| candidate.slice == slice)
            .find_map(|slice| slice.metrics.get(metric))
    }

    /// Every slice in the report, in [`SLICE_ORDER`] and ascending key order
    /// within a kind, deduplicated across the sections.
    pub fn slices(&self) -> Vec<(SliceKind, String)> {
        let mut by_kind: BTreeMap<SliceKind, BTreeSet<String>> = BTreeMap::new();
        for section in &self.sections {
            for slice in &section.slices {
                by_kind
                    .entry(slice.slice_kind)
                    .or_default()
                    .insert(slice.slice.clone());
            }
        }
        SLICE_ORDER
            .into_iter()
            .flat_map(|kind| {
                by_kind
                    .remove(&kind)
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |key| (kind, key))
            })
            .collect()
    }
}

/// Failure to run an experiment or to build its report.
#[derive(Debug, thiserror::Error)]
pub enum ExperimentError {
    /// A spec or report file could not be read or written.
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
    /// The experiment spec could not be parsed.
    #[error("experiment spec '{path}' is not readable JSON: {source}")]
    Spec {
        /// The spec that failed to parse.
        path: PathBuf,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// The spec declares fewer than the two variants a comparison needs.
    #[error(
        "experiment spec '{path}' declares {variants} variants; a comparison needs two: a variant to compare against (side A) and one to compare (side B)"
    )]
    Variants {
        /// The spec that names the wrong number of variants.
        path: PathBuf,
        /// The number it declares.
        variants: usize,
    },
    /// The spec names one variant twice.
    #[error("experiment spec '{path}' names the variant '{variant}' more than once")]
    DuplicateVariant {
        /// The spec that repeats the variant.
        path: PathBuf,
        /// The repeated variant name.
        variant: String,
    },
    /// The spec's tick count, step, and duration disagree with each other.
    #[error(
        "experiment spec '{path}' declares the {fidelity} fidelity at a {step_s} s step for {ticks} ticks, which covers {duration_s} s, but its own duration is {declared_duration_s} s"
    )]
    SpecDuration {
        /// The spec that disagrees with itself.
        path: PathBuf,
        /// The fidelity it declares.
        fidelity: String,
        /// The step it declares.
        step_s: f64,
        /// The steps it declares.
        ticks: u64,
        /// The duration it declares.
        declared_duration_s: f64,
        /// The duration its step and ticks cover.
        duration_s: f64,
    },
    /// A variant's scenario could not be loaded.
    #[error("variant '{variant}' of '{path}': {source}")]
    Scenario {
        /// The variant whose scenario failed.
        variant: String,
        /// The spec that names it.
        path: PathBuf,
        /// The loader's failure.
        #[source]
        source: LoadError,
    },
    /// The seed bank could not be read.
    #[error(transparent)]
    SeedBank(#[from] SeedBankError),
    /// A variant's batch could not be run.
    #[error("variant '{variant}': {source}")]
    Batch {
        /// The variant whose batch failed.
        variant: String,
        /// The batch failure.
        #[source]
        source: BatchError,
    },
    /// A variant's batch could not be aggregated.
    #[error("variant '{variant}': {source}")]
    Aggregate {
        /// The variant whose aggregation failed.
        variant: String,
        /// The aggregation failure.
        #[source]
        source: AggregateError,
    },
    /// The two variants' batches could not be paired.
    #[error(transparent)]
    Compare(#[from] CompareError),
}

/// Run one checked-in experiment and build its comparison report.
///
/// Every path in the spec is relative to the working directory, which is the
/// repository root when the command is run from there. Each variant runs into
/// `run_root/<variant>` through the batch machinery, so a completed run
/// directory is never rewritten; the variants are then aggregated, paired over
/// the one bank, and turned into the report. Nothing is written: the caller
/// writes the returned report.
pub fn run_experiment(
    spec_path: &Path,
    run_root: &Path,
    jobs: u64,
) -> Result<ExperimentReport, ExperimentError> {
    let spec = read_spec(spec_path)?;
    let bank = read_seed_bank(Path::new(&spec.seed_bank))?;
    let seed_bank = SeedBankReference {
        path: spec.seed_bank.clone(),
        content_sha256: bank.content_sha256.clone(),
    };

    let mut variants = Vec::with_capacity(spec.variants.len());
    for variant in &spec.variants {
        let (scenario, content_sha256) = load_scenario_hashed(Path::new(&variant.scenario))
            .map_err(|source| ExperimentError::Scenario {
                variant: variant.variant.clone(),
                path: spec_path.to_path_buf(),
                source,
            })?;
        let provenance = ScenarioProvenance {
            id: scenario.id().to_owned(),
            source_path: variant.scenario.clone(),
            schema_version: scenario.schema_version(),
            content_sha256,
        };
        let root = run_root.join(&variant.variant);
        run_batch(BatchRequest {
            root: root.clone(),
            scenario,
            provenance: provenance.clone(),
            ticks: spec.ticks,
            step_s: spec.step_s,
            sampling: spec.sampling,
            seeds: bank.bank.seeds.clone(),
            seed_bank: Some(seed_bank.clone()),
            jobs,
        })
        .map_err(|source| ExperimentError::Batch {
            variant: variant.variant.clone(),
            source,
        })?;
        let aggregation = aggregate_batch(&root).map_err(|source| ExperimentError::Aggregate {
            variant: variant.variant.clone(),
            source,
        })?;
        variants.push((
            ReportVariant {
                variant: variant.variant.clone(),
                scenario: provenance,
                root: root.display().to_string(),
                batch: aggregation.batch.clone(),
                runs: aggregation.seeds.clone(),
            },
            aggregation,
        ));
    }

    let comparison = compare_batches(
        &run_root.join(&spec.variants[0].variant),
        &run_root.join(&spec.variants[1].variant),
        Path::new(&spec.seed_bank),
    )?;

    let slices = report_slices([&variants[0].1, &variants[1].1], &comparison);
    let spec_bytes = read(spec_path, "read experiment spec")?;

    Ok(ExperimentReport {
        report_version: REPORT_VERSION,
        metric_definition_version: METRIC_DEFINITION_VERSION,
        experiment: ExperimentReference {
            experiment_version: spec.experiment_version,
            id: spec.id.clone(),
            path: spec_path.display().to_string(),
            content_sha256: sha256_hex(&spec_bytes),
        },
        fidelity: ReportFidelity {
            fidelity: spec.fidelity.clone(),
            step_s: spec.step_s,
            ticks: spec.ticks,
            duration_s: spec.duration_s,
        },
        sampling: spec.sampling,
        seed_bank,
        side_a: spec.variants[0].variant.clone(),
        side_b: spec.variants[1].variant.clone(),
        links: ReportLinks {
            metric_definition_version: METRIC_DEFINITION_VERSION,
            manifests: MANIFEST_LINK_RULE.to_owned(),
            paired_difference: PAIRED_DIFFERENCE.to_owned(),
        },
        variants: variants.into_iter().map(|(variant, _)| variant).collect(),
        sections: group_sections(&slices),
    })
}

/// Read and check the experiment spec's own consistency.
fn read_spec(path: &Path) -> Result<ExperimentSpec, ExperimentError> {
    let bytes = read(path, "read experiment spec")?;
    let spec: ExperimentSpec =
        serde_json::from_slice(&bytes).map_err(|source| ExperimentError::Spec {
            path: path.to_path_buf(),
            source,
        })?;
    if spec.variants.len() < 2 {
        return Err(ExperimentError::Variants {
            path: path.to_path_buf(),
            variants: spec.variants.len(),
        });
    }
    let names: BTreeSet<&String> = spec
        .variants
        .iter()
        .map(|variant| &variant.variant)
        .collect();
    if names.len() != spec.variants.len() {
        let repeated = spec
            .variants
            .iter()
            .map(|variant| &variant.variant)
            .find(|name| {
                spec.variants
                    .iter()
                    .filter(|variant| &variant.variant == *name)
                    .count()
                    > 1
            })
            .expect("a repeated name exists when the names are not all distinct");
        return Err(ExperimentError::DuplicateVariant {
            path: path.to_path_buf(),
            variant: repeated.clone(),
        });
    }
    let duration_s = spec.ticks as f64 * spec.step_s;
    if (duration_s - spec.duration_s).abs() > DURATION_EPSILON_S {
        return Err(ExperimentError::SpecDuration {
            path: path.to_path_buf(),
            fidelity: spec.fidelity.clone(),
            step_s: spec.step_s,
            ticks: spec.ticks,
            declared_duration_s: spec.duration_s,
            duration_s,
        });
    }
    Ok(spec)
}

/// Read a file, or fail with the path and the operation that targeted it.
fn read(path: &Path, action: &'static str) -> Result<Vec<u8>, ExperimentError> {
    fs::read(path).map_err(|source| ExperimentError::Io {
        path: path.to_path_buf(),
        action,
        source,
    })
}

/// Every slice of the report, in the declared slice order.
///
/// `aggregations` are the variants in the spec's declared order, and
/// `comparison` pairs them with the first as side A and the second as side B.
fn report_slices(aggregations: [&Aggregation; 2], comparison: &Comparison) -> Vec<ReportSlice> {
    let mut slices = Vec::new();

    // The run as a whole. The operational per-mode keys are the mode slices'
    // records below, so they are not repeated here.
    let run_keys: BTreeSet<&String> = aggregations
        .iter()
        .flat_map(|aggregation| aggregation.metrics.keys())
        .filter(|key| mode_of_operational_key(key).is_none())
        .collect();
    let mut run: BTreeMap<String, ReportMetric> = BTreeMap::new();
    for key in run_keys {
        run.insert(
            key.clone(),
            record(
                key,
                aggregations.map(|aggregation| aggregation.metrics.get(key)),
                comparison.metrics.get(key),
            ),
        );
    }
    slices.push(ReportSlice {
        slice_kind: SliceKind::Run,
        slice: RUN_SLICE.to_owned(),
        metrics: run,
    });

    // One slice per mode: the mode's operational values, named by their leaf
    // because the slice names the mode, and the counted event families that
    // mode's own agents produced.
    let mut modes: BTreeSet<&str> = BTreeSet::new();
    for aggregation in aggregations {
        modes.extend(
            aggregation
                .metrics
                .keys()
                .filter_map(|key| mode_of_operational_key(key)),
        );
        modes.extend(aggregation.mode_event_slices.keys().map(String::as_str));
    }
    for mode in modes {
        let mut metrics: BTreeMap<String, ReportMetric> = BTreeMap::new();
        let operational: BTreeSet<&str> = aggregations
            .iter()
            .flat_map(|aggregation| aggregation.metrics.keys())
            .filter(|key| mode_of_operational_key(key) == Some(mode))
            .filter_map(|key| key.rsplit('.').next())
            .collect();
        for leaf in operational {
            let key = format!("operational.by_mode.{mode}.{leaf}");
            metrics.insert(
                leaf.to_owned(),
                record(
                    leaf,
                    aggregations.map(|aggregation| aggregation.metrics.get(&key)),
                    comparison.metrics.get(&key),
                ),
            );
        }
        for key in
            slice_keys(aggregations.map(|aggregation| aggregation.mode_event_slices.get(mode)))
        {
            metrics.insert(
                key.clone(),
                record(
                    key,
                    aggregations.map(|aggregation| {
                        aggregation
                            .mode_event_slices
                            .get(mode)
                            .and_then(|slice| slice.get(key))
                    }),
                    comparison
                        .mode_event_slices
                        .get(mode)
                        .and_then(|slice| slice.get(key)),
                ),
            );
        }
        slices.push(ReportSlice {
            slice_kind: SliceKind::Mode,
            slice: mode.to_owned(),
            metrics,
        });
    }

    // One slice per mode pair: the pair's separation minimum.
    for label in family_keys(
        aggregations
            .iter()
            .flat_map(|aggregation| aggregation.mode_pair_slices.keys()),
    ) {
        let mut metrics: BTreeMap<String, ReportMetric> = BTreeMap::new();
        for key in
            slice_keys(aggregations.map(|aggregation| aggregation.mode_pair_slices.get(&label)))
        {
            metrics.insert(
                key.clone(),
                record(
                    key,
                    aggregations.map(|aggregation| {
                        aggregation
                            .mode_pair_slices
                            .get(&label)
                            .and_then(|slice| slice.get(key))
                    }),
                    comparison
                        .mode_pair_slices
                        .get(&label)
                        .and_then(|slice| slice.get(key)),
                ),
            );
        }
        slices.push(ReportSlice {
            slice_kind: SliceKind::ModePair,
            slice: label,
            metrics,
        });
    }

    // One slice per pair of movement keys: the pair's interaction minima.
    for bucket in family_keys(
        aggregations
            .iter()
            .flat_map(|aggregation| aggregation.movement_slices.keys()),
    ) {
        let metrics = slice_keys(aggregations.map(|aggregation| {
            aggregation
                .movement_slices
                .get(&bucket)
                .map(|slice| &slice.metrics)
        }))
        .into_iter()
        .map(|key| {
            let record = record(
                key,
                aggregations.map(|aggregation| {
                    aggregation
                        .movement_slices
                        .get(&bucket)
                        .and_then(|slice| slice.metrics.get(key))
                }),
                comparison
                    .movement_slices
                    .get(&bucket)
                    .and_then(|slice| slice.metrics.get(key)),
            );
            (key.clone(), record)
        })
        .collect();
        slices.push(ReportSlice {
            slice_kind: SliceKind::MovementPair,
            slice: bucket,
            metrics,
        });
    }

    // One slice per agent movement key: the movement's operational values and
    // the counted families its own agents produced.
    for movement in family_keys(
        aggregations
            .iter()
            .flat_map(|aggregation| aggregation.agent_movement_slices.keys()),
    ) {
        let metrics = slice_keys(
            aggregations.map(|aggregation| aggregation.agent_movement_slices.get(&movement)),
        )
        .into_iter()
        .map(|key| {
            let record = record(
                key,
                aggregations.map(|aggregation| {
                    aggregation
                        .agent_movement_slices
                        .get(&movement)
                        .and_then(|slice| slice.get(key))
                }),
                comparison
                    .agent_movement_slices
                    .get(&movement)
                    .and_then(|slice| slice.get(key)),
            );
            (key.clone(), record)
        })
        .collect();
        slices.push(ReportSlice {
            slice_kind: SliceKind::Movement,
            slice: movement,
            metrics,
        });
    }

    slices
}

/// The union of every slice key the two variants' aggregations carry, ascending.
fn family_keys<'a>(keys: impl IntoIterator<Item = &'a String>) -> BTreeSet<String> {
    keys.into_iter().map(String::clone).collect()
}

/// Every metric key one slice family's two sides carry, ascending.
fn slice_keys(slices: [Option<&BTreeMap<String, MetricDistribution>>; 2]) -> BTreeSet<&String> {
    slices
        .into_iter()
        .flatten()
        .flat_map(|slice| slice.keys())
        .collect()
}

/// One metric's record from the two variants' distributions and their pairing.
///
/// The key comes from the aggregations, so at least one side carries the
/// distribution and its unit is the one the record reports.
fn record(
    metric: &str,
    sides: [Option<&MetricDistribution>; 2],
    paired: Option<&PairedDistribution>,
) -> ReportMetric {
    let unit = sides
        .iter()
        .flatten()
        .map(|distribution| distribution.unit.clone())
        .next()
        .or_else(|| paired.map(|distribution| distribution.unit.clone()))
        .expect("a record's metric is read from a distribution");
    ReportMetric {
        metric_definition_version: METRIC_DEFINITION_VERSION,
        unit,
        section: section_of(metric).to_owned(),
        a: sides[0].map(ReportDistribution::of),
        b: sides[1].map(ReportDistribution::of),
        paired: paired.map(ReportPaired::of),
    }
}

/// The mode an `operational.by_mode.<mode>.<metric>` key names, if it names one.
fn mode_of_operational_key(key: &str) -> Option<&str> {
    let rest = key.strip_prefix("operational.by_mode.")?;
    rest.split('.').next()
}

/// The gate section a metric belongs to.
fn section_of(metric: &str) -> &'static str {
    match metric.rsplit('.').next().unwrap_or(metric) {
        "throughput_agents_per_s" => "throughput",
        "mean_travel_time_s"
        | "total_travel_time_s"
        | "mean_stopped_delay_s"
        | "total_stopped_delay_s"
        | "mean_control_delay_s"
        | "total_control_delay_s" => "delay",
        "maximum_queue_length_agents" | "maximum_queue_duration_s" | "mean_queue_duration_s" => {
            "queues"
        }
        "minimum_ttc_s" => "time_to_collision",
        "minimum_post_encroachment_s" => "post_encroachment_time",
        "minimum_separation_m" => "minimum_separation",
        _ => match event_family(metric) {
            Some("collisions") => "collisions",
            Some("near_misses") => "near_misses",
            Some("violations") => "violations",
            Some("queue_events") => "queues",
            _ => "events",
        },
    }
}

/// The counted event family a metric names, if it names one.
fn event_family(metric: &str) -> Option<&str> {
    let rest = metric.strip_prefix("event_counts.")?;
    let rest = rest
        .strip_prefix("by_family_kind.")
        .or_else(|| rest.strip_prefix("by_family."))
        .unwrap_or(rest);
    let family = rest.split('.').next().unwrap_or(rest);
    EVENT_FAMILY_LABELS.contains(&family).then_some(family)
}

/// Group the slices into the declared gate sections.
///
/// Every metric belongs to exactly one section, so a slice appears in a section
/// only when it carries a metric of that section, and no number is repeated.
fn group_sections(slices: &[ReportSlice]) -> Vec<ReportSection> {
    let mut sections = Vec::new();
    for section in SECTION_ORDER {
        let mut grouped = Vec::new();
        for slice in slices {
            let metrics: BTreeMap<String, ReportMetric> = slice
                .metrics
                .iter()
                .filter(|(_, record)| record.section == section)
                .map(|(metric, record)| (metric.clone(), record.clone()))
                .collect();
            if !metrics.is_empty() {
                grouped.push(ReportSlice {
                    slice_kind: slice.slice_kind,
                    slice: slice.slice.clone(),
                    metrics,
                });
            }
        }
        if !grouped.is_empty() {
            sections.push(ReportSection {
                section: section.to_owned(),
                slices: grouped,
            });
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every counted family resolves to a section, and the families with a gate
    /// item of their own land in it rather than in the catch-all.
    #[test]
    fn every_counted_family_resolves_to_a_section() {
        assert_eq!(section_of("event_counts.collisions"), "collisions");
        assert_eq!(
            section_of("event_counts.by_family.collisions"),
            "collisions"
        );
        assert_eq!(section_of("event_counts.near_misses"), "near_misses");
        assert_eq!(
            section_of("event_counts.by_family_kind.violations.ran_red_light"),
            "violations"
        );
        assert_eq!(
            section_of("event_counts.by_family_kind.violations.crossed_against_signal"),
            "violations"
        );
        assert_eq!(section_of("event_counts.queue_events"), "queues");
        for family in EVENT_FAMILY_LABELS {
            let section = section_of(&format!("event_counts.{family}"));
            assert!(
                SECTION_ORDER.contains(&section),
                "family '{family}' resolves to '{section}', which is not a section"
            );
        }
        // A kinded family with no section of its own is a counted event, and a
        // metric that is not a counted family at all is the catch-all too.
        assert_eq!(
            section_of("event_counts.by_family_kind.control_transitions.signal_stop"),
            "events"
        );
        assert_eq!(section_of("event_counts.total"), "events");
        assert_eq!(section_of("event_counts.by_family.spawns"), "events");
    }

    /// The operational and interaction metric names resolve to their gate items.
    #[test]
    fn the_operational_and_interaction_metrics_resolve_to_their_sections() {
        assert_eq!(
            section_of("operational.run.throughput_agents_per_s"),
            "throughput"
        );
        assert_eq!(section_of("throughput_agents_per_s"), "throughput");
        assert_eq!(section_of("mean_travel_time_s"), "delay");
        assert_eq!(section_of("operational.run.total_control_delay_s"), "delay");
        assert_eq!(section_of("mean_stopped_delay_s"), "delay");
        assert_eq!(section_of("maximum_queue_length_agents"), "queues");
        assert_eq!(section_of("maximum_queue_duration_s"), "queues");
        assert_eq!(section_of("minimum_ttc_s"), "time_to_collision");
        assert_eq!(
            section_of("minimum_post_encroachment_s"),
            "post_encroachment_time"
        );
        assert_eq!(section_of("minimum_separation_m"), "minimum_separation");
    }

    /// An operational by-mode key names its mode, and any other key names none.
    #[test]
    fn an_operational_by_mode_key_names_its_mode() {
        assert_eq!(
            mode_of_operational_key("operational.by_mode.vehicle.throughput_agents_per_s"),
            Some("vehicle")
        );
        assert_eq!(
            mode_of_operational_key("operational.by_mode.pedestrian.mean_travel_time_s"),
            Some("pedestrian")
        );
        assert_eq!(
            mode_of_operational_key("operational.run.mean_travel_time_s"),
            None
        );
        assert_eq!(mode_of_operational_key("event_counts.total"), None);
    }
}
