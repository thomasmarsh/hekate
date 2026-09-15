//! TAS-079: Increment 1 narrow-mode seeded reproducibility.
//!
//! `PHASE_2_PLAN.md` *Increment 1* gates the increment on "Repeated seeded runs
//! reproduce demand, profiles, decisions, events, and trace hashes". This suite
//! is that gate:
//!
//! - every checked-in `narrow_isolated_*` fixture runs twice at the fixed seed
//!   `20_260_913` and must produce an identical canonical trace hash at each of
//!   the matrix's `F`/`S`/`f` presets (0.1/0.05/0.02 s) — the "recorded trace
//!   hash is stable across the checked-in presets tested" clause;
//! - a batch over the checked-in seed bank
//!   `scenarios/phase2/inc1/narrow_isolated_seed_bank.json` records every
//!   per-seed trace hash, cross-checks each against the canonical trace hash of
//!   the same run, and pins each run manifest's stream hash to that batch hash.
//!
//! The batch runs through the same `run_batch`/`write_run_directory` path the
//! `batch` command uses, and each per-seed hash is cross-checked against
//! [`canonical_trace`] at the same seed, so the manifest's link to a run is the
//! canonical trace hash rather than a second hashing scheme.

use std::path::{Path, PathBuf};

use hekate_cli::{
    BATCH_MANIFEST_FILE, BatchManifest, BatchRequest, EventRetention, RunManifest, SamplingPolicy,
    SeedBankReference, TrajectorySampling, canonical_trace, load_scenario_provenance,
    read_seed_bank, run_batch,
};
use hekate_sim::{Event, RunConfig, Seconds};

/// The checked-in Increment 1 narrow isolated fixtures, by id and path.
const FIXTURES: [(&str, &str); 6] = [
    (
        "narrow_isolated_straight_v2",
        "scenarios/phase2/inc1/narrow_isolated_straight_v2.json5",
    ),
    (
        "narrow_isolated_curve_v2",
        "scenarios/phase2/inc1/narrow_isolated_curve_v2.json5",
    ),
    (
        "narrow_isolated_braking_v2",
        "scenarios/phase2/inc1/narrow_isolated_braking_v2.json5",
    ),
    (
        "narrow_following_v2",
        "scenarios/phase2/inc1/narrow_following_v2.json5",
    ),
    (
        "narrow_signal_v2",
        "scenarios/phase2/inc1/narrow_signal_v2.json5",
    ),
    (
        "narrow_crossing_v2",
        "scenarios/phase2/inc1/narrow_crossing_v2.json5",
    ),
];

/// The checked-in, declared narrow-mode seed bank this suite batches over.
const SEED_BANK: &str = "scenarios/phase2/inc1/narrow_isolated_seed_bank.json";

/// The three Phase 2 fidelity presets a `CC-NARROW` cell is judged at.
const PRESETS: [(&str, f64); 3] = [("Fast", 0.1), ("Standard", 0.05), ("Fine", 0.02)];

/// Fixed seed the increment's fixtures record their trace hash at.
const SEED: u64 = 20_260_913;

/// A seed that is not banked, used only to prove a trace hash is not constant.
const OTHER_SEED: u64 = SEED + 1;

/// 30 s at the default step: long enough for both facilities to admit agents,
/// short enough that the whole suite stays a unit test.
const TICKS: u64 = 600;

/// A scratch working directory removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-narrow-determinism-{name}-{}",
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

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn config(seed: u64, step_s: f64) -> RunConfig {
    RunConfig::new(seed).with_step(Seconds::from_secs(step_s))
}

/// The canonical trace hash of one run of a fixture at one seed and step.
fn trace_hash(path: &str, seed: u64, step_s: f64) -> String {
    let (scenario, _) = load_scenario_provenance(&repo_path(path)).expect("the fixture loads");
    let trace = canonical_trace(scenario, config(seed, step_s), TICKS)
        .unwrap_or_else(|error| panic!("'{path}' failed to run: {error}"));
    let spawned = std::str::from_utf8(trace.bytes())
        .expect("a trace is UTF-8")
        .lines()
        .filter(|line| line.contains("\"event\":\"spawned\""))
        .count();
    assert!(
        spawned > 0,
        "'{path}' produced no spawn event, so its trace hash proves nothing"
    );
    trace.hash().to_owned()
}

/// Every checked-in narrow fixture runs twice at one fixed seed and produces one
/// stable trace hash per preset.
#[test]
#[ignore = "slow: every narrow fixture at every preset; run scripts/run-test-harness.sh"]
fn each_narrow_fixture_reproduces_its_trace_hash_at_every_preset() {
    for (id, path) in FIXTURES {
        for (preset, step_s) in PRESETS {
            let first = trace_hash(path, SEED, step_s);
            let second = trace_hash(path, SEED, step_s);
            assert_eq!(
                first, second,
                "{id} [{preset}]: the same seed and step produced two trace hashes"
            );
            println!("{id} [{preset}] {first}");
        }
        // The hash is a function of the seed: a banked run is not reproduced by
        // every seed, so the determinism above is not a constant.
        let standard = trace_hash(path, SEED, 0.05);
        let other = trace_hash(path, OTHER_SEED, 0.05);
        assert_ne!(
            standard, other,
            "{id}: two different seeds produced one trace hash"
        );
    }
}

/// The batch's per-seed trace hash equals the canonical trace hash of the same
/// run, so the manifest links to the run by the canonical hash and not a second
/// scheme.
fn assert_batch_hash_is_canonical(manifest: &BatchManifest, path: &str) {
    let (scenario, _) = load_scenario_provenance(&repo_path(path)).expect("the fixture loads");
    for run in &manifest.runs {
        let expected = canonical_trace(
            scenario.clone(),
            config(run.seed, manifest.spec.step_s),
            manifest.spec.ticks,
        )
        .expect("the fixture runs")
        .hash()
        .to_owned();
        assert_eq!(
            run.trace_sha256, expected,
            "'{path}' seed {}: the batch recorded a hash that is not the canonical trace hash",
            run.seed
        );
    }
}

fn batch_request(
    root: &Path,
    path: &str,
    seeds: &[u64],
    seed_bank: &SeedBankReference,
) -> BatchRequest {
    let (scenario, provenance) =
        load_scenario_provenance(&repo_path(path)).expect("the fixture loads");
    BatchRequest {
        root: root.to_path_buf(),
        scenario,
        provenance,
        ticks: TICKS,
        step_s: 0.05,
        sampling: SamplingPolicy {
            policy_version: hekate_cli::SAMPLING_POLICY_VERSION,
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

/// A batch over the declared seed bank records every per-seed trace hash, pins
/// each run manifest's stream hash to that hash, and cross-checks each against
/// the canonical trace hash of the same run.
#[test]
#[ignore = "slow: seed-bank batch over every narrow fixture; run scripts/run-test-harness.sh"]
fn the_narrow_seed_bank_batch_reproduces_every_per_seed_hash_and_event_stream() {
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

    for (id, path) in FIXTURES {
        let root = scratch.path(id);
        let batch = run_batch(batch_request(&root, path, &seeds, &reference))
            .unwrap_or_else(|error| panic!("{id} batch failed: {error}"));

        assert_eq!(batch.seeds, seeds, "{id}: the batch ran the bank's seeds");
        assert_eq!(
            batch
                .seed_bank
                .as_ref()
                .map(|bank| bank.content_sha256.as_str()),
            Some(loaded.content_sha256.as_str()),
            "{id}: the batch must name the declared bank"
        );
        assert_batch_hash_is_canonical(&batch, path);

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
                "{id} seed {}: the run manifest's stream hash must be the batch's trace hash",
                batch_run.seed
            );
            recorded.push(format!("{}={}", batch_run.seed, batch_run.trace_sha256));
        }
        println!("{id}: {recorded:?}");
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
    let (id, path) = FIXTURES[0];

    let banked = run_batch(batch_request(
        &scratch.path(&format!("{id}-banked")),
        path,
        &seeds,
        &SeedBankReference {
            path: SEED_BANK.to_owned(),
            content_sha256: loaded.content_sha256.clone(),
        },
    ))
    .expect("the banked batch runs");

    let (scenario, provenance) =
        load_scenario_provenance(&repo_path(path)).expect("the fixture loads");
    let explicit = run_batch(BatchRequest {
        root: scratch.path(&format!("{id}-explicit")),
        scenario,
        provenance,
        ticks: TICKS,
        step_s: 0.05,
        sampling: SamplingPolicy {
            policy_version: hekate_cli::SAMPLING_POLICY_VERSION,
            events: EventRetention::All,
            trajectories: TrajectorySampling::off(),
        },
        seeds,
        seed_bank: None,
        jobs: 1,
    })
    .expect("the explicit batch runs");

    assert_batch_hash_is_canonical(&banked, path);
    let stored = read_batch_manifest(&scratch.path(&format!("{id}-banked")));
    let hashes: Vec<&str> = stored
        .runs
        .iter()
        .map(|run| run.trace_sha256.as_str())
        .collect();
    let explicit_hashes: Vec<&str> = explicit
        .runs
        .iter()
        .map(|run| run.trace_sha256.as_str())
        .collect();
    assert_eq!(hashes, explicit_hashes, "the bank must not change the runs");
}

/// The fixtures really spawn narrow agents, so the determinism above is not an
/// artifact of empty runs. The spawn events name both narrow facilities.
#[test]
fn the_narrow_determinism_executes_the_fixtures() {
    for (id, path) in FIXTURES {
        let (scenario, _) = load_scenario_provenance(&repo_path(path)).expect("the fixture loads");
        let mut sim = hekate_sim::Simulation::new(scenario, config(SEED, 0.05))
            .expect("the simulation builds");
        let mut spawned = 0usize;
        for _ in 0..TICKS {
            for event in sim.step().events() {
                if matches!(event, Event::Spawned { .. }) {
                    spawned += 1;
                }
            }
        }
        assert!(spawned > 0, "{id}: no agent spawned over {TICKS} ticks");
    }
}
