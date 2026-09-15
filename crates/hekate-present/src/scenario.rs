//! Load a JSON5 scenario file into a compiled scenario.
//!
//! The kernel crates are filesystem-free by contract, so file access belongs to
//! the presentation layer and its applications. Every renderer mirrors the
//! headless CLI's load path exactly so all of them drive the same validated
//! [`CompiledScenario`]: the schema version is negotiated from the document
//! itself, a version-1 source keeps the migration path through
//! [`CompiledScenario::compile`], and a version-2 source compiles directly
//! through [`CompiledScenario::compile_v2`].

use std::path::{Path, PathBuf};

use hekate_model::{
    CompiledScenario, Diagnostic, DocumentReadError, ParseError, ScenarioDocument,
    parse_scenario_document,
};

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

/// Read, parse, negotiate the schema version, validate, and compile a JSON5
/// scenario file.
pub fn load_scenario(path: &Path) -> Result<CompiledScenario, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
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
    match document {
        ScenarioDocument::V1(source) => {
            CompiledScenario::compile(source).map_err(|diagnostics| LoadError::Invalid {
                path: path.to_path_buf(),
                diagnostics,
            })
        }
        ScenarioDocument::V2(source) => {
            CompiledScenario::compile_v2(source).map_err(|diagnostics| LoadError::Invalid {
                path: path.to_path_buf(),
                diagnostics,
            })
        }
    }
}

fn render_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
