//! Contract tests for the `validate` command.
//!
//! Each invalid fixture names the stage that must reject it: JSON5 syntax, the
//! source schema, or the semantic validation the loader runs. Every rejection
//! exits non-zero and prints an actionable diagnostic, an accepted source exits
//! zero with one report line, and no invocation writes a run artifact.

use std::path::PathBuf;
use std::process::{Command, Output};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

/// A minimally valid source: one guide path with a portal at each end.
const VALID_SOURCE: &str = r#"
{
  schema_version: 1,
  id: 'validate_valid_v1',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
  ],
  portals: [
    { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
}
"#;

/// Well-formed JSON5 that is truncated mid-document.
const MALFORMED_SOURCE: &str = r#"{
  schema_version: 1,
  id: 'validate_malformed_v1',
"#;

/// Well-formed JSON5 that violates the source schema: `speed_limit_mps` is not
/// a field of the version 1 document.
const SCHEMA_INVALID_SOURCE: &str = r#"
{
  schema_version: 1,
  id: 'validate_schema_invalid_v1',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
  ],
  portals: [
    { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
  ],
  speed_limit_mps: 50.0,
}
"#;

/// Structurally valid JSON5 the semantic validator rejects: a duplicated portal
/// identifier and a portal attached to an undeclared path.
const LOADER_INVALID_SOURCE: &str = r#"
{
  schema_version: 1,
  id: 'validate_loader_invalid_v1',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
  ],
  portals: [
    { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'west_entry', path: 'nowhere', end: 'end', width_m: 3.5 },
  ],
}
"#;

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tangle-cli-validate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    /// Write a source into the scratch directory and return its name, which is
    /// also its path relative to the command's working directory.
    fn write_source(&self, name: &str, contents: &str) -> String {
        std::fs::write(self.dir.join(name), contents).expect("fixture is written");
        name.to_owned()
    }

    /// Sorted entries the scratch directory holds.
    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .expect("scratch directory is readable")
            .map(|entry| {
                entry
                    .expect("directory entry is readable")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
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

/// Run `validate` from inside the scratch directory.
fn validate(scratch: &Scratch, scenario: &str) -> Output {
    Command::new(CLI)
        .arg("validate")
        .arg(scenario)
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The diagnostics the report places on their own lines.
fn diagnostic_lines(output: &Output) -> Vec<String> {
    stderr(output)
        .lines()
        .filter(|line| line.starts_with("  ["))
        .map(str::to_owned)
        .collect()
}

#[test]
fn valid_source_exits_zero_and_reports_the_scenario() {
    let scratch = Scratch::new("valid");
    let source = scratch.write_source("valid.json5", VALID_SOURCE);

    let output = validate(&scratch, &source);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "valid.json5: valid scenario 'validate_valid_v1' (schema version 1)\n"
    );
    assert_eq!(stderr(&output), "");
}

#[test]
fn checked_in_walking_scenario_validates_by_absolute_path() {
    let scratch = Scratch::new("walking");
    let scenario = repo_path("scenarios/walking/walking_guide_v1.json5");

    let output = Command::new(CLI)
        .arg("validate")
        .arg(&scenario)
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("valid scenario 'walking_guide_v1'"),
        "unexpected report: {}",
        stdout(&output)
    );
}

#[test]
fn malformed_source_exits_non_zero_with_a_positioned_message() {
    let scratch = Scratch::new("malformed");
    let source = scratch.write_source("malformed.json5", MALFORMED_SOURCE);

    let output = validate(&scratch, &source);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("is not valid JSON5"),
        "unexpected message: {message}"
    );
    assert!(message.contains("malformed.json5"), "message: {message}");
    // The parser reports where the document stopped being parseable: the
    // closed document ends on line 4.
    assert!(message.contains("4:1"), "message: {message}");
}

#[test]
fn schema_invalid_source_exits_non_zero_naming_the_offending_field() {
    let scratch = Scratch::new("schema-invalid");
    let source = scratch.write_source("schema_invalid.json5", SCHEMA_INVALID_SOURCE);

    let output = validate(&scratch, &source);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("is not valid JSON5"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("speed_limit_mps"),
        "the message must name the rejected field: {message}"
    );
}

#[test]
fn loader_invalid_source_exits_non_zero_with_every_diagnostic() {
    let scratch = Scratch::new("loader-invalid");
    let source = scratch.write_source("loader_invalid.json5", LOADER_INVALID_SOURCE);

    let output = validate(&scratch, &source);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "", "a rejected source has no report line");
    let message = stderr(&output);
    assert!(
        message.contains("failed validation"),
        "unexpected message: {message}"
    );

    let diagnostics = diagnostic_lines(&output);
    assert_eq!(
        diagnostics.len(),
        2,
        "each diagnostic belongs on its own line: {message}"
    );
    assert!(
        diagnostics[0].contains("E_ID_DUPLICATE") && diagnostics[0].contains("west_entry"),
        "unexpected diagnostic: {}",
        diagnostics[0]
    );
    assert!(
        diagnostics[1].contains("E_PORTAL_UNKNOWN_PATH") && diagnostics[1].contains("'nowhere'"),
        "unexpected diagnostic: {}",
        diagnostics[1]
    );
}

#[test]
fn validate_writes_no_run_artifact() {
    let scratch = Scratch::new("no-artifact");
    let valid = scratch.write_source("valid.json5", VALID_SOURCE);
    let invalid = scratch.write_source("invalid.json5", LOADER_INVALID_SOURCE);

    let accepted = validate(&scratch, &valid);
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&accepted)
    );
    let rejected = validate(&scratch, &invalid);
    assert_eq!(rejected.status.code(), Some(1));

    // `run` writes a trace (and optionally a hash file) into the working
    // directory; `validate` must leave it holding exactly the two sources.
    assert_eq!(
        scratch.entries(),
        vec!["invalid.json5".to_owned(), "valid.json5".to_owned()],
        "validate wrote a run artifact"
    );
    for output in [&accepted, &rejected] {
        let report = format!("{}{}", stdout(output), stderr(output));
        assert!(
            !report.contains("trace hash"),
            "validate reported a run: {report}"
        );
    }
}

#[test]
fn help_documents_usage_inputs_and_exit_codes() {
    let output = Command::new(CLI)
        .args(["validate", "--help"])
        .output()
        .expect("tangle-cli runs");

    assert_eq!(output.status.code(), Some(0));
    let help = stdout(&output);
    for expected in [
        "Usage:",
        "tangle-cli validate <SCENARIO>",
        "<SCENARIO>",
        "Exit codes:",
        "the loader accepts the source",
        "command-line usage error",
    ] {
        assert!(
            help.contains(expected),
            "help text is missing '{expected}':\n{help}"
        );
    }
}
