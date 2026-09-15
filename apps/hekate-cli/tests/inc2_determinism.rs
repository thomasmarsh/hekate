//! TAS-133: Increment 2 seeded reproducibility.
//!
//! `PHASE_2_PLAN.md`'s Increment 2 gate is that every fixture "repeats
//! identically at fixed seeds and required presets" and that a declared seed
//! bank "reproduces per-seed batch artifacts and replay verifies their event
//! streams". This suite is that gate for the six checked-in fixtures under
//! `scenarios/phase2/inc2/`:
//!
//! - every fixture repeats its canonical trace hash and bytes at its declared
//!   bank seed at both required presets, Standard 50 ms and Fine 20 ms — the
//!   `CC-OVERTAKE`/`CC-OPPOSE` cells' declared presets — with the Fine run kept
//!   at the same simulated horizon by `converge::fidelity_ticks`;
//! - a seed outside the bank produces a different trace, so the stability above
//!   is not a constant;
//! - the checked-in bank `scenarios/phase2/inc2/inc2_seed_bank.json` declares
//!   exactly the pinned seeds, and each pinned seed admits its fixture's
//!   intended maneuver at *both* presets, so the reproduction is a reproduction
//!   of a maneuver and not of an empty run;
//! - a batch over the bank reproduces every per-seed trace hash and its
//!   compressed event stream byte for byte, and each recorded hash is the
//!   canonical trace hash of the same run.
//!
//! The batch runs through the same `run_batch`/`write_run_directory` path the
//! `batch --seed-bank` command uses, and each per-seed hash is cross-checked
//! against [`canonical_trace`], so the manifest's link to a run is the canonical
//! trace hash rather than a second hashing scheme. The batch command only runs
//! the Standard step, so the bank batch is the Standard one; the Fine preset's
//! reproduction is the in-process half above.
//!
//! Arrivals are a per-tick Bernoulli thinning of a Poisson process, so the
//! Standard and Fine grids realise different arrival series from one root seed.
//! That is why each fixture names a *pinned bank seed* rather than reusing the
//! benchmark seed 0 its own model suite is pinned to: see `inc2_support` for the
//! measurement and `scenarios/phase2/inc2/inc2_seed_bank.json` for the
//! declaration.

mod inc2_support;

use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use hekate_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, BatchRequest, EVENT_STREAM_FILE, EventRetention,
    RunManifest, SAMPLING_POLICY_VERSION, SamplingPolicy, SeedBankReference, TrajectorySampling,
    load_scenario_provenance, read_seed_bank, run_batch,
};
use hekate_sim::ManeuverState;
use inc2_support::{
    FIXTURES, Fixture, PRESETS, Plan, SEED_BANK, STANDARD_STEP_S, assert_maneuver_occurs,
    canonical, repo_path, run,
};

/// A seed that is not banked, used only to prove a trace hash is not constant.
const OTHER_SEED: u64 = 1_000_000;

/// A scratch working directory removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-inc2-determinism-{name}-{}",
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

/// Every checked-in Increment 2 fixture repeats its canonical trace bytes and
/// hash at the fixed pinned seed at both required presets, and a different seed
/// changes the trace.
///
/// The driven runs are `TraceRecorder` runs, and for the three fixtures whose
/// maneuver needs no request they are asserted to be exactly the
/// [`canonical_trace`] bytes `hekate-cli run` writes, so the Standard half of
/// the reproducibility claim is CLI-reproducible for those three.
#[test]
fn every_inc2_fixture_reproduces_its_trace_hash_at_both_required_presets() {
    for fixture in FIXTURES {
        for (preset, step_s) in PRESETS {
            let first = run(&fixture, fixture.pinned_seed, step_s);
            let second = run(&fixture, fixture.pinned_seed, step_s);
            assert_eq!(
                first.trace.hash(),
                second.trace.hash(),
                "{} [{preset}]: the same seed and step produced two trace hashes",
                fixture.id
            );
            assert_eq!(
                first.trace.bytes(),
                second.trace.bytes(),
                "{} [{preset}]: the same seed and step produced two traces",
                fixture.id
            );
            println!(
                "{} [{preset}] seed {} {} bytes {}",
                fixture.id,
                fixture.pinned_seed,
                first.trace.hash(),
                first.trace.bytes().len()
            );
        }

        let standard = run(&fixture, fixture.pinned_seed, STANDARD_STEP_S);
        let other = run(&fixture, OTHER_SEED, STANDARD_STEP_S);
        assert_ne!(
            standard.trace.hash(),
            other.trace.hash(),
            "{}: two different seeds produced one trace hash",
            fixture.id
        );

        if fixture.plan == Plan::Autonomous {
            let direct = canonical(&fixture, fixture.pinned_seed, STANDARD_STEP_S);
            assert_eq!(
                standard.trace.bytes(),
                direct.bytes(),
                "{}: a run with no request must be the canonical trace `hekate-cli run` writes",
                fixture.id
            );
        }
    }
}

/// The checked-in bank declares exactly the pinned seeds, and every pinned seed
/// admits its fixture's intended maneuver at both required presets.
///
/// This is what makes the hash stability above meaningful: a hash comparison
/// over a run that never admits the fixture's pair proves only that an empty run
/// is empty.
#[test]
fn the_declared_seed_bank_admits_every_fixtures_maneuver_at_both_required_presets() {
    let loaded = read_seed_bank(&repo_path(SEED_BANK)).expect("the declared seed bank reads");
    let mut pinned: Vec<u64> = FIXTURES.iter().map(|fixture| fixture.pinned_seed).collect();
    pinned.sort_unstable();
    pinned.dedup();
    assert_eq!(
        loaded.bank.seeds, pinned,
        "the declared bank is exactly the fixtures' pinned seeds"
    );

    for fixture in FIXTURES {
        assert!(
            loaded.bank.seeds.contains(&fixture.pinned_seed),
            "{}: pinned seed {} is not a declared bank seed",
            fixture.id,
            fixture.pinned_seed
        );
        for (preset, step_s) in PRESETS {
            let run = run(&fixture, fixture.pinned_seed, step_s);
            assert_maneuver_occurs(&fixture, preset, &run);
            assert!(
                run.observation
                    .edges
                    .values()
                    .all(|edges| edges.iter().all(|(_, to, _)| *to != ManeuverState::Aborted)),
                "{} [{preset}]: the fixture's maneuver must not abort",
                fixture.id
            );
            println!(
                "{} [{preset}] seed {} admits its maneuver: {} overtaking interval(s), {} handoff(s), {} wrong-way entr(ies)",
                fixture.id,
                fixture.pinned_seed,
                run.observation.overtakes.len(),
                run.observation.facility_transitions,
                run.observation.wrong_way.len()
            );
        }
    }
}

/// The decompressed canonical event stream of a run directory.
fn event_stream(directory: &Path) -> Vec<u8> {
    let compressed =
        std::fs::read(directory.join(EVENT_STREAM_FILE)).expect("the event stream is written");
    let mut decoded = Vec::new();
    GzDecoder::new(&compressed[..])
        .read_to_end(&mut decoded)
        .expect("the event stream decompresses");
    decoded
}

fn batch_request(
    root: &Path,
    fixture: &Fixture,
    seeds: &[u64],
    seed_bank: &SeedBankReference,
) -> BatchRequest {
    let (scenario, provenance) =
        load_scenario_provenance(&repo_path(fixture.path)).expect("the fixture loads");
    BatchRequest {
        root: root.to_path_buf(),
        scenario,
        provenance,
        ticks: fixture.standard_ticks,
        step_s: STANDARD_STEP_S,
        sampling: SamplingPolicy {
            policy_version: SAMPLING_POLICY_VERSION,
            events: EventRetention::All,
            trajectories: TrajectorySampling::off(),
        },
        seeds: seeds.to_vec(),
        seed_bank: Some(seed_bank.clone()),
        jobs: 1,
    }
}

fn read_batch_manifest(root: &Path) -> BatchManifest {
    let json = std::fs::read_to_string(root.join(BATCH_MANIFEST_FILE)).expect("batch.json is read");
    serde_json::from_str(&json).expect("batch.json is JSON")
}

fn read_run_manifest(directory: &Path) -> RunManifest {
    let json = std::fs::read_to_string(directory.join(hekate_cli::MANIFEST_FILE))
        .expect("manifest.json is written");
    serde_json::from_str(&json).expect("manifest.json is JSON")
}

/// The batch's per-seed trace hash equals the canonical trace hash of the same
/// run, so the manifest links to the run by the canonical hash and not a second
/// scheme.
fn assert_batch_hash_is_canonical(manifest: &BatchManifest, fixture: &Fixture) {
    for recorded in &manifest.runs {
        let expected = canonical(fixture, recorded.seed, manifest.spec.step_s);
        assert_eq!(
            recorded.trace_sha256,
            expected.hash(),
            "{} seed {}: the batch recorded a hash that is not the canonical trace hash",
            fixture.id,
            recorded.seed
        );
    }
}

/// A batch over the declared seed bank reproduces every per-seed trace hash and
/// event stream across two runs, and the manifest names the declared bank.
#[test]
fn the_inc2_seed_bank_batch_reproduces_every_per_seed_hash_and_event_stream() {
    let scratch = Scratch::new("seed-bank-batch");
    let loaded = read_seed_bank(&repo_path(SEED_BANK)).expect("the declared seed bank reads");
    let reference = SeedBankReference {
        path: SEED_BANK.to_owned(),
        content_sha256: loaded.content_sha256.clone(),
    };
    let seeds = loaded.bank.seeds.clone();
    assert!(
        seeds.len() >= 2,
        "a bank that pairs runs needs at least two seeds, got {}",
        seeds.len()
    );

    for fixture in FIXTURES {
        let left_root = scratch.path(&format!("{}-left", fixture.id));
        let right_root = scratch.path(&format!("{}-right", fixture.id));
        let left = run_batch(batch_request(&left_root, &fixture, &seeds, &reference))
            .unwrap_or_else(|error| panic!("{} left batch failed: {error}", fixture.id));
        let right = run_batch(batch_request(&right_root, &fixture, &seeds, &reference))
            .unwrap_or_else(|error| panic!("{} right batch failed: {error}", fixture.id));

        for manifest in [&left, &right] {
            assert_eq!(
                manifest.seeds, seeds,
                "{}: the batch ran the bank's seeds",
                fixture.id
            );
            assert_eq!(
                manifest
                    .seed_bank
                    .as_ref()
                    .map(|bank| bank.content_sha256.as_str()),
                Some(loaded.content_sha256.as_str()),
                "{}: the batch must name the declared bank",
                fixture.id
            );
            assert_eq!(
                manifest.spec.ticks, fixture.standard_ticks,
                "{}: the batch runs the fixture's declared horizon",
                fixture.id
            );
            assert_eq!(manifest.spec.step_s, STANDARD_STEP_S, "{}", fixture.id);
        }
        assert_batch_hash_is_canonical(&left, &fixture);

        assert_eq!(left.runs.len(), seeds.len());
        let mut recorded = Vec::new();
        for (left_run, right_run) in left.runs.iter().zip(&right.runs) {
            assert_eq!(
                left_run.seed, right_run.seed,
                "{}: per-seed pairing must line up",
                fixture.id
            );
            assert_eq!(
                left_run.trace_sha256, right_run.trace_sha256,
                "{} seed {}: two batches disagreed on the trace hash",
                fixture.id, left_run.seed
            );
            let left_event = event_stream(&left_root.join(&left_run.directory));
            let right_event = event_stream(&right_root.join(&right_run.directory));
            assert_eq!(
                left_event, right_event,
                "{} seed {}: two batches disagreed on the event stream",
                fixture.id, left_run.seed
            );
            assert_eq!(
                read_run_manifest(&left_root.join(&left_run.directory))
                    .stream
                    .uncompressed_sha256,
                left_run.trace_sha256,
                "{} seed {}: the run manifest's stream hash must be the batch's trace hash",
                fixture.id,
                left_run.seed
            );
            recorded.push(format!("{}={}", left_run.seed, left_run.trace_sha256));
        }

        // The stored batch manifest is the one the API returned, so a consumer
        // reading batch.json sees the same pairing.
        let stored = read_batch_manifest(&left_root);
        let stored_hashes: Vec<&str> = stored
            .runs
            .iter()
            .map(|recorded| recorded.trace_sha256.as_str())
            .collect();
        let expected_hashes: Vec<&str> = left
            .runs
            .iter()
            .map(|recorded| recorded.trace_sha256.as_str())
            .collect();
        assert_eq!(
            stored_hashes, expected_hashes,
            "{}: batch.json must hold the run's own hashes",
            fixture.id
        );
        println!("{}: {recorded:?}", fixture.id);
    }
}

/// The declared bank and the canonical trace path agree: an explicit `--seeds`
/// batch over the bank's seeds records the same hash the batch-over-bank path
/// records, so the bank adds pairing without changing the runs.
#[test]
fn the_declared_seed_bank_runs_the_same_traces_as_an_explicit_seed_list() {
    let scratch = Scratch::new("bank-vs-explicit");
    let loaded = read_seed_bank(&repo_path(SEED_BANK)).expect("the declared seed bank reads");
    let seeds = loaded.bank.seeds.clone();
    let fixture = FIXTURES[0];

    let banked = run_batch(batch_request(
        &scratch.path(&format!("{}-banked", fixture.id)),
        &fixture,
        &seeds,
        &SeedBankReference {
            path: SEED_BANK.to_owned(),
            content_sha256: loaded.content_sha256.clone(),
        },
    ))
    .expect("the banked batch runs");

    let mut request = batch_request(
        &scratch.path(&format!("{}-explicit", fixture.id)),
        &fixture,
        &seeds,
        &SeedBankReference {
            path: SEED_BANK.to_owned(),
            content_sha256: loaded.content_sha256.clone(),
        },
    );
    request.seed_bank = None;
    let explicit = run_batch(request).expect("the explicit batch runs");

    assert_batch_hash_is_canonical(&banked, &fixture);
    let hashes: Vec<&str> = banked
        .runs
        .iter()
        .map(|recorded| recorded.trace_sha256.as_str())
        .collect();
    let explicit_hashes: Vec<&str> = explicit
        .runs
        .iter()
        .map(|recorded| recorded.trace_sha256.as_str())
        .collect();
    assert_eq!(hashes, explicit_hashes, "the bank must not change the runs");
}

/// The banked seeds really run the fixtures, so the batch evidence above is not
/// an artifact of empty runs: each fixture's canonical trace admits agents.
#[test]
fn the_inc2_seed_bank_batch_runs_every_fixture() {
    for fixture in FIXTURES {
        let run = run(&fixture, fixture.pinned_seed, STANDARD_STEP_S);
        let spawned = std::str::from_utf8(run.trace.bytes())
            .expect("a trace is UTF-8")
            .lines()
            .filter(|line| line.contains("\"event\":\"spawned\""))
            .count();
        assert!(
            spawned > 0,
            "{}: its canonical trace records no spawn event",
            fixture.id
        );
        assert!(
            spawned as usize == run.observation.spawned.len(),
            "{}: every agent the trace admits is an observed spawn",
            fixture.id
        );
    }
}
