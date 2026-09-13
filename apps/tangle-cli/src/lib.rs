//! Headless command-line interface for Tangle.
//!
//! The binary in `main.rs` is a thin argument parser; the reusable work lives
//! here so tests and future tooling can load a scenario, check it, and produce
//! the same canonical trace without spawning a process.
//!
//! This crate is an application: it reads scenario files from disk and it owns
//! the run-directory output layer, including the Parquet trajectory artifact.
//! The kernel crates remain filesystem-free and free of those dependencies, and
//! this crate depends on them rather than the other way around.

mod aggregate;
mod baseline;
mod batch;
mod compare;
mod converge;
mod experiment;
mod replay;
mod run_dir;
mod run_metrics;
mod seed_bank;
mod trace;
mod trajectories;
mod validate;

use std::path::{Path, PathBuf};

use tangle_model::{
    CompiledScenario, Diagnostic, DocumentReadError, MIGRATION_VERSION, ParseError,
    ScenarioDocument, migrate_v1_to_v2, parse_scenario_document, to_canonical_v2_json,
};

pub use aggregate::{
    AGGREGATION_FILE, AGGREGATION_VERSION, AggregateError, AggregatedBatch, AggregatedSeed,
    Aggregation, CONFIDENCE_LEVEL, ConfidenceInterval, INTERVAL_METHOD,
    LARGEST_TABULATED_DEGREES_OF_FREEDOM, LEAST_INTERVAL_SEEDS, MetricDistribution,
    NORMAL_CRITICAL_975, Spread, StatisticsMethod, T_CRITICAL_975, aggregate_batch,
};
pub use baseline::{
    BASELINE_VERSION, Baseline, CaptureError, CaptureRequest, Convergence,
    PERFORMANCE_REPORT_VERSION, PRESETS, PerformanceReport, Preset, PresetPerformance, PresetTrace,
    ScenarioProvenance, capture,
};
pub use batch::{
    BATCH_MANIFEST_FILE, BATCH_MANIFEST_VERSION, BatchError, BatchManifest, BatchRequest, BatchRun,
    BatchSpec, run_batch,
};
pub use compare::{
    COMPARISON_FILE, COMPARISON_VERSION, CompareError, ComparedBatch, ComparedMovementSlice,
    ComparedPair, ComparedRun, Comparison, PAIRED_DIFFERENCE, PAIRED_INTERVAL_METHOD,
    PairedDistribution, PairedMethod, Side, UnpairedSeed, compare_batches,
};
pub use converge::{
    CONTINUOUS_CLASS, CONVERGENCE_COUNT_TOLERANCE, CONVERGENCE_FILE, CONVERGENCE_TOLERANCE,
    CONVERGENCE_VERSION, COUNT_CLASS, COUNT_UNITS, ConvergenceError, ConvergenceReport,
    ConvergenceVerdict, FidelityBatch, FidelityValue, MetricSensitivity, MetricTolerance,
    MovementSensitivity, RefinementStep, SLICE_FAMILIES, SLICE_KEY_RUN, SliceFamily,
    SliceSensitivities, TOLERANCE_MEASURE, TOLERANCE_REFERENCE, TOLERANCE_RULE, Tolerance,
    VERDICT_REFINEMENT, converge_batches, fidelity_ticks, standard_fidelity,
};
pub use experiment::{
    CONVERGENCE_EVIDENCE_FILE, CONVERGENCE_EVIDENCE_VERSION, CONVERGENCE_RUN_DIR,
    CONVERGENCE_SUMMARY_FILE, EXPERIMENT_VERSION, ExperimentConvergence, ExperimentError,
    ExperimentReference, ExperimentReport, ExperimentSpec, ExperimentVariant, FINDING_DIFFERENCE,
    FINDING_DIRECTION_RULE, FINDING_FIDELITY, FINDING_REFERENCE_FIDELITY, FindingDirection,
    FindingEvidence, FindingMethod, MetricConvergence, REPORT_FILE, REPORT_VERSION, RUN_SLICE,
    RefinementReading, ReportDistribution, ReportFidelity, ReportLinks, ReportMetric, ReportPaired,
    ReportRun, ReportSection, ReportSlice, ReportVariant, SECTION_ORDER, SLICE_ORDER,
    SelectedFinding, SliceConvergence, SliceKind, render_convergence_summary, run_experiment,
    run_experiment_convergence,
};
pub use replay::{ReplayError, replay_run_directory};
pub use run_dir::{
    DEFAULT_MAX_TRAJECTORY_SAMPLES, DEFAULT_TRAJECTORY_STRIDE_TICKS, EVENT_STREAM_COMPRESSION,
    EVENT_STREAM_FILE, EventRetention, EventStream, MANIFEST_FILE, MANIFEST_TEMP_FILE,
    RUN_MANIFEST_VERSION, RUN_SUMMARY_VERSION, RunDirectoryError, RunDirectoryRequest, RunManifest,
    RunSummary, SAMPLING_POLICY_VERSION, SUMMARY_FILE, SamplingPolicy, TrajectoryRetention,
    TrajectorySampling, write_run_directory,
};
pub use run_metrics::{
    EVENT_FAMILY_LABELS, EventCounts, METRIC_DEFINITION_VERSION, METRICS_FILE, MetricStatus,
    MetricValue, MovementMinima, OperationalMetrics, OperationalValues, RunMetrics,
    RunMetricsArtifact, RunMetricsRecorder,
};
pub use seed_bank::{
    LoadedSeedBank, SEED_BANK_VERSION, SeedBank, SeedBankError, SeedBankReference, read_seed_bank,
};
pub use trace::{
    Trace, TraceRecorder, canonical_run, canonical_run_captured, canonical_run_sampled,
    canonical_trace,
};
pub use trajectories::{
    TRAJECTORY_FILE, TRAJECTORY_FORMAT, TrajectoryArtifact, TrajectoryError, TrajectoryRecorder,
    TrajectorySample, read_trajectories, write_trajectories,
};
pub use validate::{ValidationSummary, render_validation_failure, validate_scenario};

/// Failure to migrate a version-1 scenario source.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    /// The scenario file could not be read.
    #[error("cannot read scenario '{path}': {source}")]
    Read {
        /// The path that was read.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// The document did not match its declared schema version.
    #[error("scenario '{path}' is not a version-1 scenario source: {source}")]
    Document {
        /// The path that was read.
        path: PathBuf,
        /// The structural parse or version-negotiation failure.
        #[source]
        source: DocumentReadError,
    },
    /// The document already declares the supported version, so nothing migrates.
    #[error(
        "scenario '{path}' already declares schema_version 2; migrate rewrites only a \
         version-1 source"
    )]
    AlreadyVersion2 {
        /// The path that was read.
        path: PathBuf,
    },
}

/// Read a version-1 JSON5 scenario file and return the canonical normalized
/// version-2 document.
///
/// The transform is a pure function of the document, so the only file read is
/// the scenario named; an unchanged source always produces the same bytes. A
/// document that is not version 1 is rejected rather than re-emitted, so the
/// result is always the product of the version-1 to version-2 transform.
pub fn migrate_scenario(path: &Path) -> Result<String, MigrateError> {
    let text = std::fs::read_to_string(path).map_err(|source| MigrateError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let document = parse_scenario_document(&text).map_err(|source| MigrateError::Document {
        path: path.to_path_buf(),
        source,
    })?;
    match document {
        ScenarioDocument::V1(source) => Ok(to_canonical_v2_json(&migrate_v1_to_v2(&source))),
        ScenarioDocument::V2(_) => Err(MigrateError::AlreadyVersion2 {
            path: path.to_path_buf(),
        }),
    }
}

/// Failure to load and compile a scenario file.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The scenario file could not be read.
    #[error("cannot read scenario '{path}': {source}")]
    Read {
        /// The path that was read.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// The file was not valid JSON5 for the scenario source schema.
    #[error("scenario '{path}' is not valid JSON5: {source}")]
    Parse {
        /// The path that was read.
        path: PathBuf,
        /// The structural parse failure.
        #[source]
        source: ParseError,
    },
    /// The document declares a schema version this build cannot read.
    #[error("scenario '{path}' declares schema_version {version}, which this build cannot read")]
    UnsupportedSchemaVersion {
        /// The path that was read.
        path: PathBuf,
        /// The schema version the document declared.
        version: u32,
    },
    /// The scenario parsed but failed semantic validation.
    #[error(
        "scenario '{path}' failed validation: {}",
        render_diagnostics(diagnostics)
    )]
    Invalid {
        /// The path that was read.
        path: PathBuf,
        /// Every diagnostic produced by validation, in source order.
        diagnostics: Vec<Diagnostic>,
    },
}

/// Read, parse, validate, and compile a JSON5 scenario file.
///
/// Semantic validation and compilation are the same mandatory steps the kernel
/// requires, so a scenario that loads here is ready to run.
pub fn load_scenario(path: &Path) -> Result<CompiledScenario, LoadError> {
    load_scenario_hashed(path).map(|(scenario, _)| scenario)
}

/// Read, parse, negotiate the schema version, validate, and compile a scenario,
/// also returning the SHA-256 of its raw source bytes.
///
/// The content hash is provenance, not a parse product: it covers the authored
/// bytes exactly as written, including comments and whitespace, so a run
/// manifest can name the artifact it ran.
pub fn load_scenario_hashed(path: &Path) -> Result<(CompiledScenario, String), LoadError> {
    load_scenario_provenance(path)
        .map(|(scenario, provenance)| (scenario, provenance.content_sha256))
}

/// Read, parse, negotiate the schema version, validate, and compile a scenario,
/// also returning the provenance that identifies exactly the bytes it consumed.
///
/// The provenance carries all four inputs of a run: the source schema version,
/// the SHA-256 of the raw source bytes as read, the SHA-256 of the canonical
/// normalized version-2 document the kernel consumed, and the migration applied.
/// A version-1 document is migrated before compilation and records
/// [`MIGRATION_VERSION`]; a native version-2 document is compiled as authored and
/// records migration version zero. Computing the normalized hash is a pure
/// function of the document, so it does not change what the kernel runs.
pub fn load_scenario_provenance(
    path: &Path,
) -> Result<(CompiledScenario, ScenarioProvenance), LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let content_sha256 = trace::sha256_hex(text.as_bytes());
    let document = parse_scenario_document(&text).map_err(|source| match source {
        DocumentReadError::Parse(source) => LoadError::Parse {
            path: path.to_path_buf(),
            source,
        },
        DocumentReadError::UnsupportedVersion { version } => LoadError::UnsupportedSchemaVersion {
            path: path.to_path_buf(),
            version,
        },
    })?;
    let (scenario, normalized_sha256, migration_version) = match document {
        ScenarioDocument::V1(source) => {
            let normalized = to_canonical_v2_json(&migrate_v1_to_v2(&source));
            let scenario =
                CompiledScenario::compile(source).map_err(|diagnostics| LoadError::Invalid {
                    path: path.to_path_buf(),
                    diagnostics,
                })?;
            (
                scenario,
                trace::sha256_hex(normalized.as_bytes()),
                MIGRATION_VERSION,
            )
        }
        ScenarioDocument::V2(source) => {
            let normalized = to_canonical_v2_json(&source);
            let scenario =
                CompiledScenario::compile_v2(source).map_err(|diagnostics| LoadError::Invalid {
                    path: path.to_path_buf(),
                    diagnostics,
                })?;
            (scenario, trace::sha256_hex(normalized.as_bytes()), 0)
        }
    };
    let provenance = ScenarioProvenance {
        id: scenario.id().to_owned(),
        source_path: path.to_string_lossy().into_owned(),
        schema_version: scenario.schema_version(),
        content_sha256,
        normalized_sha256,
        migration_version,
    };
    Ok((scenario, provenance))
}

fn render_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
