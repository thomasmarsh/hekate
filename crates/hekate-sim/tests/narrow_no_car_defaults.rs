//! TAS-079: every checked-in `narrow_isolated_*` fixture is stream-isolated
//! from the Phase-1 passenger-car profile.
//!
//! `PHASE_2_PLAN.md` *Increment 1* gates the increment on "each mode passes
//! independent fixtures without relying on car-specific dimensions or
//! controller defaults". The kernel dispatches a demand source to the narrow
//! path by the compiled [`AgentFamily`] of its mode template
//! (`AgentFamily::WheeledCapsule`, a capsule that steers); a template that is
//! not that family falls back to the Phase-1 passenger-car `profiles`
//! distribution that `scenario.profiles()` carries. So a narrow fixture whose
//! capsule template was ignored — or whose capsule template was replaced by the
//! car profile — would realize the Phase-1 passenger-car constants (body
//! length 4.0–5.2 m, width 1.7–2.0 m, speed 9.0–15.0 m/s) instead of its
//! authored capsule body and limits.
//!
//! This suite holds each live agent on a narrow fixture's facility to its
//! compiled mode template — the realized capsule body, desired speed,
//! acceleration, braking, steering rate, and lateral clearance must all be the
//! template's sampled values — and flags any agent whose realized values came
//! from the Phase-1 passenger-car profile instead. The second test is the
//! falsification probe: it constructs exactly that substitution through the
//! kernel and proves the check detects it, so the guard is not vacuous.

use std::collections::BTreeMap;

use hekate_model::{
    AgentBody, AgentFamily, BodyKind, CompiledModeTemplate, CompiledScenario, ModeTemplateId,
    PathId, parse_scenario_source_v2,
};
use hekate_sim::{AgentId, RunConfig, Simulation, Snapshot, SnapshotDetail};

/// The checked-in Increment 1 narrow isolated fixtures, by id.
const FIXTURES: [(&str, &str); 6] = [
    (
        "narrow_isolated_straight_v2",
        include_str!("../../../scenarios/phase2/inc1/narrow_isolated_straight_v2.json5"),
    ),
    (
        "narrow_isolated_curve_v2",
        include_str!("../../../scenarios/phase2/inc1/narrow_isolated_curve_v2.json5"),
    ),
    (
        "narrow_isolated_braking_v2",
        include_str!("../../../scenarios/phase2/inc1/narrow_isolated_braking_v2.json5"),
    ),
    (
        "narrow_following_v2",
        include_str!("../../../scenarios/phase2/inc1/narrow_following_v2.json5"),
    ),
    (
        "narrow_signal_v2",
        include_str!("../../../scenarios/phase2/inc1/narrow_signal_v2.json5"),
    ),
    (
        "narrow_crossing_v2",
        include_str!("../../../scenarios/phase2/inc1/narrow_crossing_v2.json5"),
    ),
];

/// The fixed seed the fixture's demand schedule is generated at.
const SEED: u64 = 20_260_913;

/// Long enough that both facilities admit several agents, so the check observes
/// live narrow bodies rather than an empty world.
const TICKS: u64 = 600;

/// Tolerance for an exact match between a realized value and its compiled
/// template constant: the fixtures author constant (`min == max`) ranges, so any
/// drift is the substitution this guard forbids.
const TOL: f64 = 1e-12;

/// Parse and compile one fixture.
fn compile(fixture: &str) -> CompiledScenario {
    let source = parse_scenario_source_v2(fixture).expect("the fixture is a version-2 document");
    CompiledScenario::compile_v2(source).expect("the fixture compiles")
}

/// The authored narrow template each facility path serves, read from the
/// compiled facilities rather than inferred from a body size or mode name.
fn path_modes(scenario: &CompiledScenario) -> BTreeMap<PathId, ModeTemplateId> {
    let mut modes = BTreeMap::new();
    for facility in scenario.facilities() {
        let path = facility
            .reference_path()
            .expect("every fixture facility carries a reference path");
        for &mode in facility.access() {
            modes.insert(path, mode);
        }
    }
    assert_eq!(
        modes.len(),
        scenario.facilities().len(),
        "every fixture facility carries exactly one mode"
    );
    modes
}

/// Whether `value` lies in the inclusive Phase-1 passenger-car band `range`.
fn in_band(value: f64, range: hekate_model::ProfileRange) -> bool {
    value >= range.min() - TOL && value <= range.max() + TOL
}

/// The body length and width a compiled template reports for an agent. A
/// capsule's width is twice its radius; a box carries an explicit width.
fn template_body(template: &CompiledModeTemplate) -> (f64, f64) {
    match template.body() {
        AgentBody::Capsule { length_m, radius_m } => (length_m.min(), radius_m.min() * 2.0),
        AgentBody::Box { length_m, width_m } => (length_m.min(), width_m.min()),
        other => panic!(
            "the narrow fixtures authorize box or capsule templates, got a {} body",
            other.kind().label()
        ),
    }
}

/// Every way a live agent on a narrow fixture's facility could be drawing a
/// dimension or limit from the Phase-1 passenger-car profile instead of its
/// compiled narrow template.
///
/// The check is behavioral: it compares the values the kernel realized to the
/// values the facility's own compiled mode template authored, and to the
/// passenger-car band `scenario.profiles()` carries. An empty result means
/// every agent's body and limits came from its template.
fn car_default_violations(
    scenario: &CompiledScenario,
    sim: &Simulation,
    snapshot: &Snapshot,
) -> Vec<String> {
    let car = scenario.profiles();
    let modes = path_modes(scenario);
    let mut violations = Vec::new();
    for sample in snapshot.agents() {
        let Some(motion) = sample.motion.as_ref() else {
            continue;
        };
        let Some(&mode) = modes.get(&motion.path) else {
            continue;
        };
        let template = scenario
            .mode_template(mode)
            .expect("a facility's mode compiles");
        let who = format!("agent {} on mode '{}'", sample.id.get(), template.id());

        if template.family() != Some(AgentFamily::WheeledCapsule) {
            violations.push(format!(
                "{who}: the facility's mode does not compile to the narrow wheeled capsule family, \
                 so its agent cannot take the narrow path and falls back to the Phase-1 \
                 passenger-car profile"
            ));
        }
        let Some(narrow) = sim.agent_narrow_profile(sample.id) else {
            violations.push(format!(
                "{who}: no narrow profile; the agent's body and limits were sampled from the \
                 Phase-1 passenger-car profile rather than a narrow template"
            ));
            continue;
        };
        let Some(profile) = sim.agent_profile(sample.id) else {
            violations.push(format!("{who}: no sampled vehicle profile"));
            continue;
        };

        let (length_m, width_m) = template_body(template);
        if motion.body_kind != BodyKind::Capsule {
            violations.push(format!(
                "{who}: realized a {} body, not the template's capsule",
                motion.body_kind.label()
            ));
        }
        if (motion.body_length_m - length_m).abs() > TOL {
            violations.push(format!(
                "{who}: realized body length {} m is not the template's {length_m} m (the \
                 Phase-1 passenger-car band is {}–{} m)",
                motion.body_length_m,
                car.length_m().min(),
                car.length_m().max()
            ));
        }
        if (motion.body_width_m - width_m).abs() > TOL {
            violations.push(format!(
                "{who}: realized body width {} m is not the template's {width_m} m",
                motion.body_width_m
            ));
        }

        let desired = template.profile().desired_speed_mps().min();
        if (profile.desired_speed_mps - desired).abs() > TOL {
            violations.push(format!(
                "{who}: realized desired speed {} m/s is not the template's {desired} m/s (the \
                 Phase-1 passenger-car band is {}–{} m/s)",
                profile.desired_speed_mps,
                car.speed_mps().min(),
                car.speed_mps().max()
            ));
        }
        let accel = template
            .profile()
            .max_accel_mps2()
            .expect("a narrow wheeled template carries acceleration")
            .min();
        if (profile.max_accel_mps2 - accel).abs() > TOL {
            violations.push(format!(
                "{who}: realized max acceleration {} m/s^2 is not the template's {accel}",
                profile.max_accel_mps2
            ));
        }
        let brake = template
            .profile()
            .comfortable_brake_mps2()
            .expect("a narrow wheeled template carries braking")
            .min();
        if (profile.comfortable_brake_mps2 - brake).abs() > TOL {
            violations.push(format!(
                "{who}: realized comfortable braking {} m/s^2 is not the template's {brake}",
                profile.comfortable_brake_mps2
            ));
        }
        let steering = template
            .profile()
            .steering_rate_max_rad_s()
            .expect("a narrow wheeled template carries a steering rate")
            .min();
        if (narrow.steering_rate_max_rad_s - steering).abs() > TOL {
            violations.push(format!(
                "{who}: realized steering rate {} rad/s is not the template's {steering}",
                narrow.steering_rate_max_rad_s
            ));
        }
        let clearance = template
            .profile()
            .lateral_clearance_m()
            .expect("a narrow wheeled template carries a lateral clearance")
            .min();
        if (narrow.lateral_clearance_m - clearance).abs() > TOL {
            violations.push(format!(
                "{who}: realized lateral clearance {} m is not the template's {clearance}",
                narrow.lateral_clearance_m
            ));
        }
    }
    violations
}

/// Run a scenario for [`TICKS`] steps and return the live agents the check
/// observes in the final snapshot, together with the violations it reports.
fn run_and_check(scenario: CompiledScenario) -> (Vec<AgentId>, Vec<String>) {
    let mut sim = Simulation::new(scenario, RunConfig::new(SEED)).expect("the simulation builds");
    for _ in 0..TICKS {
        sim.step();
    }
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let live: Vec<AgentId> = snapshot.agents().iter().map(|sample| sample.id).collect();
    let violations = car_default_violations(sim.scenario(), &sim, &snapshot);
    (live, violations)
}

/// The increment's narrow fixtures realize their compiled template on every
/// live agent: no dimension or limit comes from the Phase-1 passenger-car
/// profile.
#[test]
fn narrow_fixtures_realize_their_compiled_template_with_no_passenger_car_defaults() {
    for (id, fixture) in FIXTURES {
        let scenario = compile(fixture);
        let narrow_templates = scenario
            .mode_templates()
            .iter()
            .filter(|template| template.family() == Some(AgentFamily::WheeledCapsule))
            .count();
        assert_eq!(
            narrow_templates, 2,
            "{id} authors the bicycle and scooter narrow templates"
        );

        let (live, violations) = run_and_check(scenario);
        assert!(
            !live.is_empty(),
            "{id}: the check observed no live agent, so it proves nothing"
        );
        assert!(
            violations.is_empty(),
            "{id}: a narrow agent drew a dimension or limit from the Phase-1 passenger-car profile"
        );
    }
}

/// The falsification probe: construct a narrow fixture facility whose mode
/// cannot take the narrow path, so the kernel samples the Phase-1 passenger-car
/// profile for its agent, and prove the check reports it.
///
/// The probe mode is a box (`AgentFamily::WheeledBox`), like the Increment 0
/// passenger car, so `Simulation::agent_narrow_profile` is `None` and the
/// agent's body and limits come from `scenario.profiles()` — the Phase-1
/// passenger-car constants. Its authored template numbers differ from those
/// constants, so any realized value equal to the car profile is a substitution
/// the check must catch. This is the failure TAS-070 constructs against the
/// no-mode-branch guard, expressed behaviorally.
const PROBE: &str = r#"{
  schema_version: 2,
  id: 'narrow_no_car_defaults_probe',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.0 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.0 },
  ],
  regions: [
    { id: 'band', points: [
      { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
      { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
    ] },
  ],
  facilities: [
    { id: 'lane', region: 'band', reference_path: 'guide', width_m: 3.0,
      nominal_direction: 'forward', access: { modes: [ 'narrow_probe' ] },
      lateral_use: 'shared', speed_policy: { limit_mps: null } },
  ],
  movements: [
    { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
      direction: 'forward' },
  ],
  mode_templates: [
    {
      id: 'narrow_probe',
      body: { kind: 'box', length_m: { min: 1.8, max: 1.8 },
        width_m: { min: 0.7, max: 0.7 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 5.0, max: 5.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        compliance: { min: 1.0, max: 1.0 },
      },
    },
  ],
  permissions: [],
  demand: [
    { id: 'inflow', mode: 'narrow_probe',
      spawn: { rate: {
        portal: 'entry',
        rate_per_hour: 240.0,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
      } } },
  ],
}"#;

#[test]
fn the_no_car_defaults_check_flags_a_narrow_facility_whose_agent_uses_the_car_profile() {
    let scenario = compile(PROBE);
    // The probe's authored template is not the car profile, so a realized value
    // equal to the car profile is the substitution, not the authoring.
    let template = scenario
        .mode_templates()
        .iter()
        .find(|template| template.id() == "narrow_probe")
        .expect("the probe authors its mode");
    assert_ne!(template.family(), Some(AgentFamily::WheeledCapsule));

    let mut sim =
        Simulation::new(scenario.clone(), RunConfig::new(SEED)).expect("the probe builds");
    for _ in 0..TICKS {
        sim.step();
    }
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    assert!(
        !snapshot.agents().is_empty(),
        "the probe admitted no agent, so it constructs no failure"
    );

    // The realized values are the Phase-1 passenger-car constants: the kernel
    // sampled `scenario.profiles()` because the probe's mode is not a narrow
    // capsule. The fixture authors no `profiles`, so the compiled profile is the
    // Phase-1 default band.
    let car = sim.scenario().profiles();
    assert!(
        car.length_m().min() >= 4.0 && car.length_m().max() <= 5.2,
        "the probe must carry the Phase-1 passenger-car body band, got {}–{}",
        car.length_m().min(),
        car.length_m().max()
    );
    let violations = car_default_violations(sim.scenario(), &sim, &snapshot);
    assert!(
        !violations.is_empty(),
        "the check must flag a narrow facility whose agent's body and limits came from the \
         Phase-1 passenger-car profile"
    );
    let joined = violations.join("\n");
    assert!(
        joined.contains("body length") || joined.contains("narrow wheeled capsule"),
        "the reported violation must name the car-default body substitution:\n{joined}"
    );
    assert!(
        joined.contains("no narrow profile"),
        "the reported violation must name the missing narrow template:\n{joined}"
    );
    // Sanity: every realized agent's body really is the car profile band, not
    // the probe's authored 1.8 m template, so the probe constructs the exact
    // failure rather than an unrelated one.
    for sample in snapshot.agents() {
        let motion = sample.motion.as_ref().expect("full detail");
        assert!(
            in_band(motion.body_length_m, car.length_m()),
            "probe agent {} realized body length {} outside the car band",
            sample.id.get(),
            motion.body_length_m
        );
        assert!(
            (motion.body_length_m - 1.8).abs() > TOL,
            "probe agent {} realized the authored template body, not the car profile",
            sample.id.get()
        );
        assert!(sim.agent_narrow_profile(sample.id).is_none());
    }
}
