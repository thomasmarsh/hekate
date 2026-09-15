//! Increment 3: an admitted bus or rigid truck drives with its **own**
//! dimensions and dynamics, not the passenger car's.
//!
//! `scenarios/phase2/inc3/heavy_isolated_v2.json5` authors three wheeled box
//! mode templates on one straight corridor — `passenger_car`, `bus`, and
//! `rigid_truck` — with distinct constant body lengths and widths and a heavy
//! longitudinal envelope for each heavy mode (slower desired speed, lower
//! maximum acceleration, lower comfortable braking, larger time gap). Every
//! range is constant (`min == max`), so a realized body dimension identifies the
//! template that spawned the agent and a sampled dynamic is the authored value.
//!
//! The kernel dispatches a demand source to a wheeled box's own compiled
//! template by its [`AgentFamily::WheeledBox`] family. Before this slice every
//! non-capsule box sampled the version-1 passenger-car view
//! (`Simulation::try_admit`), which `CompiledScenario`'s `v2_to_v1_view` derives
//! from the `passenger_car` template alone, so all three modes realized 4.5 m by
//! 1.8 m and the car's dynamics. This suite holds the realized bodies and
//! sampled profiles to the compiled templates, so a bus or a rigid truck that
//! fell back to the car profile fails rather than passing on a plausible run:
//! the run must admit **all three** templates at their own dimensions.
//!
//! The drive is also bounded: no speed leaves `[0, desired speed]`, and no two
//! bodies on the shared path overlap. Both are properties of the shared
//! longitudinal stages the heavy modes reuse unchanged — this slice authors no
//! new dynamics law — so a violation is a kernel regression, not a heavy-mode
//! tolerance.

use std::collections::{BTreeMap, BTreeSet};

use hekate_model::{
    AgentBody, AgentFamily, CompiledScenario, ProfileRange, parse_scenario_source_v2,
};
use hekate_sim::{AgentMode, RunConfig, Simulation, Snapshot, SnapshotDetail};

/// The Increment 3 heavy isolated straight fixture, compiled into this test.
const FIXTURE: &str = include_str!("../../../scenarios/phase2/inc3/heavy_isolated_v2.json5");

/// The fixture's scenario id, which its demand and templates hang off.
const FIXTURE_ID: &str = "heavy_isolated_v2";

/// The fixture's declared root seed. Every fixture runs at root seed 0.
const SEED: u64 = 0;

/// Whole Standard steps the checked run advances. Long enough that each of the
/// three demand sources admits an agent (the slowest source's first arrival is
/// well inside this horizon) and short enough that no agent completes the
/// 600 m corridor, so the assertions observe every admitted mode live.
const TICKS: u64 = 400;

/// Tolerance for an exact match between a realized value and its compiled
/// template constant.
const TOL: f64 = 1e-9;

/// Tolerance for a speed bound, which integration can leave a rounding error
/// inside.
const SPEED_TOL: f64 = 1e-6;

/// One wheeled box mode template of the fixture, as the compiled scenario
/// carries it. The expected values are read from the scenario rather than
/// restated here, so a fixture edit moves the assertion with it.
struct HeavyMode {
    id: String,
    length_m: f64,
    width_m: f64,
    speed_mps: ProfileRange,
    max_accel_mps2: ProfileRange,
    comfortable_brake_mps2: ProfileRange,
    time_gap_s: ProfileRange,
}

impl HeavyMode {
    /// Whether every dynamic of `profile` lies inside this template's authored
    /// ranges.
    fn contains(&self, profile: hekate_sim::VehicleProfile) -> Vec<String> {
        let mut violations = Vec::new();
        let mut check = |name: &str, value: f64, range: ProfileRange| {
            if value < range.min() - TOL || value > range.max() + TOL {
                violations.push(format!(
                    "{}: sampled {name} {value} is outside its authored range {}–{}",
                    self.id,
                    range.min(),
                    range.max()
                ));
            }
        };
        check("desired speed", profile.desired_speed_mps, self.speed_mps);
        check("time gap", profile.time_gap_s, self.time_gap_s);
        check(
            "maximum acceleration",
            profile.max_accel_mps2,
            self.max_accel_mps2,
        );
        check(
            "comfortable braking",
            profile.comfortable_brake_mps2,
            self.comfortable_brake_mps2,
        );
        if !(0.0..=1.0).contains(&profile.compliance) {
            violations.push(format!(
                "{}: sampled compliance {} left [0, 1]",
                self.id, profile.compliance
            ));
        }
        violations
    }
}

/// Parse and compile the fixture.
fn compiled() -> CompiledScenario {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    CompiledScenario::compile_v2(source).expect("the fixture compiles")
}

/// Every wheeled box mode template the fixture authors, in declared order.
///
/// Each is asserted to carry the wheeled box family and a box body, so the
/// fixture proves the box spawn path rather than a template that would take the
/// capsule one.
fn heavy_modes(scenario: &CompiledScenario) -> Vec<HeavyMode> {
    scenario
        .mode_templates()
        .iter()
        .map(|template| {
            assert_eq!(
                template.family(),
                Some(AgentFamily::WheeledBox),
                "{}: the heavy fixture authors wheeled box templates",
                template.id()
            );
            let AgentBody::Box { length_m, width_m } = template.body() else {
                panic!("{}: the heavy fixture authors box bodies", template.id());
            };
            let profile = template.profile();
            HeavyMode {
                id: template.id().to_owned(),
                length_m: length_m.min(),
                width_m: width_m.min(),
                speed_mps: profile.desired_speed_mps(),
                max_accel_mps2: profile
                    .max_accel_mps2()
                    .expect("a wheeled box declares maximum acceleration"),
                comfortable_brake_mps2: profile
                    .comfortable_brake_mps2()
                    .expect("a wheeled box declares comfortable braking"),
                time_gap_s: profile
                    .time_gap_s()
                    .expect("a wheeled box declares a time gap"),
            }
        })
        .collect()
}

/// The one mode template whose authored body length is `body_length_m`.
fn mode_of(modes: &[HeavyMode], body_length_m: f64) -> Option<&HeavyMode> {
    let mut matches = modes
        .iter()
        .filter(|mode| (mode.length_m - body_length_m).abs() <= TOL);
    let mode = matches.next()?;
    assert!(
        matches.next().is_none(),
        "body length {body_length_m} m identifies more than one authored template, so the \
         dimensions do not identify the spawning template"
    );
    Some(mode)
}

/// Whether `values` holds a value within tolerance of `expected`.
fn contains_value(values: &[f64], expected: f64) -> bool {
    values.iter().any(|value| (value - expected).abs() <= TOL)
}

/// The distinct values `values` realized, for a failure message that names what
/// the run produced instead of repeating every tick's sample.
fn distinct(values: &[f64]) -> Vec<f64> {
    let mut keys: Vec<i64> = values
        .iter()
        .map(|value| (value * 1e6).round() as i64)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter().map(|key| key as f64 / 1e6).collect()
}

/// One observed vehicle: the template its body dimensions identify and the
/// values the run realized for it.
struct Observation {
    mode: String,
    speed_mps: f64,
    desired_speed_mps: f64,
}

/// Run the fixture, holding every live vehicle to its own compiled template and
/// to the bounded-drive invariants on every tick.
///
/// Returns every observed vehicle and the distinct body lengths and widths the
/// run realized.
fn run_and_observe(
    scenario: CompiledScenario,
    modes: &[HeavyMode],
) -> (Vec<Observation>, Vec<f64>, Vec<f64>) {
    let mut sim = Simulation::new(scenario, RunConfig::new(SEED)).expect("the simulation builds");
    let mut observations = Vec::new();
    let mut lengths = Vec::new();
    let mut widths = Vec::new();
    let mut violations = Vec::new();
    for tick in 0..TICKS {
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        for sample in snapshot.agents() {
            let motion = sample.motion.as_ref().expect("a full snapshot");
            if motion.mode != AgentMode::Vehicle {
                continue;
            }
            if let Some(profile) = motion.profile {
                let mode = mode_of(modes, motion.body_length_m);
                let Some(mode) = mode else {
                    violations.push(format!(
                        "tick {tick}: agent {} realized body length {} m, which no authored \
                         template declares",
                        sample.id.get(),
                        motion.body_length_m
                    ));
                    continue;
                };
                violations.extend(
                    mode.contains(profile)
                        .into_iter()
                        .map(|violation| format!("tick {tick}: {violation}")),
                );
                if (motion.body_width_m - mode.width_m).abs() > TOL {
                    violations.push(format!(
                        "tick {tick}: agent {} of mode '{}' realized body width {} m, not the \
                         template's {} m",
                        sample.id.get(),
                        mode.id,
                        motion.body_width_m,
                        mode.width_m
                    ));
                }
                if motion.speed_mps < -SPEED_TOL {
                    violations.push(format!(
                        "tick {tick}: agent {} of mode '{}' drove at {} m/s, which is negative",
                        sample.id.get(),
                        mode.id,
                        motion.speed_mps
                    ));
                }
                if motion.speed_mps > profile.desired_speed_mps + SPEED_TOL {
                    violations.push(format!(
                        "tick {tick}: agent {} of mode '{}' drove at {} m/s, above its desired \
                         speed {} m/s",
                        sample.id.get(),
                        mode.id,
                        motion.speed_mps,
                        profile.desired_speed_mps
                    ));
                }
                lengths.push(motion.body_length_m);
                widths.push(motion.body_width_m);
                observations.push(Observation {
                    mode: mode.id.clone(),
                    speed_mps: motion.speed_mps,
                    desired_speed_mps: profile.desired_speed_mps,
                });
            }
        }
        violations.extend(overlaps(&snapshot, tick));
    }
    assert!(
        violations.is_empty(),
        "the heavy straight run violated its template or bounded-drive invariants:\n{}",
        violations.join("\n")
    );
    (observations, lengths, widths)
}

/// Every same-path pair of live vehicles whose bodies overlap at this tick.
fn overlaps(snapshot: &Snapshot, tick: u64) -> Vec<String> {
    let mut by_path: BTreeMap<usize, Vec<(u32, f64, f64)>> = BTreeMap::new();
    for sample in snapshot.agents() {
        let motion = sample.motion.as_ref().expect("a full snapshot");
        if motion.mode != AgentMode::Vehicle {
            continue;
        }
        by_path.entry(motion.path.index()).or_default().push((
            sample.id.get(),
            motion.path_distance_m,
            motion.body_length_m,
        ));
    }
    let mut violations = Vec::new();
    for (_, mut bodies) in by_path {
        bodies.sort_by(|a, b| a.1.total_cmp(&b.1));
        for pair in bodies.windows(2) {
            let (first, second) = (&pair[0], &pair[1]);
            let separation = second.1 - first.1;
            let contact = (first.2 + second.2) * 0.5;
            if separation < contact - SPEED_TOL {
                violations.push(format!(
                    "tick {tick}: agents {} and {} on one path are {separation} m apart, inside \
                     the {contact} m their bodies occupy",
                    first.0, second.0
                ));
            }
        }
    }
    violations
}

/// Every heavy mode template is admitted, and every admitted agent's body
/// dimensions and sampled dynamics come from its own template.
#[test]
fn every_heavy_mode_is_admitted_with_its_own_body_and_dynamics() {
    let scenario = compiled();
    let modes = heavy_modes(&scenario);
    assert_eq!(scenario.id(), FIXTURE_ID);
    assert_eq!(
        modes
            .iter()
            .map(|mode| mode.id.as_str())
            .collect::<Vec<_>>(),
        ["passenger_car", "bus", "rigid_truck"],
        "the fixture authors the car and the two heavy modes"
    );
    // The dimensions identify a template only if they are distinct, which is
    // what lets the observation below attribute an agent to a mode at all.
    for (index, mode) in modes.iter().enumerate() {
        for other in &modes[index + 1..] {
            assert!(
                (mode.length_m - other.length_m).abs() > TOL,
                "'{}' and '{}' share a body length",
                mode.id,
                other.id
            );
            assert!(
                (mode.width_m - other.width_m).abs() > TOL,
                "'{}' and '{}' share a body width",
                mode.id,
                other.id
            );
        }
    }

    let (observations, lengths, widths) = run_and_observe(scenario, &modes);
    assert!(
        !observations.is_empty(),
        "the run admitted no vehicle, so it proves nothing"
    );

    // The set of realized body dimensions covers every authored template: this
    // is the per-template dispatch. Before the slice every box mode realized the
    // passenger car's 4.5 m by 1.8 m, so the bus and rigid-truck lengths never
    // appeared.
    let seen: BTreeSet<&str> = observations
        .iter()
        .map(|observation| observation.mode.as_str())
        .collect();
    for mode in &modes {
        assert!(
            contains_value(&lengths, mode.length_m),
            "no admitted vehicle realized mode '{}' body length {} m; the run realized {:?}",
            mode.id,
            mode.length_m,
            distinct(&lengths)
        );
        assert!(
            contains_value(&widths, mode.width_m),
            "no admitted vehicle realized mode '{}' body width {} m; the run realized {:?}",
            mode.id,
            mode.width_m,
            distinct(&widths)
        );
        assert!(
            seen.contains(mode.id.as_str()),
            "the run never admitted mode '{}'; it admitted {seen:?}",
            mode.id
        );
    }

    // A heavy envelope is a distinct one: the two heavy modes are slower than
    // the car, so their realized free speeds differ from its authored speed and
    // from each other.
    let car = modes
        .iter()
        .find(|mode| mode.id == "passenger_car")
        .expect("the fixture authors the car");
    for mode in modes.iter().filter(|mode| mode.id != "passenger_car") {
        assert!(
            mode.speed_mps.max() < car.speed_mps.min(),
            "mode '{}' must author a desired speed below the car's",
            mode.id
        );
        assert!(
            mode.max_accel_mps2.max() < car.max_accel_mps2.min(),
            "mode '{}' must author a lower maximum acceleration than the car",
            mode.id
        );
        assert!(
            mode.comfortable_brake_mps2.max() < car.comfortable_brake_mps2.min(),
            "mode '{}' must author lower comfortable braking than the car",
            mode.id
        );
        assert!(
            mode.time_gap_s.min() > car.time_gap_s.max(),
            "mode '{}' must author a larger time gap than the car",
            mode.id
        );
        assert!(
            observations
                .iter()
                .any(|observation| observation.mode == mode.id
                    && (observation.desired_speed_mps - mode.speed_mps.min()).abs() <= TOL),
            "no admitted agent of mode '{}' sampled the template's own desired speed",
            mode.id
        );
    }

    for mode in &modes {
        let sampled: Vec<f64> = observations
            .iter()
            .filter(|observation| observation.mode == mode.id)
            .map(|observation| observation.speed_mps)
            .collect();
        let low = sampled.iter().copied().fold(f64::INFINITY, f64::min);
        let high = sampled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!(
            "{}: {} vehicle ticks, body {} x {} m, realized speeds {low}–{high} m/s",
            mode.id,
            sampled.len(),
            mode.length_m,
            mode.width_m,
        );
    }
}
