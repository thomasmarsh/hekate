//! Offline scenario validation.
//!
//! `validate` answers "would the loader accept this source?" without starting a
//! run. It shares [`crate::load_scenario`] with `run` and `baseline`, so a
//! source it accepts is exactly a source `run` can start: one read, one JSON5
//! parse against the source schema, one semantic validation, and one
//! compilation. Nothing here advances a tick or writes a run artifact.

use std::path::{Path, PathBuf};

use crate::{LoadError, load_scenario};

/// What the loader checked in a scenario source it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationSummary {
    /// Path of the checked source, exactly as the command received it.
    pub source_path: PathBuf,
    /// Authored scenario identifier.
    pub scenario_id: String,
    /// Schema version the document was authored against.
    pub schema_version: u32,
}

/// Read, parse, validate, and compile a scenario source without running it.
///
/// Semantic validation and compilation are the same mandatory steps the kernel
/// requires, so an `Ok` here is the strongest statement the loader can make
/// short of executing the scenario.
pub fn validate_scenario(path: &Path) -> Result<ValidationSummary, LoadError> {
    let scenario = load_scenario(path)?;
    Ok(ValidationSummary {
        source_path: path.to_path_buf(),
        scenario_id: scenario.id().to_owned(),
        schema_version: scenario.schema_version(),
    })
}

/// Render a loader failure as an actionable report, one diagnostic per line.
///
/// [`LoadError`]'s `Display` joins diagnostics with `; ` for the single-line
/// console error `run` prints. A check command has room for every finding, so
/// each diagnostic gets its own indented line instead of being folded into one.
pub fn render_validation_failure(error: &LoadError) -> String {
    match error {
        LoadError::Invalid { path, diagnostics } => {
            let mut report = format!("scenario '{}' failed validation:", path.display());
            for diagnostic in diagnostics {
                report.push_str("\n  ");
                report.push_str(&diagnostic.to_string());
            }
            report
        }
        other => other.to_string(),
    }
}
