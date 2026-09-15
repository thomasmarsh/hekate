//! Contract tests for the `migrate` command.
//!
//! Each test runs the built binary on a checked-in version-1 fixture and pins
//! the bytes it writes: the demand and population fixtures must reproduce the
//! golden normalized version 2 exactly, `--output` must receive the same bytes
//! stdout does, and a source that is not version 1 must be reported rather than
//! re-emitted.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

/// The checked-in migration fixtures, shared with `hekate-model`'s tests.
const FIXTURE_DIR: &str = "crates/hekate-model/tests/fixtures/migration";

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn fixture(name: &str) -> PathBuf {
    repo_path(&format!("{FIXTURE_DIR}/{name}"))
}

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("hekate-cli-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, contents).expect("scratch fixture is written");
        path
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

fn migrate(scenario: &Path) -> Output {
    Command::new(CLI)
        .arg("migrate")
        .arg(scenario)
        .output()
        .expect("hekate-cli runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn demand_fixture_migrates_to_the_golden_version_2_bytes() {
    let output = migrate(&fixture("demand_v1.json5"));

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        output.stdout,
        std::fs::read(fixture("demand_v2.json")).expect("the golden version 2 is checked in"),
        "migrate must write the checked-in golden bytes exactly"
    );
}

#[test]
fn population_fixture_migrates_to_the_golden_version_2_bytes() {
    let output = migrate(&fixture("population_v1.json5"));

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        output.stdout,
        std::fs::read(fixture("population_v2.json")).expect("the golden version 2 is checked in")
    );
}

#[test]
fn output_writes_the_same_bytes_as_stdout_and_no_run_artifact() {
    let scratch = Scratch::new("output");
    let destination = scratch.dir.join("normalized.json");

    let output = Command::new(CLI)
        .arg("migrate")
        .arg(fixture("demand_v1.json5"))
        .arg("--output")
        .arg(&destination)
        .output()
        .expect("hekate-cli runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(output.stdout, Vec::<u8>::new(), "no document on stdout");
    assert_eq!(
        std::fs::read(&destination).expect("the destination is written"),
        std::fs::read(fixture("demand_v2.json")).expect("the golden version 2 is checked in")
    );
    assert_eq!(
        scratch.entries(),
        vec!["normalized.json".to_owned()],
        "migrate writes no artifact other than the destination"
    );
    assert!(
        !stderr(&output).contains("trace hash"),
        "migrate must not run the scenario: {}",
        stderr(&output)
    );
}

#[test]
fn a_version_2_source_is_rejected_rather_than_re_emitted() {
    let output = migrate(&fixture("population_v2.json"));

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, Vec::<u8>::new(), "nothing is emitted");
    let message = stderr(&output);
    assert!(
        message.contains("already declares schema_version 2"),
        "the diagnostic must say no migration applies: {message}"
    );
}

#[test]
fn an_unknown_schema_version_is_rejected() {
    let scratch = Scratch::new("unknown-version");
    let source = scratch.write(
        "future.json5",
        "{ schema_version: 3, id: 'future', coordinate_system: { x: 'east_m', y: 'north_m' }, \
         paths: [], portals: [] }",
    );

    let output = migrate(&source);

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("schema_version 3 is not supported"),
        "the diagnostic must name the unsupported version: {message}"
    );
}

#[test]
fn a_malformed_source_is_rejected() {
    let scratch = Scratch::new("malformed");
    let source = scratch.write("malformed.json5", "{ schema_version: 1, id: 'truncated',");

    let output = migrate(&source);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("is not a version-1 scenario source"),
        "unexpected message: {}",
        stderr(&output)
    );
}

#[test]
fn help_documents_usage_inputs_output_and_exit_codes() {
    let output = Command::new(CLI)
        .args(["migrate", "--help"])
        .output()
        .expect("hekate-cli runs");

    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "Usage:",
        "hekate-cli migrate <SCENARIO> [--output <FILE>]",
        "schema_version 1",
        "trailing newline",
        "Exit codes:",
        "the document was migrated and written",
        "command-line usage error",
    ] {
        assert!(
            help.contains(expected),
            "help text is missing '{expected}':\n{help}"
        );
    }
}
