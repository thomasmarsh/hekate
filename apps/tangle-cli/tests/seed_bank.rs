//! Contract tests for the common-random-number seed bank and its batch wiring.
//!
//! A seed bank is the ordered seed list two variants of one scenario share, so
//! position *i* of each side runs at the same seed and a paired comparison
//! cancels the between-seed variance both sides carry. These tests pin the
//! properties that make it usable as an experiment input:
//!
//! - the `seed-bank` command is byte-reproducible across invocations and its
//!   output round-trips through [`read_seed_bank`];
//! - a malformed, duplicated, unsorted, empty, or future-versioned bank is
//!   refused, and a batch that names such a bank runs nothing;
//! - `batch --seed-bank` runs exactly the bank's seeds in the bank's order,
//!   records the bank's path and content hash in `batch.json`, and agrees with
//!   the equivalent `--seeds` batch on every per-run trace hash;
//! - two batches over one bank pair up seed for seed, and an explicit `--seeds`
//!   batch is unchanged.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};
use tangle_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, MANIFEST_FILE, RunManifest, SEED_BANK_VERSION, SeedBank,
    SeedBankError, SeedBankReference, read_seed_bank,
};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
/// Few enough ticks that a multi-seed batch stays fast.
const TICKS: u64 = 60;

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "tangle-cli-seed-bank-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Run the CLI in the scratch directory with the given arguments.
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(CLI)
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("tangle-cli runs")
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

fn stdout(output: &Output) -> Vec<u8> {
    output.stdout.clone()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn file_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()))
}

fn file_sha256(path: &Path) -> String {
    format!("{:x}", Sha256::digest(file_bytes(path)))
}

fn read_batch_manifest(root: &Path) -> BatchManifest {
    let json = std::fs::read_to_string(root.join(BATCH_MANIFEST_FILE)).expect("batch.json is read");
    serde_json::from_str(&json).expect("batch.json is JSON")
}

fn read_run_manifest(directory: &Path) -> RunManifest {
    let json = std::fs::read_to_string(directory.join(MANIFEST_FILE)).expect("manifest is written");
    serde_json::from_str(&json).expect("manifest is JSON")
}

/// Run `batch` for the walking scenario with the given seed source.
fn batch_command(scratch: &Scratch, out_root: &str, seed_source: &[&str]) -> Output {
    let ticks = TICKS.to_string();
    let scenario = repo_path(WALKING);
    let scenario = scenario.to_str().expect("the scenario path is UTF-8");
    let mut args = vec![
        "batch",
        scenario,
        seed_source[0],
        seed_source[1],
        "--ticks",
        &ticks,
        "--out-root",
        out_root,
    ];
    args.push("--jobs");
    args.push("1");
    scratch.cli(&args)
}

/// The ordered seeds a batch manifest records for its runs.
fn run_seeds(manifest: &BatchManifest) -> Vec<u64> {
    manifest.runs.iter().map(|run| run.seed).collect()
}

/// Write `text` as a bank file and return its path.
fn write_bank(scratch: &Scratch, name: &str, text: &str) -> PathBuf {
    let path = scratch.path(name);
    std::fs::write(&path, text).expect("the bank file is written");
    path
}

/// The `seed-bank` command writes identical bytes every time it runs, and the
/// bytes it writes are the canonical ascending bank of the requested seeds.
#[test]
fn seed_bank_output_is_byte_reproducible_across_invocations() {
    let scratch = Scratch::new("reproducible");

    let first = scratch.cli(&["seed-bank", "--seeds", "2,0,1"]);
    let second = scratch.cli(&["seed-bank", "--seeds", "2,0,1"]);
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));
    assert_eq!(
        stdout(&first),
        stdout(&second),
        "the same request produced different bank bytes"
    );

    let bank: SeedBank = serde_json::from_slice(&stdout(&first)).expect("the bank is JSON");
    assert_eq!(bank.seed_bank_version, SEED_BANK_VERSION);
    assert_eq!(bank.seeds, vec![0, 1, 2], "the bank is written ascending");

    // An arithmetic request is equally reproducible and ascending.
    let first = scratch.cli(&[
        "seed-bank",
        "--start",
        "10",
        "--count",
        "4",
        "--stride",
        "5",
    ]);
    let second = scratch.cli(&[
        "seed-bank",
        "--start",
        "10",
        "--count",
        "4",
        "--stride",
        "5",
    ]);
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));
    assert_eq!(stdout(&first), stdout(&second));
    let bank: SeedBank = serde_json::from_slice(&stdout(&first)).expect("the bank is JSON");
    assert_eq!(bank.seeds, vec![10, 15, 20, 25]);
}

/// A bank written to a file and a bank written to stdout are the same bytes, and
/// the file round-trips through the reader that batch uses.
#[test]
fn a_seed_bank_round_trips_through_a_file() {
    let scratch = Scratch::new("round-trip");

    let written = scratch.cli(&[
        "seed-bank",
        "--start",
        "5",
        "--count",
        "3",
        "--output",
        "bank.json",
    ]);
    assert_eq!(
        written.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&written)
    );

    let path = scratch.path("bank.json");
    let loaded = read_seed_bank(&path).expect("the bank round-trips");
    assert_eq!(
        loaded.bank,
        SeedBank::from_range(5, 3, 1).expect("the bank builds")
    );
    assert_eq!(loaded.bank.seeds, vec![5, 6, 7]);
    assert_eq!(
        loaded.content_sha256,
        file_sha256(&path),
        "the recorded hash must cover the bank file's exact bytes"
    );

    let to_stdout = scratch.cli(&["seed-bank", "--start", "5", "--count", "3"]);
    assert_eq!(
        file_bytes(&path),
        stdout(&to_stdout),
        "writing to a path and to stdout must produce the same bytes"
    );
    assert_eq!(
        loaded.content_sha256,
        format!("{:x}", Sha256::digest(&to_stdout.stdout))
    );
}

/// A bank that is not seed-bank JSON, declares another version, holds no seeds,
/// repeats a seed, or is not ascending is refused rather than interpreted.
#[test]
fn a_malformed_duplicated_or_unsorted_bank_is_refused() {
    let scratch = Scratch::new("invalid");

    let unsorted = write_bank(
        &scratch,
        "descending.json",
        "{\"seed_bank_version\": 1, \"seeds\": [2, 1]}",
    );
    assert!(matches!(
        read_seed_bank(&unsorted),
        Err(SeedBankError::NotAscending {
            previous: 2,
            seed: 1,
            ..
        })
    ));

    let duplicated = write_bank(
        &scratch,
        "duplicate.json",
        "{\"seed_bank_version\": 1, \"seeds\": [0, 0]}",
    );
    assert!(matches!(
        read_seed_bank(&duplicated),
        Err(SeedBankError::Duplicate { seed: 0, .. })
    ));

    let empty = write_bank(
        &scratch,
        "empty.json",
        "{\"seed_bank_version\": 1, \"seeds\": []}",
    );
    assert!(matches!(
        read_seed_bank(&empty),
        Err(SeedBankError::Empty { .. })
    ));

    let future = write_bank(
        &scratch,
        "future.json",
        "{\"seed_bank_version\": 2, \"seeds\": [0]}",
    );
    assert!(matches!(
        read_seed_bank(&future),
        Err(SeedBankError::UnsupportedVersion { version: 2, .. })
    ));

    let not_json = write_bank(&scratch, "garbage.json", "not a seed bank");
    assert!(matches!(
        read_seed_bank(&not_json),
        Err(SeedBankError::Parse { .. })
    ));

    let missing = scratch.path("missing.json");
    assert!(matches!(
        read_seed_bank(&missing),
        Err(SeedBankError::Io { .. })
    ));
}

/// `batch --seed-bank` runs exactly the bank's seeds in the bank's order and
/// records the bank's path and content hash, and it agrees with an explicit
/// `--seeds` batch over the same seeds on every per-run trace hash.
#[test]
fn batch_runs_a_seed_bank_and_records_its_identity() {
    let scratch = Scratch::new("batch-bank");

    let written = scratch.cli(&[
        "seed-bank",
        "--start",
        "5",
        "--count",
        "2",
        "--stride",
        "3",
        "--output",
        "bank.json",
    ]);
    assert_eq!(
        written.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&written)
    );

    let output = batch_command(&scratch, "from-bank", &["--seed-bank", "bank.json"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let root = scratch.path("from-bank");
    let manifest = read_batch_manifest(&root);
    assert_eq!(manifest.seeds, vec![5, 8], "the bank's seeds, in order");
    assert_eq!(run_seeds(&manifest), vec![5, 8]);
    assert_eq!(
        manifest.seed_bank,
        Some(SeedBankReference {
            path: "bank.json".to_owned(),
            content_sha256: file_sha256(&scratch.path("bank.json")),
        }),
        "the manifest must name the bank at the path the command was given"
    );
    for run in &manifest.runs {
        assert_eq!(run.directory, format!("seed-{}", run.seed));
        assert_eq!(read_run_manifest(&root.join(&run.directory)).seed, run.seed);
    }

    // The same seeds given explicitly produce the same runs, so the bank is
    // input and not a re-derivation of the seed list.
    let explicit = batch_command(&scratch, "explicit", &["--seeds", "5,8"]);
    assert_eq!(
        explicit.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&explicit)
    );
    let explicit_manifest = read_batch_manifest(&scratch.path("explicit"));
    assert_eq!(explicit_manifest.seeds, manifest.seeds);
    let explicit_hashes: Vec<&str> = explicit_manifest
        .runs
        .iter()
        .map(|run| run.trace_sha256.as_str())
        .collect();
    let bank_hashes: Vec<&str> = manifest
        .runs
        .iter()
        .map(|run| run.trace_sha256.as_str())
        .collect();
    assert_eq!(bank_hashes, explicit_hashes);
    assert!(explicit_manifest.seed_bank.is_none());
}

/// Two batches over one bank run the same seeds in the same order, so their runs
/// pair up position for position and produce identical per-run trace hashes.
#[test]
fn two_batches_from_one_bank_share_the_same_seeds_in_the_same_order() {
    let scratch = Scratch::new("paired");

    let written = scratch.cli(&[
        "seed-bank",
        "--start",
        "0",
        "--count",
        "3",
        "--output",
        "bank.json",
    ]);
    assert_eq!(
        written.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&written)
    );

    let first = batch_command(&scratch, "variant-a", &["--seed-bank", "bank.json"]);
    let second = batch_command(&scratch, "variant-b", &["--seed-bank", "bank.json"]);
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));
    assert_eq!(second.status.code(), Some(0), "stderr: {}", stderr(&second));

    let a = read_batch_manifest(&scratch.path("variant-a"));
    let b = read_batch_manifest(&scratch.path("variant-b"));
    assert_eq!(a.seed_bank, b.seed_bank, "both batches must name one bank");
    assert_eq!(
        a.seed_bank
            .as_ref()
            .map(|bank| bank.content_sha256.as_str()),
        Some(file_sha256(&scratch.path("bank.json")).as_str())
    );
    assert_eq!(a.seeds, b.seeds);
    assert_eq!(run_seeds(&a), run_seeds(&b));
    for (left, right) in a.runs.iter().zip(&b.runs) {
        assert_eq!(left.seed, right.seed, "position pairing must line up");
        assert_eq!(
            left.trace_sha256, right.trace_sha256,
            "seed {} did not produce the same run on both sides",
            left.seed
        );
    }
}

/// An explicit `--seeds` batch is unchanged: ascending runs, no bank field, and
/// the same bytes the manifest had before the field existed.
#[test]
fn batch_still_runs_an_explicit_seed_list() {
    let scratch = Scratch::new("explicit-seeds");

    let output = batch_command(&scratch, "batch", &["--seeds", "2,0,1"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let root = scratch.path("batch");
    let manifest = read_batch_manifest(&root);
    assert_eq!(manifest.seeds, vec![0, 1, 2]);
    assert_eq!(run_seeds(&manifest), vec![0, 1, 2]);
    assert!(manifest.seed_bank.is_none());

    let json = std::fs::read_to_string(root.join(BATCH_MANIFEST_FILE)).expect("batch.json");
    assert!(
        !json.contains("seed_bank"),
        "an explicit-seed batch must not grow a seed_bank field: {json}"
    );
}

/// A batch names exactly one seed source: neither or both is a usage error that
/// writes nothing.
#[test]
fn a_batch_needs_exactly_one_seed_source() {
    let scratch = Scratch::new("one-source");
    write_bank(
        &scratch,
        "bank.json",
        "{\"seed_bank_version\": 1, \"seeds\": [0]}",
    );

    let scenario = repo_path(WALKING);
    let scenario = scenario.to_str().expect("the scenario path is UTF-8");
    let mut args = vec!["batch", scenario, "--out-root", "none"];
    let missing = scratch.cli(&args);
    args.extend(["--seeds", "0", "--seed-bank", "bank.json"]);
    let both = scratch.cli(&args);

    assert_eq!(
        missing.status.code(),
        Some(2),
        "stderr: {}",
        stderr(&missing)
    );
    assert_eq!(both.status.code(), Some(2), "stderr: {}", stderr(&both));
    assert!(
        !scratch.path("none").exists(),
        "a usage error wrote a batch root"
    );
}

/// A bank that cannot be read or is invalid stops the batch before any run
/// starts and before the output root exists.
#[test]
fn an_unreadable_or_invalid_bank_is_refused_and_runs_nothing() {
    let scratch = Scratch::new("bad-bank");
    write_bank(
        &scratch,
        "descending.json",
        "{\"seed_bank_version\": 1, \"seeds\": [1, 0]}",
    );

    for (root, bank) in [
        ("missing-root", "missing.json"),
        ("invalid-root", "descending.json"),
    ] {
        let output = batch_command(&scratch, root, &["--seed-bank", bank]);
        assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
        assert!(
            !scratch.path(root).exists(),
            "'{root}' was written from a bad bank"
        );
    }
}

/// The seed-bank request itself is validated: an unusable request is a usage
/// error, and a request that cannot be a bank is refused.
#[test]
fn an_unusable_seed_bank_request_is_refused() {
    let scratch = Scratch::new("request");

    for args in [
        vec!["seed-bank", "--start", "0", "--count", "0"],
        vec!["seed-bank", "--start", "0", "--count", "1", "--stride", "0"],
        vec!["seed-bank", "--seeds", "0", "--start", "0", "--count", "1"],
        vec!["seed-bank", "--start", "0"],
        vec!["seed-bank"],
    ] {
        let output = scratch.cli(&args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?} must be a usage error: {}",
            stderr(&output)
        );
        assert!(output.stdout.is_empty(), "{args:?} wrote a bank");
        assert!(!scratch.path("bank.json").exists(), "{args:?} wrote a bank");
    }

    let repeated = scratch.cli(&["seed-bank", "--seeds", "1,1"]);
    assert_eq!(
        repeated.status.code(),
        Some(1),
        "stderr: {}",
        stderr(&repeated)
    );
    assert!(repeated.stdout.is_empty(), "a refused request wrote a bank");
}
