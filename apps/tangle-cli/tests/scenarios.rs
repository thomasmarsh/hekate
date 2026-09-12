//! Every checked-in scenario loads, validates, and compiles.
//!
//! Phase 1 Increment 1 gate: the benchmark layouts must be expressible through
//! general primitives with no scenario kind. Structural compilation of each
//! benchmark is the observable contract here; Increment 2 adds the demand that
//! makes a benchmark runnable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tangle_cli::load_scenario;
use tangle_sim::{AgentId, Event, RunConfig, Simulation, SnapshotDetail};

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn json5_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("scenario directory is readable") {
        let entry = entry.expect("directory entry is readable");
        let path = entry.path();
        if path.is_dir() {
            json5_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "json5") {
            out.push(path);
        }
    }
}

#[test]
fn every_checked_in_scenario_loads() {
    let root = repo_path("scenarios");
    let mut files = Vec::new();
    json5_files(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no scenarios found under {}",
        root.display()
    );
    for file in &files {
        load_scenario(file).unwrap_or_else(|error| {
            panic!("scenario '{}' failed to load: {error}", file.display())
        });
    }
}

#[test]
fn benchmark_layouts_compile_from_general_primitives() {
    for name in [
        "straight_approach_v1",
        "perpendicular_conflict_v1",
        "four_leg_signal_v1",
        "car_following_v1",
    ] {
        let path = repo_path(&format!("scenarios/benchmarks/{name}.json5"));
        let scenario = load_scenario(&path)
            .unwrap_or_else(|error| panic!("benchmark '{name}' failed to load: {error}"));
        assert_eq!(scenario.id(), name);
        assert!(
            !scenario.paths().is_empty(),
            "benchmark '{name}' has no guide path"
        );
        // Every compiled primitive exposes its authored name in the id map.
        assert_eq!(
            scenario.boundaries().len(),
            scenario.id_map().boundaries().len()
        );
        assert_eq!(scenario.regions().len(), scenario.id_map().regions().len());
        assert_eq!(
            scenario.movements().len(),
            scenario.id_map().movements().len()
        );
        assert_eq!(
            scenario.crossings().len(),
            scenario.id_map().crossings().len()
        );
        assert_eq!(
            scenario.conflict_regions().len(),
            scenario.id_map().conflict_regions().len()
        );
        assert_eq!(scenario.rules().len(), scenario.id_map().rules().len());
        assert_eq!(scenario.signals().len(), scenario.id_map().signals().len());
        assert_eq!(scenario.demand().len(), scenario.id_map().demand().len());
        assert!(
            !scenario.demand().is_empty(),
            "benchmark '{name}' has no portal demand"
        );
        for demand in scenario.demand() {
            assert!(!demand.routes().is_empty());
            assert!(demand.rate_vph() > 0.0);
        }
        assert!(
            scenario
                .boundaries()
                .iter()
                .all(|boundary| boundary.polygon().area() > 0.0),
            "benchmark '{name}' has a degenerate boundary"
        );
    }
}

#[test]
fn four_leg_benchmark_exposes_its_signal_and_endpoints() {
    let path = repo_path("scenarios/benchmarks/four_leg_signal_v1.json5");
    let scenario = load_scenario(&path).expect("four-leg benchmark loads");

    assert_eq!(scenario.movements().len(), 2);
    assert_eq!(scenario.crossings().len(), 2);
    assert_eq!(scenario.conflict_regions().len(), 1);
    assert_eq!(scenario.movements()[0].name(), "ew_through");
    assert_eq!(scenario.id_map().movements()[0], "ew_through");
    // The authored stop line compiles just upstream of the centre conflict.
    assert!((scenario.movements()[0].stop_line_m() - 34.0).abs() < 1e-9);
    assert!((scenario.movements()[1].stop_line_m() - 34.0).abs() < 1e-9);

    // Entry and exit endpoints derive from the movement's portals.
    let movement = &scenario.movements()[0];
    let entry = scenario
        .portal(movement.from())
        .expect("from portal compiles");
    assert!((movement.entry() - entry.position()).length() < 1e-9);

    let signal = scenario
        .signals()
        .first()
        .expect("four-leg signal compiles");
    assert_eq!(signal.heads().len(), 2);
    assert_eq!(signal.phases().len(), 4);
    assert!((signal.cycle_s() - 58.0).abs() < 1e-9);
}

#[test]
fn benchmark_demand_generates_routed_vehicles() {
    for name in [
        "straight_approach_v1",
        "perpendicular_conflict_v1",
        "four_leg_signal_v1",
        "car_following_v1",
    ] {
        let path = repo_path(&format!("scenarios/benchmarks/{name}.json5"));
        let scenario = load_scenario(&path)
            .unwrap_or_else(|error| panic!("benchmark '{name}' failed to load: {error}"));
        let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("benchmark runs");

        let mut spawned = 0u32;
        for _ in 0..2000 {
            let arrivals: Vec<AgentId> = sim
                .step()
                .events()
                .iter()
                .filter_map(|event| match event {
                    Event::Spawned { agent, .. } => Some(*agent),
                    Event::Despawned { .. } => None,
                })
                .collect();
            for agent in arrivals {
                spawned += 1;
                assert!(
                    sim.agent_route(agent).is_some(),
                    "benchmark '{name}' admitted a vehicle without a route"
                );
                assert!(
                    sim.agent_profile(agent).is_some(),
                    "benchmark '{name}' admitted a vehicle without a profile"
                );
            }
        }
        assert!(
            spawned > 0,
            "benchmark '{name}' produced no demand vehicles"
        );
    }
}

/// Phase 1 Increment 2 gate: every commanded acceleration and speed stays
/// inside the sampled profile bounds, and no two bodies on the corridor
/// overlap while faster followers catch slower leaders.
#[test]
fn car_following_benchmark_obeys_controller_bounds_without_overlap() {
    let path = repo_path("scenarios/benchmarks/car_following_v1.json5");
    let step = 0.05;
    let mut follow_brakes = 0u64;
    for seed in 0..3u64 {
        let scenario = load_scenario(&path).expect("car-following benchmark loads");
        let mut sim = Simulation::new(scenario, RunConfig::new(seed)).expect("benchmark runs");
        let mut previous: HashMap<u32, f64> = HashMap::new();
        for _ in 0..4000 {
            sim.step();
            let snapshot = sim.snapshot(SnapshotDetail::Full);
            let agents = snapshot.agents();
            for (index, sample) in agents.iter().enumerate() {
                let motion = sample.motion.expect("full detail");
                let profile = sim
                    .agent_profile(sample.id)
                    .expect("demand vehicle has a profile");
                assert!(
                    motion.speed_mps >= -1e-9,
                    "seed {seed}: negative speed {}",
                    motion.speed_mps
                );
                assert!(
                    motion.speed_mps <= profile.desired_speed_mps + 1e-9,
                    "seed {seed}: speed {} above desired {}",
                    motion.speed_mps,
                    profile.desired_speed_mps
                );
                if let Some(previous_speed) = previous.get(&sample.id.get()) {
                    let accel = (motion.speed_mps - previous_speed) / step;
                    assert!(
                        accel <= profile.max_accel_mps2 + 1e-6,
                        "seed {seed}: acceleration {accel} above max {}",
                        profile.max_accel_mps2
                    );
                    assert!(
                        accel >= -profile.comfortable_brake_mps2 - 1e-6,
                        "seed {seed}: braking {accel} beyond comfortable {}",
                        profile.comfortable_brake_mps2
                    );
                    if accel < -0.5 {
                        follow_brakes += 1;
                    }
                }
                previous.insert(sample.id.get(), motion.speed_mps);
                for other in &agents[index + 1..] {
                    let other_motion = other.motion.expect("full detail");
                    if other_motion.path != motion.path {
                        continue;
                    }
                    let half_lengths = (motion.body_length_m + other_motion.body_length_m) * 0.5;
                    let gap = (motion.path_distance_m - other_motion.path_distance_m).abs();
                    assert!(
                        gap >= half_lengths - 1e-9,
                        "seed {seed}: bodies overlap (gap {gap} < {half_lengths})"
                    );
                }
            }
        }
    }
    assert!(
        follow_brakes > 0,
        "the car-following benchmark must exercise braking while following"
    );
}
