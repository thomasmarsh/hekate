//! Increment 3: heavy bodies on a constant-radius turn stay inside the lane.
//!
//! `scenarios/phase2/inc3/heavy_turning_v2.json5` authors two concentric
//! quarter-circle guide paths — one for `bus`, one for `rigid_truck` — and each
//! heavy template authors its Increment 3 axle geometry (a wheelbase and a
//! maximum steering angle), so the compiler's turning-limit check reads a
//! genuine geometric radius. The fixture's arcs are inside the limit, so the
//! document validates and the run admits both heavy modes.
//!
//! The lane boundary is the property this suite pins. The kernel's corridor
//! check bounds an agent's *centre* offset; a long body on a curve is a chord of
//! the arc, so its corners are the part that can clip a curb even while the
//! centre is inside. The test projects every body corner onto the facility
//! reference and holds it to the facility's half-width, which is the "bodies
//! never silently clip through a curb" contract at the geometry level the
//! compiler admits.
//!
//! A second case tightens the authored axle geometry below the fixture's
//! curvature and holds the document to the `E_FACILITY_CURVATURE` diagnostic:
//! the same constant-radius turn that is feasible for the authored wheelbase is
//! infeasible for a shorter one, so the wheelbase genuinely participates in the
//! turning limit rather than being inert data.

use glam::DVec2;
use hekate_model::{
    CompiledScenario, DiagnosticCode, ProfileRangeSource, parse_scenario_source_v2, validate_v2,
};
use hekate_sim::{AgentMode, RunConfig, Simulation, SnapshotDetail};

/// The Increment 3 heavy constant-radius turning fixture, compiled into this
/// test.
const FIXTURE: &str = include_str!("../../../scenarios/phase2/inc3/heavy_turning_v2.json5");

/// The fixture's scenario id.
const FIXTURE_ID: &str = "heavy_turning_v2";

/// The fixture's declared root seed.
const SEED: u64 = 0;

/// Whole Standard steps the checked run advances. Long enough to admit several
/// agents of each heavy mode on the quarter-circle arcs and to carry the first
/// of each most of the way around.
const TICKS: u64 = 400;

/// Tolerance for a corner sitting exactly on the declared lane boundary.
const LANE_TOLERANCE_M: f64 = 1e-9;

/// Parse and compile the fixture.
fn compiled() -> CompiledScenario {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    let diagnostics = validate_v2(&source);
    assert!(
        diagnostics.is_empty(),
        "the constant-radius heavy fixture is a valid version-2 document: {diagnostics:?}"
    );
    CompiledScenario::compile_v2(source).expect("the fixture compiles")
}

/// The four world-space corners of a box body at `centre` with `heading_rad`.
fn corners(centre: DVec2, heading_rad: f64, length_m: f64, width_m: f64) -> [DVec2; 4] {
    let forward = DVec2::from_angle(heading_rad);
    let left = DVec2::new(-forward.y, forward.x);
    let half_length = length_m * 0.5;
    let half_width = width_m * 0.5;
    [
        centre + forward * half_length + left * half_width,
        centre + forward * half_length - left * half_width,
        centre - forward * half_length + left * half_width,
        centre - forward * half_length - left * half_width,
    ]
}

/// Run the fixture and hold every heavy body's four corners to its lane.
#[test]
fn every_heavy_body_corner_stays_inside_the_constant_radius_lane() {
    let scenario = compiled();
    assert_eq!(scenario.id(), FIXTURE_ID);
    let mut sim =
        Simulation::new(scenario.clone(), RunConfig::new(SEED)).expect("the simulation builds");

    let mut admitted = std::collections::BTreeSet::new();
    let mut violations = Vec::new();
    let mut observed = 0_u32;
    for tick in 0..TICKS {
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        for sample in snapshot.agents() {
            let motion = sample.motion.as_ref().expect("a full snapshot");
            if motion.mode != AgentMode::Vehicle {
                continue;
            }
            // The mode template's authored body length identifies it; the
            // fixture authors one length per heavy mode.
            let mode = match motion.body_length_m {
                length if (length - 12.0).abs() < 1e-9 => "bus",
                length if (length - 9.5).abs() < 1e-9 => "rigid_truck",
                other => {
                    violations.push(format!(
                        "tick {tick}: agent {} realized body length {other} m, which no heavy \
                         template authors",
                        sample.id.get()
                    ));
                    continue;
                }
            };
            admitted.insert(mode);

            // The facility whose reference path this agent traverses; the
            // fixture permits exactly one heavy mode per facility, so its
            // half-width is the lane the body must stay inside.
            let facility = scenario
                .facilities()
                .iter()
                .find(|facility| facility.reference_path() == Some(motion.path))
                .unwrap_or_else(|| {
                    panic!(
                        "agent {} traverses a path no facility references",
                        sample.id.get()
                    )
                });
            let geometry = facility
                .reference()
                .expect("the fixture's facilities carry a reference path")
                .geometry();
            let half_width = facility.width_m() * 0.5;

            for corner in corners(
                sample.position,
                sample.heading_rad,
                motion.body_length_m,
                motion.body_width_m,
            ) {
                let offset = geometry.project(corner).d().abs();
                if offset > half_width + LANE_TOLERANCE_M {
                    violations.push(format!(
                        "tick {tick}: a corner of {mode} agent {} is {offset} m from the \
                         centreline, past the facility's {half_width} m half-width",
                        sample.id.get()
                    ));
                }
            }
            observed += 1;
        }
    }

    assert!(
        observed > 0,
        "the run admitted no vehicle, so the lane bound proves nothing"
    );
    assert!(
        admitted.contains("bus") && admitted.contains("rigid_truck"),
        "the run admitted only {admitted:?}; the fixture exercises both heavy modes"
    );
    assert!(
        violations.is_empty(),
        "a heavy body left its constant-radius lane:\n{}",
        violations.join("\n")
    );
}

/// A shorter wheelbase makes the same fixture curve infeasible, so validation
/// rejects the document with the turning-limit diagnostic.
#[test]
fn a_tighter_authored_turning_limit_rejects_the_fixture_curve() {
    let mut source =
        parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    assert!(
        validate_v2(&source).is_empty(),
        "the fixture curve is within the authored turning limits"
    );

    // A longer wheelbase makes the vehicle's minimum turning radius larger, so
    // the 40 m fixture arc no longer fits the resulting turning limit.
    for template in &mut source.mode_templates {
        if template.id == "bus" {
            template.wheelbase_m = Some(ProfileRangeSource {
                min: 20.0,
                max: 20.0,
            });
        }
    }

    let diagnostics = validate_v2(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::FacilityCurvature),
        "a wheelbase with a smaller turning radius must reject the fixture curve, got \
         {diagnostics:?}"
    );
}
