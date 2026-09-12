//! Load a JSON5 scenario file into a compiled scenario.
//!
//! The kernel crates are filesystem-free by contract, so file access belongs to
//! the presentation layer and its applications. Every renderer mirrors the
//! headless CLI's load path exactly so all of them drive the same validated
//! [`CompiledScenario`].

use std::path::{Path, PathBuf};

use tangle_model::{CompiledScenario, Diagnostic, ParseError, parse_scenario_source};

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
pub fn load_scenario(path: &Path) -> Result<CompiledScenario, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let source = parse_scenario_source(&text).map_err(|source| LoadError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    CompiledScenario::compile(source).map_err(|diagnostics| LoadError::Invalid {
        path: path.to_path_buf(),
        diagnostics,
    })
}

fn render_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
