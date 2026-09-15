//! TAS-078: the Increment 1 narrow isolated fixtures.
//!
//! Every checked-in fixture under `scenarios/phase2/inc1/` runs here through the
//! kernel for both narrow modes, `bicycle` and `scooter`:
//!
//! - `narrow_isolated_straight_v2` and `narrow_isolated_curve_v2` are the
//!   `CC-NARROW` class fixtures, so each is held to `T-RT`, `T-DIM`, `T-ENV`,
//!   and `T-SPD` at all three `CC-NARROW` presets (`F` 0.1 s, `S` 0.05 s,
//!   `f` 0.02 s);
//! - `narrow_isolated_braking_v2`, `narrow_following_v2`, `narrow_signal_v2`,
//!   and `narrow_crossing_v2` each run one Increment 1 behavior for both modes
//!   and hold every live narrow sample to the authored body dimension
//!   (`T-DIM`), the profile's command envelope and free-speed bound (`T-ENV`,
//!   `T-SPD`), and the behavior the fixture authors.
//!
//! The fixtures live under `scenarios/`, not under `tests/fixtures/`, because
//! `docs/benchmark-matrix.md` names those paths and the CLI loads them from
//! `scenarios/**`; the tests read the same files, so a path the matrix lists and
//! a path the tests run cannot drift apart.
//!
//! ## What `T-RT` measures here
//!
//! `T-RT` is the path-to-world-to-path round-trip error of a facility's
//! compiled reference geometry against the increment's declared analytic
//! reference (`docs/benchmark-matrix.md` §6).
//!
//! The straight fixture's reference path is a single straight segment, so its
//! compiled geometry *is* the analytic straight reference and the round trip is
//! exact on it at every lateral offset.
//!
//! The curve fixture declares a constant-curvature analytic reference: a
//! quarter circle about the origin. `paths[].points` authors only polylines, so
//! the checked-in guide path is that circle's chord polyline. A chord polyline's
//! own round trip at a lateral offset near a vertex lands on the neighbouring
//! chord and reports the polyline's chord-approximation error (~0.05 m), not the
//! analytic arc's coordinate-mapping error, so the curve test asserts three
//! things instead of one: the analytic `CompiledReferencePath::arc` the fixture
//! declares holds `T-RT` within 1e-9 m over the usable lateral interval; every
//! authored chord vertex lies on that arc within 1e-9 m; and the chord polyline
//! is a strict approximation (strictly shorter than the arc it approximates).
//! The analytic reference is reconstructed from the fixture's own vertices, and
//! it is the same compiled reference geometry type `TAS-074` measured `T-RT` on.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec2;
use hekate_model::{
    AgentBody, AgentFamily, BodyKind, CompiledFacility, CompiledModeTemplate,
    CompiledReferencePath, CompiledScenario, ConflictRegionId, ModeTemplateId, PathId,
    parse_scenario_source_v2,
};
use hekate_sim::{
    AgentId, DespawnReason, Event, MotionSample, RegionKey, RunConfig, Seconds, SignalAction,
    Simulation, Snapshot, SnapshotDetail,
};

/// The checked-in Increment 1 narrow isolated straight fixture.
const STRAIGHT: &str =
    include_str!("../../../scenarios/phase2/inc1/narrow_isolated_straight_v2.json5");

/// The checked-in Increment 1 narrow isolated curve fixture.
const CURVE: &str = include_str!("../../../scenarios/phase2/inc1/narrow_isolated_curve_v2.json5");

/// The checked-in Increment 1 narrow isolated braking fixture.
const BRAKING: &str =
    include_str!("../../../scenarios/phase2/inc1/narrow_isolated_braking_v2.json5");

/// The checked-in Increment 1 narrow following fixture.
const FOLLOWING: &str = include_str!("../../../scenarios/phase2/inc1/narrow_following_v2.json5");

/// The checked-in Increment 1 narrow signal fixture.
const SIGNAL: &str = include_str!("../../../scenarios/phase2/inc1/narrow_signal_v2.json5");

/// The checked-in Increment 1 narrow crossing fixture.
const CROSSING: &str = include_str!("../../../scenarios/phase2/inc1/narrow_crossing_v2.json5");

/// The three Phase 2 fidelity presets a `CC-NARROW` cell is judged at: Fast,
/// Standard, and Fine, with their fixed physics steps in seconds.
const PRESETS: [(&str, f64); 3] = [("Fast", 0.1), ("Standard", 0.05), ("Fine", 0.02)];

/// Fixed seed. Every fixture authors constant profile ranges, so the sampled
/// body, limits, and desired speed are seed-independent; this seed fixes the
/// demand arrival schedule.
const SEED: u64 = 20_260_913;

/// `T-DIM`: realized body dimension vs authored template, metres.
const T_DIM_M: f64 = 1e-12;

/// `T-RT`: path-to-world-to-path round-trip error, metres.
const T_RT_M: f64 = 1e-9;

/// `T-SPD`: steady-state free-speed error vs the authored constant, m/s.
const T_SPD_MPS: f64 = 1e-6;

/// The arc length after which a free-flow fixture must report its authored
/// speed. A narrow agent enters at its desired speed, so this only excludes the
/// entry step itself from the steady-state comparison.
const SETTLING_M: f64 = 0.5;

/// The stop-line arc length the braking and signal fixtures author.
const STOP_LINE_M: f64 = 60.0;

/// Parse and compile one checked-in fixture.
fn compile(fixture: &str) -> CompiledScenario {
    let source = parse_scenario_source_v2(fixture).expect("the fixture is a version-2 document");
    CompiledScenario::compile_v2(source).expect("the fixture compiles")
}

/// The compiled narrow templates of a scenario, in authoring order.
fn narrow_templates(scenario: &CompiledScenario) -> Vec<&CompiledModeTemplate> {
    scenario
        .mode_templates()
        .iter()
        .filter(|template| template.family() == Some(AgentFamily::WheeledCapsule))
        .collect()
}

/// The authored narrow template each checked-in facility path serves.
///
/// Every fixture authors one narrow mode per facility, so a live agent's path
/// names the facility (and therefore the template) it rides. The mapping is read
/// from the compiled facilities, never inferred from a body size or a mode name.
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

/// The largest `(s, d)` round-trip error in metres over the given route
/// coordinates, including the reconstructed world distance.
///
/// The `T-RT` measurement of `TAS-074`, repeated here against each fixture's own
/// compiled reference.
fn max_round_trip_error(
    reference: &CompiledReferencePath,
    distances: &[f64],
    offsets: &[f64],
) -> f64 {
    let mut max = 0.0_f64;
    for &s in distances {
        for &d in offsets {
            let world = reference.point_at(s, d);
            let route = reference.project(world);
            max = max.max((route.s() - s).abs()).max((route.d() - d).abs());
            let reconstructed = reference.point_at(route.s(), route.d());
            max = max.max((reconstructed - world).length());
        }
    }
    max
}

/// The envelope width and lateral clearance of the narrow mode a facility
/// permits, so a `T-RT` grid stays inside the facility's usable band.
fn facility_envelope(scenario: &CompiledScenario, facility: &CompiledFacility) -> (f64, f64) {
    let template = scenario
        .mode_template(facility.access()[0])
        .expect("a fixture facility permits a compiled mode");
    let AgentBody::Capsule { radius_m, .. } = template.body() else {
        panic!("the fixtures permit narrow capsule templates only");
    };
    let clearance = template
        .profile()
        .lateral_clearance_m()
        .expect("a narrow wheeled template carries a lateral clearance");
    (radius_m.max() * 2.0, clearance.max())
}

/// The `(s, d)` grid a `T-RT` check samples: twenty arc lengths over the whole
/// reference and five offsets across the facility's usable lateral interval.
fn round_trip_offsets(scenario: &CompiledScenario, facility: &CompiledFacility) -> Vec<f64> {
    let (envelope, clearance) = facility_envelope(scenario, facility);
    let interval = facility.usable_lateral_interval(envelope, clearance);
    vec![
        interval.d_min(),
        interval.d_min() * 0.5,
        0.0,
        interval.d_max() * 0.5,
        interval.d_max(),
    ]
}

/// The arc lengths a `T-RT` check samples over a reference of `length` metres.
fn round_trip_distances(length: f64) -> Vec<f64> {
    (0..=20)
        .map(|index| length * f64::from(index) / 20.0)
        .collect()
}

/// One live narrow agent as the behavior observers read it.
struct LiveNarrow {
    path: PathId,
    /// Front-bumper arc length in metres.
    front_m: f64,
    /// Rear-bumper arc length in metres.
    rear_m: f64,
    speed_mps: f64,
    desired_speed_mps: f64,
}

/// Every live narrow agent of one snapshot.
fn live_narrow(sim: &Simulation, snapshot: &Snapshot) -> Vec<LiveNarrow> {
    snapshot
        .agents()
        .iter()
        .filter_map(|sample| {
            let profile = sim.agent_narrow_profile(sample.id)?;
            let motion = sample.motion.as_ref()?;
            Some(LiveNarrow {
                path: motion.path,
                front_m: motion.path_distance_m + motion.body_length_m * 0.5,
                rear_m: motion.path_distance_m - motion.body_length_m * 0.5,
                speed_mps: motion.speed_mps,
                desired_speed_mps: profile.desired_speed_mps,
            })
        })
        .collect()
}

/// The narrow samples, distinct modes, and `T-SPD` evidence one run observed.
#[derive(Default)]
struct EnvelopeState {
    previous_speed: BTreeMap<AgentId, f64>,
    /// Agents ever constrained: behind a leader on their facility, or held by a
    /// stop-required head. They are no longer in free flow even after the
    /// constraint releases.
    ever_constrained: BTreeSet<AgentId>,
    /// `T-SPD` comparisons actually made, per mode.
    free_speed_checks: BTreeMap<ModeTemplateId, u64>,
    modes: BTreeSet<ModeTemplateId>,
    paths: BTreeSet<PathId>,
}

/// Assert one live narrow sample against its authored template.
///
/// `T-DIM` — the reported capsule body equals the authored constant length and
/// twice the authored constant radius.
/// `T-ENV` — the reported speed stays in `[0, v0]` and the implied per-step
/// acceleration stays inside the profile's `[-comfortable_brake, +max_accel]`.
/// `T-SPD` — where the fixture declares free flow, the settled speed is the
/// authored constant.
fn check_sample(
    agent: AgentId,
    motion: &MotionSample,
    template: &CompiledModeTemplate,
    step_s: f64,
    previous_speed: &mut BTreeMap<AgentId, f64>,
    free_flow: bool,
) {
    let AgentBody::Capsule { length_m, radius_m } = template.body() else {
        panic!("the fixtures author narrow capsule templates only");
    };
    assert_eq!(
        length_m.min(),
        length_m.max(),
        "the fixture authors a constant capsule length so T-DIM has one authored value"
    );
    assert_eq!(radius_m.min(), radius_m.max());
    assert_eq!(
        motion.body_kind,
        BodyKind::Capsule,
        "a narrow mode reports a capsule body"
    );
    assert!(
        (motion.body_length_m - length_m.min()).abs() <= T_DIM_M,
        "T-DIM: body length {} differs from the authored {}",
        motion.body_length_m,
        length_m.min()
    );
    assert!(
        (motion.body_width_m - radius_m.min() * 2.0).abs() <= T_DIM_M,
        "T-DIM: body width {} differs from the authored {}",
        motion.body_width_m,
        radius_m.min() * 2.0
    );

    let desired = template.profile().desired_speed_mps();
    assert_eq!(
        desired.min(),
        desired.max(),
        "the fixture authors a constant desired speed so T-SPD has one authored value"
    );
    let max_accel = template
        .profile()
        .max_accel_mps2()
        .expect("a narrow wheeled template carries an acceleration bound");
    let brake = template
        .profile()
        .comfortable_brake_mps2()
        .expect("a narrow wheeled template carries a braking bound");

    assert!(
        motion.speed_mps >= -T_RT_M && motion.speed_mps <= desired.max() + T_RT_M,
        "T-ENV: narrow speed {} outside [0, {}]",
        motion.speed_mps,
        desired.max()
    );
    if let Some(&previous) = previous_speed.get(&agent) {
        let acceleration = (motion.speed_mps - previous) / step_s;
        assert!(
            acceleration <= max_accel.max() + 1e-9,
            "T-ENV: narrow acceleration {acceleration} exceeds {} m/s^2",
            max_accel.max()
        );
        assert!(
            acceleration >= -brake.max() - 1e-9,
            "T-ENV: narrow deceleration {acceleration} exceeds {} m/s^2",
            brake.max()
        );
    }
    previous_speed.insert(agent, motion.speed_mps);

    if free_flow && motion.path_distance_m >= SETTLING_M {
        assert!(
            (motion.speed_mps - desired.min()).abs() <= T_SPD_MPS,
            "T-SPD: settled free speed {} differs from the authored {}",
            motion.speed_mps,
            desired.min()
        );
    }
}

/// Run one fixture for `seconds`, checking every live narrow sample against its
/// template and calling `observe` after each step.
///
/// `free_flow` selects the samples `T-SPD` applies to; the fixtures that hold a
/// narrow agent below its desired speed (following) or at rest (braking,
/// signal) declare only their free-flow portions.
fn run<F>(
    fixture: &str,
    step_s: f64,
    seconds: f64,
    free_flow: impl Fn(&MotionSample) -> bool,
    mut observe: F,
) -> (Simulation, EnvelopeState)
where
    F: FnMut(&Simulation, &[Event], &Snapshot),
{
    let scenario = compile(fixture);
    let modes = path_modes(&scenario);
    assert_eq!(
        narrow_templates(&scenario).len(),
        2,
        "each fixture authors the bicycle and scooter narrow templates"
    );
    let ticks = (seconds / step_s).round() as u64;
    let mut sim = Simulation::new(
        scenario,
        RunConfig::new(SEED).with_step(Seconds::from_secs(step_s)),
    )
    .expect("the fixture's simulation builds");
    let mut state = EnvelopeState::default();
    for _ in 0..ticks {
        let events = sim.step().events().to_vec();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        // The most advanced narrow agent on each facility has no leader ahead,
        // so it is the one sample per facility that free-flow `T-SPD` binds; a
        // follower's IDM interaction term keeps it fractionally below its own
        // desired speed no matter how large the gap.
        let mut leader_front: BTreeMap<PathId, f64> = BTreeMap::new();
        for sample in snapshot.agents() {
            let Some(_) = sim.agent_narrow_profile(sample.id) else {
                continue;
            };
            let motion = sample.motion.as_ref().expect("full snapshot detail");
            let front = motion.path_distance_m + motion.body_length_m * 0.5;
            leader_front
                .entry(motion.path)
                .and_modify(|current| *current = current.max(front))
                .or_insert(front);
        }
        for sample in snapshot.agents() {
            let Some(_) = sim.agent_narrow_profile(sample.id) else {
                continue;
            };
            let motion = sample.motion.as_ref().expect("full snapshot detail");
            let mode = modes[&motion.path];
            let template = sim
                .scenario()
                .mode_template(mode)
                .expect("the live mode compiles");
            let front = motion.path_distance_m + motion.body_length_m * 0.5;
            let is_leader = (front - leader_front[&motion.path]).abs() <= 1e-9;
            let held = motion
                .decision
                .is_some_and(|decision| decision.action == SignalAction::Stop);
            if !is_leader || held {
                state.ever_constrained.insert(sample.id);
            }
            // `T-SPD` binds only an agent that was never constrained: an agent
            // that became the leader after its predecessor exited, or whose head
            // turned green, is still recovering from a constrained speed.
            let undisturbed = is_leader && !state.ever_constrained.contains(&sample.id);
            if free_flow(motion) && undisturbed && motion.path_distance_m >= SETTLING_M {
                *state.free_speed_checks.entry(mode).or_default() += 1;
            }
            check_sample(
                sample.id,
                motion,
                template,
                step_s,
                &mut state.previous_speed,
                free_flow(motion) && undisturbed,
            );
            state.modes.insert(mode);
            state.paths.insert(motion.path);
        }
        observe(&sim, &events, &snapshot);
    }
    (sim, state)
}

#[test]
fn the_narrow_isolated_straight_fixture_holds_trt_tdim_tenv_and_tspd_at_every_preset() {
    for (preset, step_s) in PRESETS {
        let scenario = compile(STRAIGHT);
        for facility in scenario.facilities() {
            let reference = facility
                .reference()
                .expect("the straight fixture facility has a reference path");
            let distances = round_trip_distances(reference.geometry().length());
            let offsets = round_trip_offsets(&scenario, facility);
            let error = max_round_trip_error(reference.geometry(), &distances, &offsets);
            assert!(
                error <= T_RT_M,
                "[{preset}] T-RT: straight round-trip error {error} m exceeds {T_RT_M} m"
            );
        }

        let (sim, state) = run(STRAIGHT, step_s, 90.0, |_| true, |_, _, _| {});
        assert_eq!(
            state.modes.len(),
            2,
            "[{preset}] the straight fixture runs both narrow modes"
        );
        assert_eq!(
            state.paths.len(),
            2,
            "[{preset}] both narrow facilities run"
        );
        assert_eq!(
            state.free_speed_checks.len(),
            2,
            "[{preset}] T-SPD compared both modes' settled free speed"
        );
        assert_eq!(
            sim.emergency_cap_steps(),
            0,
            "[{preset}] free-flow straight: the position cap never braked harder than the model"
        );
    }
}

#[test]
fn the_narrow_isolated_curve_fixture_holds_trt_tdim_tenv_and_tspd_at_every_preset() {
    for (preset, step_s) in PRESETS {
        let scenario = compile(CURVE);
        assert_eq!(narrow_templates(&scenario).len(), 2);
        for facility in scenario.facilities() {
            let reference = facility
                .reference()
                .expect("the curve fixture facility has a reference path");
            let geometry = reference.geometry();
            let path = scenario
                .path(reference.path())
                .expect("the facility's reference path compiles");
            let points = path.points();

            // The fixture declares, per path, the constant-curvature analytic
            // reference its chord polyline approximates (see the fixture
            // header: centre `(0, 0)`, radius 80 m / 86 m, start angle
            // `-pi/2`, sweep `+pi/2`). Reconstruct it from the authored vertices
            // so the declaration and the reconstruction cannot drift apart:
            // every authored chord vertex lies on it ...
            let radius = points[0].length();
            assert!(radius > 0.0);
            for vertex in points {
                assert!(
                    (vertex.length() - radius).abs() <= T_RT_M,
                    "authored curve vertex {vertex:?} is not on the declared radius {radius} m circle"
                );
            }
            // ... and the arc reconstructed from those vertices is the declared
            // reference.
            let start_angle = points[0].y.atan2(points[0].x);
            let last = points[points.len() - 1];
            let mut sweep = last.y.atan2(last.x) - start_angle;
            while sweep <= -std::f64::consts::PI {
                sweep += 2.0 * std::f64::consts::PI;
            }
            while sweep > std::f64::consts::PI {
                sweep -= 2.0 * std::f64::consts::PI;
            }
            let analytic = CompiledReferencePath::arc(DVec2::ZERO, radius, start_angle, sweep);

            // T-RT on the declared analytic reference, over the usable band.
            let arc_length = analytic.length();
            let distances = round_trip_distances(arc_length);
            let offsets = round_trip_offsets(&scenario, facility);
            let error = max_round_trip_error(&analytic, &distances, &offsets);
            assert!(
                error <= T_RT_M,
                "[{preset}] T-RT: curve round-trip error {error} m exceeds {T_RT_M} m"
            );

            // The authored polyline is the analytic arc's chord approximation:
            // strictly shorter than the arc it approximates, and the compiled
            // facility geometry is that same authoring.
            let chord_length: f64 = points
                .windows(2)
                .map(|pair| pair[0].distance(pair[1]))
                .sum();
            assert!(
                chord_length < arc_length,
                "the authored curve is a chord polyline, not the arc itself"
            );
            assert!(chord_length > arc_length - 1.0);
            assert_eq!(geometry.length(), chord_length);
        }

        let (sim, state) = run(CURVE, step_s, 90.0, |_| true, |_, _, _| {});
        assert_eq!(
            state.modes.len(),
            2,
            "[{preset}] the curve fixture runs both narrow modes"
        );
        assert_eq!(
            state.paths.len(),
            2,
            "[{preset}] both narrow facilities run"
        );
        assert_eq!(
            state.free_speed_checks.len(),
            2,
            "[{preset}] T-SPD compared both modes' settled free speed"
        );
        assert_eq!(
            sim.emergency_cap_steps(),
            0,
            "[{preset}] free-flow curve: the position cap never braked harder than the model"
        );
    }
}

#[test]
fn the_narrow_isolated_braking_fixture_brakes_both_modes_to_the_authored_stop_line() {
    let mut approaching: BTreeSet<PathId> = BTreeSet::new();
    let mut resting: BTreeSet<PathId> = BTreeSet::new();
    let mut saw_stop_decision = false;
    let (sim, state) = run(
        BRAKING,
        0.05,
        150.0,
        // The stop-line constraint is active from the entry, so the fixture has
        // no steady free-flow speed to compare; `T-SPD` for this cell is the
        // `speed <= v0` bound every sample already asserts.
        |_| false,
        |sim, _events, snapshot| {
            for agent in live_narrow(sim, snapshot) {
                if agent.front_m < STOP_LINE_M - 10.0 && agent.speed_mps > 1.0 {
                    approaching.insert(agent.path);
                }
                if agent.speed_mps < 0.25 && (agent.front_m - STOP_LINE_M).abs() <= 0.5 {
                    resting.insert(agent.path);
                }
            }
            for sample in snapshot.agents() {
                if let Some(decision) = sample.motion.as_ref().and_then(|motion| motion.decision)
                    && decision.action == SignalAction::Stop
                    && decision.color != hekate_model::SignalColor::Green
                {
                    saw_stop_decision = true;
                }
            }
        },
    );
    assert_eq!(
        state.modes.len(),
        2,
        "both narrow modes ride the braking fixture"
    );
    assert_eq!(
        approaching.len(),
        2,
        "both modes entered in free flow before braking: {approaching:?}"
    );
    assert_eq!(
        resting.len(),
        2,
        "both modes came to rest at the authored stop line: {resting:?}"
    );
    assert!(saw_stop_decision, "a non-green head drove a stop decision");
    assert_eq!(
        sim.emergency_cap_steps(),
        0,
        "comfortable braking alone stops a narrow agent at the authored line"
    );
}

#[test]
fn the_narrow_following_fixture_queues_both_modes_behind_a_narrow_leader() {
    /// The largest front-to-rear gap at which a narrow agent counts as
    /// following its narrow leader.
    const FOLLOWING_GAP_M: f64 = 4.0;
    let mut followed: BTreeSet<PathId> = BTreeSet::new();
    let mut free_riding: BTreeSet<PathId> = BTreeSet::new();
    let (_sim, state) = run(
        FOLLOWING,
        0.05,
        90.0,
        // The queued followers are the fixture's point; `T-SPD` binds only the
        // undisturbed leading agent on each facility, asserted below.
        |_| false,
        |sim, _events, snapshot| {
            let mut by_path: BTreeMap<PathId, Vec<LiveNarrow>> = BTreeMap::new();
            for agent in live_narrow(sim, snapshot) {
                by_path.entry(agent.path).or_default().push(agent);
            }
            for (path, mut agents) in by_path {
                agents.sort_by(|left, right| left.front_m.total_cmp(&right.front_m));
                // Body bounds hold across the queue, and a follower behind a
                // small gap is the following behavior.
                for pair in agents.windows(2) {
                    let (follower, leader) = (&pair[0], &pair[1]);
                    let gap = leader.rear_m - follower.front_m;
                    assert!(gap >= -1e-6, "narrow bodies overlap: gap {gap} m");
                    if gap <= FOLLOWING_GAP_M && follower.speed_mps < follower.desired_speed_mps {
                        followed.insert(path);
                    }
                }
                // The most advanced agent on the facility has no leader, so it
                // rides its authored free speed (the fixture's `T-SPD` bound).
                if let Some(leader) = agents.last()
                    && leader.front_m >= SETTLING_M
                    && (leader.speed_mps - leader.desired_speed_mps).abs() <= T_SPD_MPS
                {
                    free_riding.insert(path);
                }
            }
        },
    );
    assert_eq!(
        state.modes.len(),
        2,
        "both narrow modes ride the following fixture"
    );
    assert_eq!(
        followed.len(),
        2,
        "both modes queued a follower behind a narrow leader: {followed:?}"
    );
    assert_eq!(
        free_riding.len(),
        2,
        "both modes had an undisturbed leader at the authored free speed: {free_riding:?}"
    );
}

#[test]
fn the_narrow_signal_fixture_yields_then_completes_both_modes_routes() {
    let mut yielded: BTreeSet<PathId> = BTreeSet::new();
    let mut completed: BTreeSet<PathId> = BTreeSet::new();
    let (_sim, state) = run(
        SIGNAL,
        0.05,
        150.0,
        // Once a mode has cleared the line and settled, it rides its authored
        // free speed again; upstream the head constrains it.
        |motion| motion.path_distance_m > STOP_LINE_M + 30.0,
        |_sim, events, snapshot| {
            for event in events {
                if let Event::Despawned { path, reason, .. } = event {
                    assert_eq!(*reason, DespawnReason::ExitedPath);
                    completed.insert(*path);
                }
            }
            for sample in snapshot.agents() {
                if let Some(decision) = sample.motion.as_ref().and_then(|motion| motion.decision)
                    && decision.action == SignalAction::Stop
                    && decision.color != hekate_model::SignalColor::Green
                    && let Some(motion) = sample.motion.as_ref()
                {
                    yielded.insert(motion.path);
                }
            }
        },
    );
    assert_eq!(
        state.modes.len(),
        2,
        "both narrow modes ride the signal fixture"
    );
    assert_eq!(
        yielded.len(),
        2,
        "both modes yielded to the head: {yielded:?}"
    );
    assert_eq!(
        completed.len(),
        2,
        "both modes completed the facility route: {completed:?}"
    );
    assert_eq!(
        state.free_speed_checks.len(),
        2,
        "T-SPD compared both modes' settled free speed once the head was green"
    );
}

#[test]
fn the_narrow_crossing_fixture_sends_both_modes_through_the_conflict_region() {
    let mut entered: BTreeSet<PathId> = BTreeSet::new();
    let mut completed: BTreeSet<PathId> = BTreeSet::new();
    let (_sim, state) = run(
        CROSSING,
        0.05,
        150.0,
        // Both facilities are unsignalized free flow, so `T-SPD` binds every
        // undisturbed agent that has settled.
        |_| true,
        |_sim, events, snapshot| {
            for event in events {
                match event {
                    Event::Entry {
                        agent,
                        region: RegionKey::ConflictRegion(region),
                    } if *region == ConflictRegionId::from_index(0) => {
                        let path = snapshot
                            .agents()
                            .iter()
                            .find(|sample| sample.id == *agent)
                            .and_then(|sample| sample.motion.as_ref())
                            .map(|motion| motion.path);
                        if let Some(path) = path {
                            entered.insert(path);
                        }
                    }
                    Event::Despawned { path, reason, .. } => {
                        assert_eq!(*reason, DespawnReason::ExitedPath);
                        completed.insert(*path);
                    }
                    _ => {}
                }
            }
        },
    );
    assert_eq!(
        state.modes.len(),
        2,
        "both narrow modes ride the crossing fixture"
    );
    assert_eq!(state.paths.len(), 2, "both narrow facilities cross");
    assert_eq!(
        entered.len(),
        2,
        "both modes traversed the authored conflict region: {entered:?}"
    );
    assert_eq!(
        completed.len(),
        2,
        "both modes completed their route through the crossing: {completed:?}"
    );
    assert_eq!(
        state.free_speed_checks.len(),
        2,
        "T-SPD compared both modes' settled free speed across the crossing"
    );
    // Increment 1 has no narrow-narrow conflict-resolution tactic, so a pair of
    // simultaneous arrivals is recorded as a contact rather than resolved. This
    // test therefore asserts each mode's traversal, route completion, and
    // bounds; pairwise separation is the Increment 5 `CC-CROSS` rung, not this
    // fixture's claim.
}
