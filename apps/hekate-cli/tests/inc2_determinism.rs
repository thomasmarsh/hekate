//! TAS-133: Increment 2 seeded reproducibility.
//!
//! `PHASE_2_PLAN.md`'s Increment 2 gate is that every fixture "repeats
//! identically at fixed seeds and required presets" and that a declared seed
//! bank "reproduces per-seed batch artifacts and replay verifies their event
//! streams". This suite is that gate for the six checked-in fixtures under
//! `scenarios/phase2/inc2/`:
//!
//! - one canary fixture repeats its canonical trace hash and bytes at its
//!   declared bank seed at Standard 50 ms, one of the `CC-OVERTAKE`/`CC-OPPOSE`
//!   cells' required presets; the per-fixture, per-preset reproduction, the Fine
//!   preset (whose runs are kept at the same simulated horizon by
//!   `converge::fidelity_ticks`), and the pinned bytes themselves belong to the
//!   golden suite in `inc2_trace.rs`;
//! - a seed outside the bank produces a different trace, so the stability above
//!   is not a constant;
//! - the checked-in bank `scenarios/phase2/inc2/inc2_seed_bank.json` declares
//!   exactly the pinned seeds; that each pinned seed admits its fixture's
//!   intended maneuver at *both* presets — so a reproduction is of a maneuver
//!   and not of an empty run — is `assert_maneuver_occurs`, which
//!   `inc2_trace.rs`'s `declared_run` runs for every fixture and preset before it
//!   compares hashes;
//! - a batch over the bank records every per-seed trace hash, cross-checks each
//!   against the canonical trace hash of the same run, and pins each run
//!   manifest's stream hash to that batch hash.
//!
//! The batch runs through the same `run_batch`/`write_run_directory` path the
//! `batch --seed-bank` command uses, and each per-seed hash is cross-checked
//! against [`canonical_trace`], so the manifest's link to a run is the canonical
//! trace hash rather than a second hashing scheme. The batch command only runs
//! the Standard step, so the bank batch is the Standard one; the Fine preset's
//! reproduction is pinned by `inc2_trace.rs`'s Fine goldens.
//!
//! Arrivals are a per-tick Bernoulli thinning of a Poisson process, so the
//! Standard and Fine grids realise different arrival series from one root seed.
//! That is why each fixture names a *pinned bank seed* rather than reusing the
//! benchmark seed 0 its own model suite is pinned to: see `inc2_support` for the
//! measurement and `scenarios/phase2/inc2/inc2_seed_bank.json` for the
//! declaration.

mod inc2_support;

use std::path::{Path, PathBuf};

use hekate_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, BatchRequest, EventRetention, RunManifest,
    SAMPLING_POLICY_VERSION, SamplingPolicy, SeedBankReference, Trace, TraceRecorder,
    TrajectorySampling, load_scenario_provenance, read_seed_bank, run_batch,
};
use hekate_model::{CompiledScenario, parse_scenario_source_v2};
use hekate_sim::{AgentId, Event, RunConfig, Seconds, Simulation, SnapshotDetail, maneuver_draw};
use inc2_support::{
    FIXTURES, Fixture, PRESETS, SEED_BANK, STANDARD_STEP_S, canonical, repo_path, run,
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

// The in-process driver's reproducibility is the canary below: one fixture's
// Standard run repeats its canonical trace bytes and hash at its pinned bank
// seed. The per-fixture, per-preset reproduction, the pinned bytes themselves,
// and the fact that the three autonomous fixtures' bytes are exactly the
// `canonical_trace` bytes `hekate-cli run` writes all belong to the golden suite
// in `inc2_trace.rs`, whose `assert_matches_golden` pins the checked-in artifact
// and whose CLI run/replay test proves `hekate-cli run` writes it.
//
// A seed outside the bank changes the trace, one `#[test]` per fixture, so the
// Standard reproducibility is not a constant.

/// The canary reproduction case: the same pinned seed and Standard step twice
/// produce the same canonical trace bytes and hash.
#[test]
fn narrow_passing_v2_reproduces_its_trace_hash_at_standard() {
    let (preset, step_s) = PRESETS[0];
    let fixture = &FIXTURES[0];
    let first = run(fixture, fixture.pinned_seed, step_s);
    let second = run(fixture, fixture.pinned_seed, step_s);
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

/// One fixture's different-seed case: a seed outside the bank produces a
/// different Standard trace, so the reproduction above is not a constant.
fn assert_other_seed_changes_trace(fixture: &Fixture) {
    let standard = run(fixture, fixture.pinned_seed, STANDARD_STEP_S);
    let other = run(fixture, OTHER_SEED, STANDARD_STEP_S);
    assert_ne!(
        standard.trace.hash(),
        other.trace.hash(),
        "{}: two different seeds produced one trace hash",
        fixture.id
    );
}

/// Generate the per-fixture different-seed tests, one `#[test]` per fixture.
macro_rules! other_seed_cases {
    ($(($name:ident, $fixture_index:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                assert_other_seed_changes_trace(&FIXTURES[$fixture_index]);
            }
        )+
    };
}

other_seed_cases!(
    (
        narrow_passing_v2_changes_its_trace_for_a_seed_outside_the_bank,
        0
    ),
    (
        motor_passing_narrow_v2_changes_its_trace_for_a_seed_outside_the_bank,
        1
    ),
    (
        motor_lane_change_v2_changes_its_trace_for_a_seed_outside_the_bank,
        2
    ),
    (
        narrow_passing_unsafe_v2_changes_its_trace_for_a_seed_outside_the_bank,
        3
    ),
    (
        motor_lane_change_boundary_v2_changes_its_trace_for_a_seed_outside_the_bank,
        4
    ),
    (
        narrow_wrong_way_v2_changes_its_trace_for_a_seed_outside_the_bank,
        5
    ),
);

/// The checked-in bank declares exactly the pinned seeds.
///
/// This is what makes the hash stability above meaningful: a hash comparison
/// over a run that never admits the fixture's pair proves only that an empty run
/// is empty. That every declared seed admits its fixture's maneuver at both
/// required presets is `assert_maneuver_occurs`, which `inc2_trace.rs`'s
/// `declared_run` runs for every fixture and preset; this test asserts only the
/// declaration.
#[test]
fn the_declared_seed_bank_declares_exactly_the_pinned_seeds() {
    let loaded = read_seed_bank(&repo_path(SEED_BANK)).expect("the declared seed bank reads");
    let mut pinned: Vec<u64> = FIXTURES.iter().map(|fixture| fixture.pinned_seed).collect();
    pinned.sort_unstable();
    pinned.dedup();
    assert_eq!(
        loaded.bank.seeds, pinned,
        "the declared bank is exactly the fixtures' pinned seeds"
    );
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

/// A batch over the declared seed bank records every per-seed trace hash, the
/// manifest names the declared bank, each run manifest's stream hash equals the
/// batch's trace hash, and every recorded hash is the canonical trace hash of
/// the same run.
#[test]
#[ignore = "slow: seed-bank batch over every fixture; run scripts/run-test-harness.sh"]
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
        let root = scratch.path(fixture.id);
        let batch = run_batch(batch_request(&root, &fixture, &seeds, &reference))
            .unwrap_or_else(|error| panic!("{} batch failed: {error}", fixture.id));

        assert_eq!(
            batch.seeds, seeds,
            "{}: the batch ran the bank's seeds",
            fixture.id
        );
        assert_eq!(
            batch
                .seed_bank
                .as_ref()
                .map(|bank| bank.content_sha256.as_str()),
            Some(loaded.content_sha256.as_str()),
            "{}: the batch must name the declared bank",
            fixture.id
        );
        assert_eq!(
            batch.spec.ticks, fixture.standard_ticks,
            "{}: the batch runs the fixture's declared horizon",
            fixture.id
        );
        assert_eq!(batch.spec.step_s, STANDARD_STEP_S, "{}", fixture.id);
        assert_batch_hash_is_canonical(&batch, &fixture);

        assert_eq!(batch.runs.len(), seeds.len());
        let mut recorded = Vec::new();
        for batch_run in &batch.runs {
            // The stream pin: the run manifest records the hash of the
            // uncompressed stream the batch wrote, and it is the batch's own
            // trace hash.
            assert_eq!(
                read_run_manifest(&root.join(&batch_run.directory))
                    .stream
                    .uncompressed_sha256,
                batch_run.trace_sha256,
                "{} seed {}: the run manifest's stream hash must be the batch's trace hash",
                fixture.id,
                batch_run.seed
            );
            recorded.push(format!("{}={}", batch_run.seed, batch_run.trace_sha256));
        }

        // The stored batch manifest is the one the API returned, so a consumer
        // reading batch.json sees the same pairing.
        let stored = read_batch_manifest(&root);
        let stored_hashes: Vec<&str> = stored
            .runs
            .iter()
            .map(|recorded| recorded.trace_sha256.as_str())
            .collect();
        let expected_hashes: Vec<&str> = batch
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
#[ignore = "slow: seed-bank batch over the declared seeds; run scripts/run-test-harness.sh"]
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

// The per-fixture "the banked seeds really run the fixtures" guard lives in
// `inc2_trace.rs`: `assert_golden_covers_its_maneuvers` requires every fixture's
// checked-in golden bytes to record a spawn event at both presets, and
// `assert_matches_golden` pins those bytes to a live run, so a fixture that
// stopped admitting agents fails there rather than here.

// ---------------------------------------------------------------------------
// TAS-134: stream isolation of the Increment 2 maneuver draws
// ---------------------------------------------------------------------------
//
// `PHASE_2_PLAN.md` requires that "adding an unrelated mode, passenger cohort,
// or group" does not change another agent's owned draws, and that adding a
// stream cannot perturb another through draw-order coupling. The kernel's
// guarantee is the named-stream derivation in `crates/hekate-sim/src/rng.rs`:
// the `demand` stream is one substream per vehicle source keyed by the source's
// *declared index*, the `maneuver` stream is one substream per agent keyed by
// the stable agent id and the agent's own decision ordinal, and adding a draw
// to one never advances another. This suite is the run-level gate for that
// contract on the Increment 2 maneuver draw.
//
// The scenario below authors one focus facility that turns a rider onto the
// connected opposing traversal — the only place a run consumes the keyed
// `maneuver` draw (`crates/hekate-sim/src/wrong_way.rs`'s `maneuver_draw`) —
// beside two facilities the unrelated demand would use: a passenger-car road
// and a bicycle lane. Three runs are compared at the fixed root seed:
//
// - the focus demand alone;
// - the focus demand plus an unrelated passenger-car source and an unrelated
//   bicycle source, declared after the focus source;
// - the same two added sources with their own declaration order reversed.
//
// Adding unrelated *arriving* demand does not leave every focus agent's id
// untouched: agent ids are assigned by admission order, so once an unrelated
// agent is admitted the focus agents admitted after it take a different id and
// therefore a different per-agent `profile` draw. That is not draw-order
// coupling — the focus source's own `demand` substream is unchanged, as its
// arrival series proves — so the trace comparison is scoped to the focus agents
// admitted *before* the first unrelated agent, the agents the added demand
// genuinely does not affect. Their ids, their own canonical trace lines, and
// their keyed `maneuver` draws must be identical across all three runs.

/// The fixed root seed the stream-isolation runs use.
const ISOLATION_SEED: u64 = 7;

/// Steps a stream-isolation run advances: 60 s at the Standard 50 ms step, long
/// enough for both added sources to admit an agent while the focus source keeps
/// admitting riders.
const ISOLATION_TICKS: u64 = 1200;

/// The focus facility's guide path, the densest path of [`isolation_scenario`].
/// Every spawn on it belongs to the focus rider source.
const FOCUS_PATH: usize = 0;

/// The scenario every stream-isolation run compiles: one focus facility `a`
/// beside its reverse continuance `left` (so `a`'s opposing traversal is
/// physically connected and the wrong-way policy is reachable), an unrelated
/// passenger-car road, and an unrelated bicycle lane.
///
/// The rider mode carries `reverse_direction` and a fully non-compliant profile
/// so the focus agent's keyed `maneuver` draw decides the turn, exactly as
/// `crates/hekate-sim/tests/wrong_way.rs`'s rider does.
const ISOLATION_HEAD: &str = r#"{
  schema_version: 2,
  id: 'inc2_stream_isolation',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'guide_a', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
    { id: 'guide_left', points: [ { x: -120.0, y: 0.0 }, { x: 0.0, y: 0.0 } ] },
    { id: 'car_guide', points: [ { x: 0.0, y: 20.0 }, { x: 120.0, y: 20.0 } ] },
    { id: 'narrow_guide', points: [ { x: 0.0, y: -20.0 }, { x: 120.0, y: -20.0 } ] },
  ],
  portals: [
    { id: 'a_entry', path: 'guide_a', end: 'start', width_m: 3.0 },
    { id: 'a_exit', path: 'guide_a', end: 'end', width_m: 3.0 },
    { id: 'car_entry', path: 'car_guide', end: 'start', width_m: 3.5 },
    { id: 'car_exit', path: 'car_guide', end: 'end', width_m: 3.5 },
    { id: 'narrow_entry', path: 'narrow_guide', end: 'start', width_m: 3.5 },
    { id: 'narrow_exit', path: 'narrow_guide', end: 'end', width_m: 3.5 },
  ],
  boundaries: [ { id: 'world', points: [
    { x: -140.0, y: -40.0 }, { x: 140.0, y: -40.0 },
    { x: 140.0, y: 40.0 }, { x: -140.0, y: 40.0 },
  ] } ],
  regions: [
    { id: 'band_a', points: [ { x: 0.0, y: -1.5 }, { x: 120.0, y: -1.5 },
      { x: 120.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
    { id: 'band_left', points: [ { x: -120.0, y: -1.5 }, { x: 0.0, y: -1.5 },
      { x: 0.0, y: 1.5 }, { x: -120.0, y: 1.5 } ] },
    { id: 'car_band', points: [ { x: 0.0, y: 18.25 }, { x: 120.0, y: 18.25 },
      { x: 120.0, y: 21.75 }, { x: 0.0, y: 21.75 } ] },
    { id: 'narrow_band', points: [ { x: 0.0, y: -21.75 }, { x: 120.0, y: -21.75 },
      { x: 120.0, y: -18.25 }, { x: 0.0, y: -18.25 } ] },
  ],
  facilities: [
    { id: 'a', region: 'band_a', reference_path: 'guide_a', width_m: 3.0,
      nominal_direction: 'forward', access: { modes: [ 'rider' ] },
      lateral_use: 'shared', speed_policy: { limit_mps: null } },
    { id: 'left', region: 'band_left', reference_path: 'guide_left', width_m: 3.0,
      nominal_direction: 'forward', access: { modes: [ 'rider' ] },
      lateral_use: 'shared', speed_policy: { limit_mps: null } },
    { id: 'car_road', region: 'car_band', reference_path: 'car_guide', width_m: 3.5,
      nominal_direction: 'forward', access: { modes: [ 'passenger_car' ] },
      lateral_use: 'shared', speed_policy: { limit_mps: null } },
    { id: 'narrow_lane', region: 'narrow_band', reference_path: 'narrow_guide', width_m: 3.5,
      nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
      lateral_use: 'shared', speed_policy: { limit_mps: null } },
  ],
  facility_connectors: [
    { id: 'left_into_a',
      from: { facility: 'left', direction: 'forward' },
      to: { facility: 'a', direction: 'forward' } },
    { id: 'a_back_to_left',
      from: { facility: 'a', direction: 'reverse' },
      to: { facility: 'left', direction: 'reverse' } },
  ],
  movements: [
    { id: 'a_through', from: 'a_entry', to: 'a_exit', path: 'guide_a',
      priority: 0, direction: 'forward' },
    { id: 'car_through', from: 'car_entry', to: 'car_exit', path: 'car_guide',
      priority: 0, direction: 'forward' },
    { id: 'narrow_through', from: 'narrow_entry', to: 'narrow_exit', path: 'narrow_guide',
      priority: 0, direction: 'forward' },
  ],
  mode_templates: [
    { id: 'rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'reverse_direction' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 6.0, max: 6.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 0.0, max: 0.0 },
      } },
    { id: 'passenger_car',
      body: { kind: 'box', length_m: { min: 4.5, max: 4.5 },
        width_m: { min: 1.8, max: 1.8 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'overtake' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 9.0, max: 9.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
      lateral: { target_clearance_m: 0.75, horizon_s: 2.0 } },
    { id: 'bicycle',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 4.5, max: 4.5 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      } },
  ],
  maneuver_policy: {
    commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
    wrong_way: { min_time_saving_s: 0.0, max_opposing_density_per_km: 100.0,
      urgency: 1.0 },
  },
"#;

/// The three demand sources the scenario chooses between, as authored JSON5.
fn isolation_demand(with_unrelated: bool, reverse: bool, focus_last: bool) -> Vec<&'static str> {
    let focus = "\n  { id: 'rider_inflow', mode: 'rider',\n    spawn: { rate: { portal: 'a_entry', rate_per_hour: 300.0,\n      interval_s: { start_s: 0.0, end_s: null },\n      choice: { movements: [ { movement: 'a_through', weight: 1.0 } ] } } } },";
    let car = "\n  { id: 'car_inflow', mode: 'passenger_car',\n    spawn: { rate: { portal: 'car_entry', rate_per_hour: 300.0,\n      interval_s: { start_s: 0.0, end_s: null },\n      choice: { movements: [ { movement: 'car_through', weight: 1.0 } ] } } } },";
    let narrow = "\n  { id: 'narrow_inflow', mode: 'bicycle',\n    spawn: { rate: { portal: 'narrow_entry', rate_per_hour: 300.0,\n      interval_s: { start_s: 0.0, end_s: null },\n      choice: { movements: [ { movement: 'narrow_through', weight: 1.0 } ] } } } },";
    let (first, second) = if reverse {
        (narrow, car)
    } else {
        (car, narrow)
    };
    let mut sources = Vec::new();
    if with_unrelated && focus_last {
        sources.push(first);
        sources.push(second);
        sources.push(focus);
    } else {
        sources.push(focus);
        if with_unrelated {
            sources.push(first);
            sources.push(second);
        }
    }
    sources
}

/// The stream-isolation scenario with the focus source, and optionally the two
/// unrelated sources declared after it (or, reversed, between each other).
fn isolation_scenario(with_unrelated: bool, reverse: bool) -> String {
    let mut source = String::from(ISOLATION_HEAD);
    source.push_str("\n  demand: [");
    for entry in isolation_demand(with_unrelated, reverse, false) {
        source.push_str(entry);
    }
    source.push_str("\n  ],\n}\n");
    source
}

/// The falsification probe's scenario: the same three sources, but the two
/// unrelated ones are declared *before* the focus source, so the focus source's
/// dense demand index — the key its `demand` substream is derived from — moves
/// from 0 to 2.
fn isolation_scenario_coupled() -> String {
    let mut source = String::from(ISOLATION_HEAD);
    source.push_str("\n  demand: [");
    for entry in isolation_demand(true, false, true) {
        source.push_str(entry);
    }
    source.push_str("\n  ],\n}\n");
    source
}

/// Compile one stream-isolation scenario.
fn compile_isolation(source: &str) -> CompiledScenario {
    let parsed = parse_scenario_source_v2(source).expect("the isolation scenario parses");
    CompiledScenario::compile_v2(parsed).expect("the isolation scenario compiles")
}

/// One stream-isolation run: its canonical trace, its admission order, and the
/// wrong-way entry the driver requested.
struct IsolationRun {
    /// The canonical trace bytes and hash.
    trace: Trace,
    /// Every spawn in admission order: `(tick, agent id, path index, distance_m)`.
    spawns: Vec<(u64, u32, usize, f64)>,
    /// The first wrong-way entry the driver requested, and the kernel's answer.
    request: Option<(u32, bool)>,
}

/// Run one stream-isolation scenario at [`ISOLATION_SEED`], driving the focus
/// rider's wrong-way request once, as soon as the first rider is admitted onto
/// the focus facility.
///
/// The request is driven exactly as the landed wrong-way suites drive it, and
/// the recorded trace is the canonical bytes and hash `TraceRecorder` produces
/// for the run.
fn isolation_run(source: &str) -> IsolationRun {
    let scenario = compile_isolation(source);
    let config = RunConfig::new(ISOLATION_SEED).with_step(Seconds::from_secs(STANDARD_STEP_S));
    let mut sim = Simulation::new(scenario, config).expect("the isolation scenario builds");
    let mut recorder = TraceRecorder::new(&sim, &config, ISOLATION_TICKS);
    let mut spawns = Vec::new();
    let mut request = None;
    for _ in 0..ISOLATION_TICKS {
        if request.is_none()
            && let Some(agent) = first_focus_agent(&sim)
        {
            request = Some((agent.get(), sim.request_wrong_way_entry(agent)));
        }
        let output = sim.step();
        recorder.record(&output);
        let tick = output.time().tick();
        for event in output.events() {
            if let Event::Spawned {
                agent,
                path,
                distance_m,
                ..
            } = event
            {
                spawns.push((tick, agent.get(), path.index(), *distance_m));
            }
        }
    }
    IsolationRun {
        trace: recorder.finish(sim.finish()),
        spawns,
        request,
    }
}

/// The lowest-id live agent riding the focus facility's guide path.
fn first_focus_agent(sim: &Simulation) -> Option<AgentId> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter(|sample| {
            sample
                .motion
                .as_ref()
                .is_some_and(|motion| motion.path.index() == FOCUS_PATH)
        })
        .map(|sample| sample.id)
        .min()
}

/// Every focus arrival, as `(tick, distance_m)`, in admission order — the focus
/// `demand` substream's own output, independent of every other agent's id.
fn focus_arrivals(run: &IsolationRun) -> Vec<(u64, f64)> {
    run.spawns
        .iter()
        .filter(|(_, _, path, _)| *path == FOCUS_PATH)
        .map(|(tick, _, _, distance_m)| (*tick, *distance_m))
        .collect()
}

/// The ids of the agents the unrelated demand admitted, in admission order.
fn unrelated_ids(run: &IsolationRun) -> Vec<u32> {
    run.spawns
        .iter()
        .filter(|(_, _, path, _)| *path != FOCUS_PATH)
        .map(|(_, agent, _, _)| *agent)
        .collect()
}

/// The id of the first unrelated agent, the point after which a focus agent's
/// own id is no longer the id it would take without the added demand.
fn first_unrelated_id(run: &IsolationRun) -> Option<u32> {
    unrelated_ids(run).into_iter().min()
}

/// The canonical trace lines that name `agent`, in stream order.
fn agent_trace_lines(trace: &Trace, agent: u32) -> Vec<String> {
    let needle = format!("\"agent\":{agent},");
    std::str::from_utf8(trace.bytes())
        .expect("a canonical trace is UTF-8")
        .lines()
        .filter(|line| line.contains(&needle))
        .map(str::to_owned)
        .collect()
}

/// The focus agents whose id is below `cut` — the agents admitted before the
/// added demand's first arrival — each with its own canonical trace lines.
fn unaffected_focus_lines(run: &IsolationRun, cut: u32) -> Vec<(u32, Vec<String>)> {
    run.spawns
        .iter()
        .filter(|(_, agent, path, _)| *path == FOCUS_PATH && *agent < cut)
        .map(|(_, agent, _, _)| (*agent, agent_trace_lines(&run.trace, *agent)))
        .collect()
}

/// The keyed `maneuver` draw of each unaffected focus agent, as
/// `(agent id, draw)`.
fn unaffected_maneuver_draws(run: &IsolationRun, cut: u32) -> Vec<(u32, f64)> {
    run.spawns
        .iter()
        .filter(|(_, agent, path, _)| *path == FOCUS_PATH && *agent < cut)
        .map(|(_, agent, _, _)| {
            (
                *agent,
                maneuver_draw(ISOLATION_SEED, AgentId::from_index(*agent as usize), 0),
            )
        })
        .collect()
}

/// Adding an unrelated passenger-car source and an unrelated bicycle source —
/// in either declaration order — leaves the focus source's arrivals, the
/// unaffected focus agents' canonical trace lines, and their keyed `maneuver`
/// draws unchanged.
#[test]
fn the_inc2_maneuver_stream_stays_isolated_from_unrelated_car_and_narrow_demand() {
    let base = isolation_run(&isolation_scenario(false, false));
    let augmented = isolation_run(&isolation_scenario(true, false));
    let reversed = isolation_run(&isolation_scenario(true, true));

    // The focus run really turns its rider, so the maneuver draw below is
    // exercised rather than merely present.
    assert_eq!(
        base.request,
        Some((0, true)),
        "the focus rider's first wrong-way entry must be admitted"
    );
    assert!(
        !focus_arrivals(&base).is_empty(),
        "the focus source admitted no rider, so the isolation claim is empty"
    );

    // The added sources really admit agents, so the isolation claim is not the
    // trivial consequence of adding nothing.
    for (name, run) in [("augmented", &augmented), ("reversed", &reversed)] {
        assert!(
            !unrelated_ids(run).is_empty(),
            "{name}: the added car and narrow demand admitted no agent"
        );
        assert_eq!(
            base.request, run.request,
            "{name}: the focus rider's wrong-way request changed"
        );
    }

    // The compared prefix really carries the maneuver the run took: the first
    // focus agent's own trace lines record the opposing traversal its keyed
    // `maneuver` draw opened. Otherwise the trace comparison below could pass
    // over a run with no maneuver in it.
    let focus_lines = &unaffected_focus_lines(&base, 1)[0].1;
    assert!(
        focus_lines
            .iter()
            .any(|line| line.contains("\"event\":\"opposing_traversal\"")),
        "the unaffected focus agent's trace must record its opposing traversal: {focus_lines:?}"
    );

    // The cut: the first id the added demand took. The focus agents below it are
    // admitted before any unrelated agent, so nothing the added demand does can
    // have changed their id — or the draws keyed by it.
    let cut = [
        first_unrelated_id(&augmented),
        first_unrelated_id(&reversed),
    ]
    .into_iter()
    .flatten()
    .min()
    .expect("the added demand admitted at least one agent");
    assert!(
        cut > 0,
        "the added demand must be admitted after the focus source's first rider"
    );

    // The owned `maneuver` draws are keyed by the agent, not shared: distinct
    // focus agents hold distinct draws, so an unkeyed maneuver choice that gave
    // every agent one stream fails here.
    let base_draws = unaffected_maneuver_draws(&base, cut);
    assert!(
        base_draws.len() >= 2,
        "the comparison needs at least two unaffected focus agents, got {base_draws:?}"
    );
    for (index, (first_id, first_draw)) in base_draws.iter().enumerate() {
        for (second_id, second_draw) in &base_draws[index + 1..] {
            assert_ne!(
                first_draw, second_draw,
                "focus agents {first_id} and {second_id} share one maneuver draw"
            );
        }
    }

    for (name, run) in [("augmented", &augmented), ("reversed", &reversed)] {
        assert_eq!(
            focus_arrivals(&base),
            focus_arrivals(run),
            "{name}: the focus source's own arrival stream moved when unrelated demand was added"
        );
        assert_eq!(
            unaffected_focus_lines(&base, cut),
            unaffected_focus_lines(run, cut),
            "{name}: an unaffected focus agent's trace changed when unrelated demand was added"
        );
        assert_eq!(
            base_draws,
            unaffected_maneuver_draws(run, cut),
            "{name}: an unaffected focus agent's keyed maneuver draw moved"
        );
    }

    println!(
        "focus arrivals {:?}; first unrelated id {cut}; unaffected agents {:?}",
        focus_arrivals(&base),
        unaffected_focus_lines(&base, cut)
            .iter()
            .map(|(agent, _)| *agent)
            .collect::<Vec<_>>()
    );
}

/// The falsification probe: the same isolation comparison catches a
/// draw-order coupling.
///
/// The probe declares the unrelated car and narrow sources *before* the focus
/// source, so the focus source's dense demand index changes and the `demand`
/// substream keyed by it changes with it. The focus arrivals must move — if they
/// did not, the check above would be vacuous — which is exactly the coupling
/// named-stream keying prevents and the check is meant to catch. Because the
/// unrelated agents are admitted first, the focus agent that would be id 0
/// takes a higher id, so its own trace lines and its keyed `maneuver` draw move
/// too; the same comparisons the isolation test runs report both.
#[test]
fn the_stream_isolation_check_flags_unrelated_demand_declared_before_the_focus() {
    let base = isolation_run(&isolation_scenario(false, false));
    let coupled = isolation_run(&isolation_scenario_coupled());

    assert_ne!(
        focus_arrivals(&base),
        focus_arrivals(&coupled),
        "declaring unrelated demand before the focus source must move the focus arrival stream; \
         if it does not, the isolation check cannot catch draw-order coupling"
    );
    assert_eq!(
        first_unrelated_id(&coupled),
        Some(0),
        "the probe must admit an unrelated agent before any focus agent"
    );

    // The comparisons the isolation test runs must report the difference: the
    // focus agent that led the base run is not the agent that leads the coupled
    // one, so neither its trace lines nor its keyed draw line up.
    assert_ne!(
        base.request, coupled.request,
        "the coupling must move the requested focus agent's id"
    );
    assert_ne!(
        unaffected_focus_lines(&base, 1),
        unaffected_focus_lines(&coupled, 1),
        "the check must report the coupled focus agent's moved trace"
    );
    assert_ne!(
        unaffected_maneuver_draws(&base, 1),
        unaffected_maneuver_draws(&coupled, 1),
        "the check must report the coupled focus agent's moved maneuver draw"
    );
}
