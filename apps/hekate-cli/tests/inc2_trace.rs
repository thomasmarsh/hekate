//! Golden traces for every checked-in Increment 2 fixture, at both required
//! presets.
//!
//! `PHASE_2_PLAN.md`'s Increment 2 gate is that each fixture reproduces its
//! decisions, claims, events, and trace hash, and that the golden traces cover
//! every maneuver transition and the wrong-way interval lifecycle. This suite
//! pins that with one checked-in golden per fixture per required preset: the
//! canonical trace bytes plus their SHA-256, recorded from the fixture's
//! *declared run* — its pinned bank seed (see `inc2_support`) and, at Fine, the
//! tick count `converge::fidelity_ticks` gives the Standard run's horizon.
//!
//! ## Which fixtures `hekate-cli run` can record, and which it cannot
//!
//! Three fixtures (`narrow_passing_v2`, `motor_passing_narrow_v2`, and
//! `narrow_passing_unsafe_v2`) decide their maneuver with the kernel's own
//! overtaking tactic, so `hekate-cli run` writes exactly the golden bytes; this
//! suite proves that end to end for their Standard goldens, including the run
//! directory manifest those bytes were recorded from and `replay --verify`.
//!
//! The other three fixtures have **no production caller** for the request their
//! maneuver needs: the two change-of-lane fixtures need
//! `Simulation::request_lateral_maneuver` and the wrong-way fixture needs
//! `Simulation::request_wrong_way_entry`, and neither seam is exposed by a CLI
//! command. Their goldens, and every fixture's Fine golden (`hekate-cli run` has
//! no `--step` flag, so it can only run the Standard 50 ms step), are therefore
//! recorded in-process through `TraceRecorder`, which the trace contract
//! documents as producing the canonical bytes and hash of the direct run. A
//! golden for those fixtures covers the maneuver the request produces; it is
//! **not** CLI-run coverage and this suite never claims it is.
//!
//! ## What these goldens do not cover: simultaneous claims
//!
//! The Increment 2 gate also names stable simultaneous-claim ordering. No
//! checked-in fixture produces two simultaneous claims at its pinned bank seed —
//! no golden here records two agents' maneuver transitions on one tick, which
//! the per-fixture+preset `*_golden_records_no_simultaneous_maneuvers_at_*`
//! tests enforce — so these goldens cannot cover the batch arbitration's
//! tie-break. That ordering is proved by TAS-127's unit and fixed-seed
//! integration suites (`crates/hekate-sim/tests/claims.rs`), which is the
//! ordering evidence this node reports; no scenario was authored to manufacture
//! a second simultaneous claim.
//!
//! ## Regenerating the goldens
//!
//! `scripts/regen-goldens.sh` runs this suite with `UPDATE_GOLDENS=1`, which
//! rewrites `tests/golden/inc2/*` from the in-process driver. The three Standard
//! goldens of the CLI-recordable fixtures are written byte for byte by:
//!
//! ```sh
//! cargo run -p hekate-cli -- run scenarios/phase2/inc2/narrow_passing_v2.json5 \
//!   --seed 102 --ticks 1900 \
//!   --output tests/golden/inc2/narrow_passing_v2.standard.trace.jsonl \
//!   --hash-file tests/golden/inc2/narrow_passing_v2.standard.trace.sha256
//! cargo run -p hekate-cli -- run scenarios/phase2/inc2/motor_passing_narrow_v2.json5 \
//!   --seed 102 --ticks 1900 \
//!   --output tests/golden/inc2/motor_passing_narrow_v2.standard.trace.jsonl \
//!   --hash-file tests/golden/inc2/motor_passing_narrow_v2.standard.trace.sha256
//! cargo run -p hekate-cli -- run scenarios/phase2/inc2/narrow_passing_unsafe_v2.json5 \
//!   --seed 102 --ticks 1900 \
//!   --output tests/golden/inc2/narrow_passing_unsafe_v2.standard.trace.jsonl \
//!   --hash-file tests/golden/inc2/narrow_passing_unsafe_v2.standard.trace.sha256
//! ```
//!
//! A failure here means canonical serialization or kernel behavior changed.
//! Inspect the diff before regenerating: an unexpected change is a regression,
//! not a stale fixture.

mod inc2_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

use hekate_cli::{MANIFEST_FILE, RunManifest};
use inc2_support::{
    FIXTURES, Fixture, PRESETS, Plan, STANDARD_STEP_S, assert_maneuver_occurs, events_of_kind,
    repo_path, run, ticks_at,
};
use sha2::{Digest, Sha256};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

/// The directory every Increment 2 golden lives in.
const GOLDEN_DIR: &str = "tests/golden/inc2";

/// The repository-relative path of one fixture's golden file.
fn golden_path(fixture: &Fixture, preset: &str, extension: &str) -> String {
    format!("{GOLDEN_DIR}/{}.{preset}.trace.{extension}", fixture.id)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Read a checked-in golden file, or write it when `UPDATE_GOLDENS` is set.
fn check_text(relative: &str, actual: &str) {
    let path = repo_path(relative);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(path.parent().expect("a golden has a parent"))
            .expect("create the golden directory");
        std::fs::write(&path, actual).expect("write the golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read golden '{}': {error}\nrun scripts/regen-goldens.sh",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "golden '{}' changed; see this suite's module docs before regenerating",
        path.display()
    );
}

/// The checked-in golden bytes of one fixture at one preset.
fn golden_text(fixture: &Fixture, preset: &str, extension: &str) -> String {
    let relative = golden_path(fixture, preset, extension);
    std::fs::read_to_string(repo_path(&relative))
        .unwrap_or_else(|error| panic!("cannot read golden '{relative}': {error}"))
}

/// The fixture's declared run at one preset: its pinned bank seed, its horizon,
/// and the trace its driver records.
fn declared_run(fixture: &Fixture, preset: &str, step_s: f64) -> inc2_support::Run {
    let run = run(fixture, fixture.pinned_seed, step_s);
    assert_maneuver_occurs(fixture, preset, &run);
    run
}

/// The `run` header of a canonical trace: the parameters the golden is bound to.
fn header(bytes: &[u8]) -> serde_json::Value {
    let first = std::str::from_utf8(bytes)
        .expect("a canonical trace is UTF-8")
        .lines()
        .next()
        .expect("a canonical trace has a header")
        .to_owned();
    serde_json::from_str(&first).expect("the header is JSON")
}

/// One fixture+preset golden case: the fixture's declared run reproduces its
/// checked-in golden bytes and hash, and the golden's own header names the
/// scenario, seed, step, and tick count it was recorded at.
fn assert_matches_golden(fixture: &Fixture, preset: &str, step_s: f64) {
    let run = declared_run(fixture, preset, step_s);
    let bytes = std::str::from_utf8(run.trace.bytes()).expect("a trace is UTF-8");

    check_text(&golden_path(fixture, preset, "jsonl"), bytes);
    check_text(
        &golden_path(fixture, preset, "sha256"),
        &format!("{}\n", run.trace.hash()),
    );
    assert_eq!(
        run.trace.hash(),
        sha256_hex(run.trace.bytes()),
        "{} [{preset}]: the trace hash is the hash of its own bytes",
        fixture.id
    );
    assert_eq!(
        golden_text(fixture, preset, "sha256").trim(),
        run.trace.hash(),
        "{} [{preset}]: the golden hash is not the hash of the golden bytes",
        fixture.id
    );

    // The header binds the golden to one scenario, seed, step, and tick
    // count, so a golden cannot silently stand for another preset.
    let header = header(run.trace.bytes());
    let ticks = ticks_at(fixture.standard_ticks, step_s);
    assert_eq!(header["scenario_id"], fixture.id, "{}", fixture.id);
    assert_eq!(
        header["seed"].as_u64(),
        Some(fixture.pinned_seed),
        "{} [{preset}]",
        fixture.id
    );
    assert_eq!(header["step_s"].as_f64(), Some(step_s), "{}", fixture.id);
    assert_eq!(header["ticks"].as_u64(), Some(ticks), "{}", fixture.id);
    println!(
        "{} [{preset}] seed {} {} ticks {} {}",
        fixture.id,
        fixture.pinned_seed,
        ticks,
        run.trace.hash(),
        run.trace.bytes().len()
    );
}

/// Generate the per-fixture+preset golden tests, one `#[test]` per case, over
/// the `FIXTURES` and `PRESETS` indices `inc2_support` declares.
///
/// Every fixture reproduces its checked-in golden bytes and hash at both
/// required presets, and each golden's own header names the preset it was
/// recorded at; nextest runs every case in its own process.
macro_rules! golden_run_cases {
    ($(($name:ident, $fixture_index:literal, $preset_index:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                let (preset, step_s) = PRESETS[$preset_index];
                assert_matches_golden(&FIXTURES[$fixture_index], preset, step_s);
            }
        )+
    };
}

golden_run_cases!(
    (
        narrow_passing_v2_matches_its_golden_bytes_and_hash_at_standard,
        0,
        0
    ),
    (
        narrow_passing_v2_matches_its_golden_bytes_and_hash_at_fine,
        0,
        1
    ),
    (
        motor_passing_narrow_v2_matches_its_golden_bytes_and_hash_at_standard,
        1,
        0
    ),
    (
        motor_passing_narrow_v2_matches_its_golden_bytes_and_hash_at_fine,
        1,
        1
    ),
    (
        motor_lane_change_v2_matches_its_golden_bytes_and_hash_at_standard,
        2,
        0
    ),
    (
        motor_lane_change_v2_matches_its_golden_bytes_and_hash_at_fine,
        2,
        1
    ),
    (
        narrow_passing_unsafe_v2_matches_its_golden_bytes_and_hash_at_standard,
        3,
        0
    ),
    (
        narrow_passing_unsafe_v2_matches_its_golden_bytes_and_hash_at_fine,
        3,
        1
    ),
    (
        motor_lane_change_boundary_v2_matches_its_golden_bytes_and_hash_at_standard,
        4,
        0
    ),
    (
        motor_lane_change_boundary_v2_matches_its_golden_bytes_and_hash_at_fine,
        4,
        1
    ),
    (
        narrow_wrong_way_v2_matches_its_golden_bytes_and_hash_at_standard,
        5,
        0
    ),
    (
        narrow_wrong_way_v2_matches_its_golden_bytes_and_hash_at_fine,
        5,
        1
    ),
);

/// One `maneuver` record of a golden, as `(from, to, edge, reason)`.
fn maneuver_record(record: &serde_json::Value) -> (String, String, String, String) {
    let field = |name: &str| {
        record
            .get(name)
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| panic!("a maneuver record carries '{name}': {record}"))
            .to_owned()
    };
    (field("from"), field("to"), field("edge"), field("reason"))
}

/// One fixture+preset golden-coverage case: the golden bytes cover the maneuver
/// transitions or the wrong-way interval lifecycle that fixture exists to prove.
fn assert_golden_covers_its_maneuvers(fixture: &Fixture, preset: &str) {
    let bytes = golden_text(fixture, preset, "jsonl").into_bytes();
    let maneuvers: Vec<(String, String, String, String)> = events_of_kind(&bytes, "maneuver")
        .iter()
        .map(maneuver_record)
        .collect();
    let transitions = events_of_kind(&bytes, "facility_transition");
    let opposing = events_of_kind(&bytes, "opposing_traversal");

    match fixture.plan {
        Plan::Autonomous => {
            for edge in [
                ("following", "preparing", "attempted"),
                ("preparing", "committed", "committed"),
                ("committed", "returning", "completed"),
                ("returning", "following", "completed"),
            ] {
                assert!(
                    maneuvers.iter().any(|(from, to, recorded, _)| (
                        from.as_str(),
                        to.as_str(),
                        recorded.as_str()
                    ) == edge),
                    "{} [{preset}]: the golden does not record the {edge:?} edge: {maneuvers:?}",
                    fixture.id
                );
            }
            assert!(
                !maneuvers.is_empty() && !events_of_kind(&bytes, "close_pass").is_empty(),
                "{} [{preset}]: the golden records the pass interval",
                fixture.id
            );
        }
        Plan::LaneChange { .. } => match fixture.expectation {
            inc2_support::Expectation::LaneChange { handoffs, .. } => {
                assert_eq!(
                    transitions.len(),
                    handoffs,
                    "{} [{preset}]: the golden records the crossings the fixture performs",
                    fixture.id
                );
                if handoffs == 0 {
                    // The prohibited-boundary variant's rejection is an
                    // inspectable route-state reason, not a stream
                    // record (`crates/hekate-sim/src/sim.rs`'s
                    // `Attempt::BoundaryForbidden` discards the intent
                    // and records the reason with no transition), so the
                    // golden proves the prevented crossing by the
                    // absence of any maneuver edge and any handoff.
                    assert!(
                        maneuvers.is_empty(),
                        "{} [{preset}]: a prevented crossing records no transition: {maneuvers:?}",
                        fixture.id
                    );
                    assert!(
                        events_of_kind(&bytes, "spawned").len() >= 2
                            && events_of_kind(&bytes, "despawned").len() >= 2,
                        "{} [{preset}]: both participants of the prevented crossing ran",
                        fixture.id
                    );
                } else {
                    for edge in [
                        ("following", "preparing", "attempted"),
                        ("preparing", "committed", "committed"),
                        ("committed", "returning", "completed"),
                        ("returning", "following", "completed"),
                    ] {
                        assert!(
                            maneuvers.iter().any(|(from, to, recorded, _)| (
                                from.as_str(),
                                to.as_str(),
                                recorded.as_str()
                            ) == edge),
                            "{} [{preset}]: the golden does not record the {edge:?} edge: {maneuvers:?}",
                            fixture.id
                        );
                    }
                    assert!(
                        transitions
                            .iter()
                            .all(|record| record["permitted"] == serde_json::json!(true)),
                        "{} [{preset}]: the configured change of lane crosses permitted boundaries",
                        fixture.id
                    );
                }
            }
            _ => unreachable!("a lane-change plan carries the lane-change expectation"),
        },
        Plan::WrongWay => {
            assert!(
                opposing
                    .iter()
                    .any(|record| record["entering"] == serde_json::json!(true)),
                "{} [{preset}]: the golden opens an opposing traversal",
                fixture.id
            );
            assert!(
                opposing
                    .iter()
                    .any(|record| record["entering"] == serde_json::json!(false)),
                "{} [{preset}]: the golden closes an opposing traversal",
                fixture.id
            );
            assert!(
                opposing
                    .iter()
                    .any(|record| record["violating"] == serde_json::json!(true)),
                "{} [{preset}]: the golden records the prohibited corridor's violation",
                fixture.id
            );
            assert!(
                opposing
                    .iter()
                    .any(|record| record["violating"] == serde_json::json!(false)),
                "{} [{preset}]: the golden records the permitted corridor's traversal",
                fixture.id
            );
            assert!(
                !events_of_kind(&bytes, "collision").is_empty(),
                "{} [{preset}]: the occupied corridor reaches its contact",
                fixture.id
            );
        }
    }

    assert!(
        !events_of_kind(&bytes, "spawned").is_empty(),
        "{} [{preset}]: the golden's run admits agents",
        fixture.id
    );
}

/// Generate the per-fixture+preset golden-artifact tests, one `#[test]` per
/// case, calling `$helper` with the fixture and the preset's name.
///
/// The golden bytes of every fixture and preset cover the maneuver transitions
/// or the wrong-way interval lifecycle that fixture exists to prove. The
/// transitions are read out of the recorded bytes rather than out of the live
/// run, so the coverage claim is about the checked-in artifact a reviewer can
/// read: the attempt, commit, and completion edges of the lifecycle; the
/// facility crossing a change of lane performs; the forbidden boundary the
/// prohibited variant records instead; and the open and close of an opposing
/// traversal.
macro_rules! golden_artifact_cases {
    ($helper:ident; $(($name:ident, $fixture_index:literal, $preset_index:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                $helper(&FIXTURES[$fixture_index], PRESETS[$preset_index].0);
            }
        )+
    };
}

golden_artifact_cases!(
    assert_golden_covers_its_maneuvers;
    (narrow_passing_v2_golden_covers_its_maneuver_transitions_at_standard, 0, 0),
    (narrow_passing_v2_golden_covers_its_maneuver_transitions_at_fine, 0, 1),
    (motor_passing_narrow_v2_golden_covers_its_maneuver_transitions_at_standard, 1, 0),
    (motor_passing_narrow_v2_golden_covers_its_maneuver_transitions_at_fine, 1, 1),
    (motor_lane_change_v2_golden_covers_its_maneuver_transitions_at_standard, 2, 0),
    (motor_lane_change_v2_golden_covers_its_maneuver_transitions_at_fine, 2, 1),
    (narrow_passing_unsafe_v2_golden_covers_its_maneuver_transitions_at_standard, 3, 0),
    (narrow_passing_unsafe_v2_golden_covers_its_maneuver_transitions_at_fine, 3, 1),
    (motor_lane_change_boundary_v2_golden_covers_its_maneuver_transitions_at_standard, 4, 0),
    (motor_lane_change_boundary_v2_golden_covers_its_maneuver_transitions_at_fine, 4, 1),
    (narrow_wrong_way_v2_golden_covers_its_maneuver_transitions_at_standard, 5, 0),
    (narrow_wrong_way_v2_golden_covers_its_maneuver_transitions_at_fine, 5, 1),
);

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

/// One fixture+preset no-simultaneous-claim case: no golden records two agents'
/// maneuver transitions on one tick.
///
/// The check guards a claim rather than proving the kernel: if a fixture's
/// arrivals ever put two maneuvers on one tick at a pinned seed, this fails and
/// points the reader at the tie-break these goldens do not cover. The ordering
/// evidence for that tie-break stays `crates/hekate-sim/tests/claims.rs`
/// (TAS-127).
fn assert_golden_has_no_simultaneous_maneuvers(fixture: &Fixture, preset: &str) {
    let bytes = golden_text(fixture, preset, "jsonl").into_bytes();
    let mut maneuvering: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
    for record in events_of_kind(&bytes, "maneuver") {
        let tick = record["tick"]
            .as_u64()
            .expect("a maneuver record has a tick");
        let agent = record["agent"]
            .as_u64()
            .expect("a maneuver record has an agent");
        maneuvering.entry(tick).or_default().insert(agent);
    }
    let simultaneous: Vec<(u64, Vec<u64>)> = maneuvering
        .iter()
        .filter(|(_, agents)| agents.len() > 1)
        .map(|(tick, agents)| (*tick, agents.iter().copied().collect()))
        .collect();
    assert!(
        simultaneous.is_empty(),
        "{} [{preset}]: the golden records simultaneous maneuvers, which needs the \
         simultaneous-claim ordering evidence this suite does not carry: {simultaneous:?}",
        fixture.id
    );
}

golden_artifact_cases!(
    assert_golden_has_no_simultaneous_maneuvers;
    (narrow_passing_v2_golden_records_no_simultaneous_maneuvers_at_standard, 0, 0),
    (narrow_passing_v2_golden_records_no_simultaneous_maneuvers_at_fine, 0, 1),
    (motor_passing_narrow_v2_golden_records_no_simultaneous_maneuvers_at_standard, 1, 0),
    (motor_passing_narrow_v2_golden_records_no_simultaneous_maneuvers_at_fine, 1, 1),
    (motor_lane_change_v2_golden_records_no_simultaneous_maneuvers_at_standard, 2, 0),
    (motor_lane_change_v2_golden_records_no_simultaneous_maneuvers_at_fine, 2, 1),
    (narrow_passing_unsafe_v2_golden_records_no_simultaneous_maneuvers_at_standard, 3, 0),
    (narrow_passing_unsafe_v2_golden_records_no_simultaneous_maneuvers_at_fine, 3, 1),
    (motor_lane_change_boundary_v2_golden_records_no_simultaneous_maneuvers_at_standard, 4, 0),
    (motor_lane_change_boundary_v2_golden_records_no_simultaneous_maneuvers_at_fine, 4, 1),
    (narrow_wrong_way_v2_golden_records_no_simultaneous_maneuvers_at_standard, 5, 0),
    (narrow_wrong_way_v2_golden_records_no_simultaneous_maneuvers_at_fine, 5, 1),
);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-cli-inc2-trace-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `hekate-cli run` writes exactly the checked-in Standard golden of a fixture
/// whose maneuver needs no request, its manifest records the parameters the
/// golden declares, and `replay --verify` reproduces the golden bytes.
///
/// The scenario is passed absolute so the manifest records an absolute source
/// path that `replay` resolves wherever the command runs.
#[test]
fn the_cli_run_and_replay_reproduce_the_standard_goldens_of_the_autonomous_fixtures() {
    for fixture in FIXTURES
        .iter()
        .filter(|fixture| fixture.plan == Plan::Autonomous)
    {
        let scratch = Scratch::new(fixture.id);
        let seed = fixture.pinned_seed.to_string();
        let ticks = fixture.standard_ticks.to_string();
        let output = Command::new(CLI)
            .arg("run")
            .arg(repo_path(fixture.path))
            .args(["--seed", &seed, "--ticks", &ticks])
            .args(["--output", "trace.jsonl", "--hash-file", "trace.sha256"])
            .args(["--run-dir", "run"])
            .current_dir(&scratch.dir)
            .output()
            .expect("hekate-cli runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: {}",
            fixture.id,
            String::from_utf8_lossy(&output.stderr)
        );

        let written = std::fs::read(scratch.path("trace.jsonl")).expect("the trace is written");
        assert_eq!(
            std::str::from_utf8(&written).expect("the trace is UTF-8"),
            golden_text(fixture, "standard", "jsonl"),
            "'{}': hekate-cli run no longer writes its checked-in Standard golden",
            fixture.id
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path("trace.sha256"))
                .expect("the hash file is written")
                .trim(),
            golden_text(fixture, "standard", "sha256").trim(),
            "'{}': hekate-cli run hashes differently from its golden",
            fixture.id
        );

        let manifest: RunManifest = serde_json::from_str(
            &std::fs::read_to_string(scratch.path("run").join(MANIFEST_FILE))
                .expect("the manifest is written"),
        )
        .expect("the manifest is JSON");
        assert_eq!(manifest.seed, fixture.pinned_seed, "'{}'", fixture.id);
        assert_eq!(manifest.ticks, fixture.standard_ticks, "'{}'", fixture.id);
        assert_eq!(manifest.step_s, STANDARD_STEP_S, "'{}'", fixture.id);
        assert_eq!(manifest.fidelity, "standard", "'{}'", fixture.id);
        assert_eq!(manifest.scenario.id, fixture.id);
        assert_eq!(
            manifest.scenario.source_path,
            repo_path(fixture.path).display().to_string(),
            "'{}'",
            fixture.id
        );
        assert_eq!(
            manifest.stream.uncompressed_sha256,
            golden_text(fixture, "standard", "sha256").trim(),
            "'{}'s manifest records the golden hash",
            fixture.id
        );

        let verified = Command::new(CLI)
            .arg("replay")
            .arg(scratch.path("run"))
            .arg("--verify")
            .current_dir(&scratch.dir)
            .output()
            .expect("hekate-cli runs");
        assert_eq!(
            verified.status.code(),
            Some(0),
            "'{}': {}",
            fixture.id,
            String::from_utf8_lossy(&verified.stderr)
        );
        assert_eq!(
            std::str::from_utf8(&verified.stdout).expect("the reproduced stream is UTF-8"),
            golden_text(fixture, "standard", "jsonl"),
            "'{}'s reproduced stream is not its checked-in golden",
            fixture.id
        );
    }
}
