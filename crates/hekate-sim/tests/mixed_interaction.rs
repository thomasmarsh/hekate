//! Phase 1 Increment 3 slice E: the minimum-separated mixed benchmark.
//!
//! The checked-in `mixed_interaction_v1` benchmark runs one road movement that
//! crosses two signal-controlled crosswalks, with a pedestrian route in each
//! direction at each crosswalk and an authored `yield` rule on the movement.
//! These tests measure that fixture directly rather than duplicating its text,
//! so the gate cannot drift from the benchmark:
//!
//! - every run completes under nominal demand without non-finite state,
//!   unresolved overlaps, or route deadlock (a pedestrian held still longer
//!   than any legitimate signal wait);
//! - the minimum surface separation between bodies is tracked for both
//!   same-mode pairs and cross-mode pairs, and stays strictly positive;
//! - the same seed reproduces the run exactly;
//! - both modes share one continuous world, one clock, one snapshot, one rule
//!   representation, and one typed event stream.
//!
//! Separation is measured in the test, not in the kernel: online
//! minimum-separation tracking is `PHASE_1_PLAN.md` Increment 4. The exact
//! body distances below are the same circle-versus-box, circle-versus-circle,
//! and box-versus-box distances the body shapes define, computed from the
//! observer snapshot, so a source of penetration can never be an artifact of
//! the kernel's own accounting. The residual the kernel still owns is that a
//! vehicle already committed inside a crossing region does not reserve it
//! against a pedestrian arriving afterwards, and the pedestrian's avoidance is
//! a bounded closing-component cap rather than a swept query; Increment 4's
//! swept collision checks own closing it.

use std::collections::BTreeMap;

use glam::DVec2;
use hekate_model::{CompiledScenario, CrossingId, PathId, parse_scenario_source};
use hekate_sim::{
    AgentId, AgentMode, AgentSample, ControllerModelNames, Event, EventKind, MotionSample,
    RunConfig, Simulation, SnapshotDetail,
};

/// The checked-in mixed gate fixture, measured rather than duplicated.
const BENCHMARK: &str = include_str!("../../../scenarios/benchmarks/mixed_interaction_v1.json5");

/// The fixture's declared sweep: this many seeds, this many ticks each.
///
/// Every seed is a full 200 s run, and the kernel is deterministic, so a seed
/// that is clean is clean for every future run of the same code: the sweep is a
/// fixed, reproducible gate rather than a sample. Slice E measured the same
/// metrics clean over 40 seeds.
const SEEDS: u64 = 24;
const GATE_TICKS: u64 = 4000;

/// Longest hold-still run in ticks that is not a route deadlock.
///
/// A pedestrian legitimately holds still while a forbidding crossing signal
/// lasts: the longest don't-walk phase in the benchmark is 22 s, and 36 s
/// (720 ticks) also covers a queue step and a blocked waypoint. Anything past
/// this threshold is an agent that never resumed, so the threshold is well
/// clear of every legitimate wait. Measured maximums at the benchmark's demand
/// are far below it.
const DEADLOCK_STALL_TICKS: u64 = 1800;

/// Route-completion window in ticks: the longest legitimate pedestrian lifetime
/// (48 m of walking at the slowest sampled speed, a full don't-walk wait, and
/// queueing) with margin. A pedestrian observed before this window still live at
/// the end never completed its route.
const COMPLETION_WINDOW_TICKS: u64 = 1800;

fn scenario(text: &str) -> CompiledScenario {
    let source = parse_scenario_source(text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(text: &str, seed: u64) -> Simulation {
    Simulation::new(scenario(text), RunConfig::new(seed)).expect("simulation builds")
}

/// Live agent samples from one observation.
fn agents(sim: &Simulation) -> Vec<AgentSample> {
    sim.snapshot(SnapshotDetail::Full).agents().to_vec()
}

/// Four corners of an oriented box body in world metres.
fn box_corners(sample: &AgentSample) -> Vec<DVec2> {
    let motion = sample.motion.as_ref().expect("full detail");
    let (sin, cos) = sample.heading_rad.sin_cos();
    [
        (-motion.body_length_m * 0.5, -motion.body_width_m * 0.5),
        (-motion.body_length_m * 0.5, motion.body_width_m * 0.5),
        (motion.body_length_m * 0.5, -motion.body_width_m * 0.5),
        (motion.body_length_m * 0.5, motion.body_width_m * 0.5),
    ]
    .into_iter()
    .map(|(x, y)| sample.position + DVec2::new(x * cos - y * sin, x * sin + y * cos))
    .collect()
}

/// Distance in metres from a point to an oriented box body.
fn point_to_box(point: DVec2, centre: DVec2, heading_rad: f64, length_m: f64, width_m: f64) -> f64 {
    let (sin, cos) = heading_rad.sin_cos();
    let offset = point - centre;
    // Into the body frame: the box axis is the heading, so the rotation is the
    // transpose of the world rotation by `heading_rad`.
    let local = DVec2::new(
        offset.x * cos + offset.y * sin,
        -offset.x * sin + offset.y * cos,
    );
    let clamped = DVec2::new(
        local.x.clamp(-length_m * 0.5, length_m * 0.5),
        local.y.clamp(-width_m * 0.5, width_m * 0.5),
    );
    (local - clamped).length()
}

/// Surface clearance in metres between a circle body and an oriented box body.
fn circle_box_clearance(
    circle_m: DVec2,
    radius_m: f64,
    centre_m: DVec2,
    heading_rad: f64,
    length_m: f64,
    width_m: f64,
) -> f64 {
    point_to_box(circle_m, centre_m, heading_rad, length_m, width_m) - radius_m
}

/// Surface clearance in metres between two oriented box bodies, negative when
/// they overlap.
///
/// In 2-D two convex boxes are disjoint exactly when one of their face-normal
/// axes separates them, so the separating-axis test over the four axes is
/// exact. When no axis separates them the boxes penetrate, and the signed
/// clearance is the negative of the least projection overlap, the minimum
/// translation that separates them. When an axis separates them the boxes are
/// disjoint and the exact distance is the closest vertex-to-box distance, which
/// for two disjoint convex polygons is achieved with a vertex of one as an
/// endpoint. The previous vertex-clamp-only form was always `>= 0`, so it could
/// not detect an overlap at all.
///
/// A penetration no larger than [`BOX_CONTACT_EPSILON_M`] reads as touching
/// (`0.0`): the anti-overlap cap deliberately holds a follower's front bumper at
/// the leader's rear, and the projection arithmetic at that exact contact can
/// read a few ulps negative. This absorbs only that sub-nanometre band, never a
/// physical overlap, so the vehicle-vehicle non-overlap assertion stays
/// meaningful.
fn box_box_clearance(first: &AgentSample, second: &AgentSample) -> f64 {
    let mut penetration_m = f64::INFINITY;
    for axis in [
        box_face_normal(first, true),
        box_face_normal(first, false),
        box_face_normal(second, true),
        box_face_normal(second, false),
    ] {
        let (first_min, first_max) = box_projection(first, axis);
        let (second_min, second_max) = box_projection(second, axis);
        let overlap = first_max.min(second_max) - first_min.max(second_min);
        if overlap < 0.0 {
            return vertex_box_distance(first, second);
        }
        penetration_m = penetration_m.min(overlap);
    }
    if penetration_m <= BOX_CONTACT_EPSILON_M {
        0.0
    } else {
        -penetration_m
    }
}

/// Penetration in metres at or below which two touching boxes are treated as
/// exactly touching rather than overlapping.
const BOX_CONTACT_EPSILON_M: f64 = 1e-9;

/// One of the two axis-aligned face normals of an oriented box body, in world
/// coordinates. `forward` selects the box's forward axis; otherwise its left
/// axis, which is perpendicular for a rigid box.
fn box_face_normal(sample: &AgentSample, forward: bool) -> DVec2 {
    let (sin, cos) = sample.heading_rad.sin_cos();
    if forward {
        DVec2::new(cos, sin)
    } else {
        DVec2::new(-sin, cos)
    }
}

/// Interval `[min, max]` an oriented box body spans when projected onto `axis`.
fn box_projection(sample: &AgentSample, axis: DVec2) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for corner in box_corners(sample) {
        let projection = corner.dot(axis);
        min = min.min(projection);
        max = max.max(projection);
    }
    (min, max)
}

/// Exact distance in metres between two disjoint oriented box bodies, via
/// clamping each box's vertices onto the other box.
fn vertex_box_distance(first: &AgentSample, second: &AgentSample) -> f64 {
    let first_motion = first.motion.as_ref().expect("full detail");
    let second_motion = second.motion.as_ref().expect("full detail");
    let mut clearance_m = f64::INFINITY;
    for corner in box_corners(first) {
        clearance_m = clearance_m.min(point_to_box(
            corner,
            second.position,
            second.heading_rad,
            second_motion.body_length_m,
            second_motion.body_width_m,
        ));
    }
    for corner in box_corners(second) {
        clearance_m = clearance_m.min(point_to_box(
            corner,
            first.position,
            first.heading_rad,
            first_motion.body_length_m,
            first_motion.body_width_m,
        ));
    }
    clearance_m
}

/// Surface clearance in metres between two bodies, negative when they overlap.
///
/// A pedestrian body is a circle of its reported radius, a vehicle body an
/// oriented box of its reported length and width.
fn pair_clearance_m(first: &AgentSample, second: &AgentSample) -> f64 {
    let first_motion = first.motion.as_ref().expect("full detail");
    let second_motion = second.motion.as_ref().expect("full detail");
    match (first_motion.mode, second_motion.mode) {
        (AgentMode::Pedestrian, AgentMode::Pedestrian) => {
            (second.position - first.position).length()
                - first_motion.body_length_m * 0.5
                - second_motion.body_length_m * 0.5
        }
        (AgentMode::Pedestrian, AgentMode::Vehicle) => circle_box_clearance(
            first.position,
            first_motion.body_length_m * 0.5,
            second.position,
            second.heading_rad,
            second_motion.body_length_m,
            second_motion.body_width_m,
        ),
        (AgentMode::Vehicle, AgentMode::Pedestrian) => circle_box_clearance(
            second.position,
            second_motion.body_length_m * 0.5,
            first.position,
            first.heading_rad,
            first_motion.body_length_m,
            first_motion.body_width_m,
        ),
        (AgentMode::Vehicle, AgentMode::Vehicle) => box_box_clearance(first, second),
    }
}

/// Whether a circle body overlaps a closed polygon ring.
fn circle_overlaps_ring(ring: &[DVec2], centre: DVec2, radius_m: f64) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = ring.len() - 1;
    for index in 0..ring.len() {
        let (start, end) = (ring[index], ring[previous]);
        if (start.y > centre.y) != (end.y > centre.y)
            && centre.x < (end.x - start.x) * (centre.y - start.y) / (end.y - start.y) + start.x
        {
            inside = !inside;
        }
        previous = index;
    }
    if inside {
        return true;
    }
    for index in 0..ring.len() {
        let (start, end) = (ring[index], ring[(index + 1) % ring.len()]);
        let segment = end - start;
        let length_sq = segment.length_squared();
        let fraction = if length_sq > 0.0 {
            ((centre - start).dot(segment) / length_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if (centre - (start + segment * fraction)).length() <= radius_m {
            return true;
        }
    }
    false
}

/// Lowest observed clearance and where it was observed.
#[derive(Debug, Clone, Copy)]
struct Minimum {
    clearance_m: f64,
    tick: u64,
    first: u32,
    second: u32,
    set: bool,
}

impl Minimum {
    fn unset() -> Self {
        Self {
            clearance_m: f64::INFINITY,
            tick: 0,
            first: 0,
            second: 0,
            set: false,
        }
    }

    fn observe(&mut self, clearance_m: f64, tick: u64, first: u32, second: u32) {
        if !self.set || clearance_m < self.clearance_m {
            *self = Self {
                clearance_m,
                tick,
                first,
                second,
                set: true,
            };
        }
    }
}

/// One run's observables, gathered as the run advances.
struct RunReport {
    /// Minimum clearance between a pedestrian and a vehicle body.
    cross_mode: Minimum,
    /// Minimum clearance between two pedestrian bodies.
    pedestrian_pair: Minimum,
    /// Minimum clearance between two vehicle bodies.
    vehicle_pair: Minimum,
    /// Overlapping pairs observed at a tick end, and the cross-mode share.
    overlaps: u32,
    cross_mode_overlaps: u32,
    /// Cross-mode overlaps a vehicle caused by moving onto a pedestrian that was
    /// not itself stepping into the vehicle.
    vehicle_initiated: u32,
    /// Ticks at which a vehicle reported yielding, and how many of those saw a
    /// pedestrian body inside the crossing region it yielded to.
    yielding_ticks: u64,
    yielding_ticks_with_occupied_region: u64,
    /// Yield transitions that began, and how many began while a pedestrian body
    /// was inside the crossing region in the frame the transition was emitted.
    yield_begins: u64,
    yield_begins_with_occupied_region: u64,
    /// Spawns and despawns seen per mode, through one event stream.
    vehicles_spawned: u64,
    pedestrians_spawned: u64,
    vehicles_despawned: u64,
    pedestrians_despawned: u64,
    /// Safety records seen per mode, through the same one event stream.
    vehicle_safety_events: u64,
    pedestrian_safety_events: u64,
    /// Safety records seen per kind, so the gate can require every kind the
    /// benchmark produces rather than a total.
    safety_event_kinds: BTreeMap<String, u64>,
    /// Ticks on which the observed records did not follow the documented
    /// within-tick order.
    out_of_order_ticks: u64,
    /// First tick each pedestrian was observed, and the pedestrians still live
    /// at the end, so the run can assert route completion rather than presence.
    pedestrian_first_seen: BTreeMap<u32, u64>,
    live_pedestrians: Vec<u32>,
    /// Whether one observation carried both modes.
    saw_both_modes_in_one_frame: bool,
    /// Longest run of ticks a pedestrian held a constant route progress.
    max_pedestrian_stall_ticks: u64,
    /// Ticks of non-finite observed state.
    non_finite: u32,
    dropped: u64,
    spawned_total: u64,
    despawned_total: u64,
}

/// Run a scenario and gather the slice-E observables.
fn run_report(text: &str, seed: u64, ticks: u64) -> RunReport {
    let mut sim = sim(text, seed);
    let mut report = RunReport {
        cross_mode: Minimum::unset(),
        pedestrian_pair: Minimum::unset(),
        vehicle_pair: Minimum::unset(),
        overlaps: 0,
        cross_mode_overlaps: 0,
        vehicle_initiated: 0,
        yielding_ticks: 0,
        yielding_ticks_with_occupied_region: 0,
        yield_begins: 0,
        yield_begins_with_occupied_region: 0,
        vehicles_spawned: 0,
        pedestrians_spawned: 0,
        vehicles_despawned: 0,
        pedestrians_despawned: 0,
        vehicle_safety_events: 0,
        pedestrian_safety_events: 0,
        safety_event_kinds: BTreeMap::new(),
        out_of_order_ticks: 0,
        pedestrian_first_seen: BTreeMap::new(),
        live_pedestrians: Vec::new(),
        saw_both_modes_in_one_frame: false,
        max_pedestrian_stall_ticks: 0,
        non_finite: 0,
        dropped: 0,
        spawned_total: 0,
        despawned_total: 0,
    };
    let scenario = sim.scenario().clone();
    let mut progress: BTreeMap<u32, f64> = BTreeMap::new();
    let mut stall: BTreeMap<u32, u64> = BTreeMap::new();

    for _ in 0..ticks {
        let tick = sim.time().tick();
        let before = agents(&sim);
        let events: Vec<Event> = sim.step().events().to_vec();
        let after = agents(&sim);

        for event in &events {
            match event {
                Event::Spawned { mode, .. } => match mode {
                    AgentMode::Vehicle => report.vehicles_spawned += 1,
                    AgentMode::Pedestrian => report.pedestrians_spawned += 1,
                },
                Event::Despawned { agent, .. } => match sim.agent_mode(*agent) {
                    Some(AgentMode::Vehicle) => report.vehicles_despawned += 1,
                    Some(AgentMode::Pedestrian) => report.pedestrians_despawned += 1,
                    None => {}
                },
                Event::Yielded {
                    crossing,
                    yielding: true,
                    ..
                } => {
                    report.yield_begins += 1;
                    if pedestrian_occupies_crossing(&scenario, &before, *crossing)
                        || pedestrian_occupies_crossing(&scenario, &after, *crossing)
                    {
                        report.yield_begins_with_occupied_region += 1;
                    }
                }
                other => match other.kind() {
                    EventKind::Spawned | EventKind::Despawned | EventKind::Yielded => {}
                    kind => {
                        *report
                            .safety_event_kinds
                            .entry(format!("{kind:?}"))
                            .or_insert(0) += 1;
                        match sim.agent_mode(other.agent()) {
                            Some(AgentMode::Vehicle) => report.vehicle_safety_events += 1,
                            Some(AgentMode::Pedestrian) => report.pedestrian_safety_events += 1,
                            None => {}
                        }
                    }
                },
            }
        }
        // The documented within-tick order is a property of the records, so the
        // benchmark gate checks it on every tick rather than in one fixture.
        if !events
            .windows(2)
            .all(|pair| pair[0].order_key() <= pair[1].order_key())
        {
            report.out_of_order_ticks += 1;
        }

        let has_vehicle = after
            .iter()
            .any(|sample| sample.motion.as_ref().expect("full detail").mode == AgentMode::Vehicle);
        let has_pedestrian = after.iter().any(|sample| {
            sample.motion.as_ref().expect("full detail").mode == AgentMode::Pedestrian
        });
        report.saw_both_modes_in_one_frame |= has_vehicle && has_pedestrian;

        // Yielding is recomputed before the step integrates; both the pre-step
        // and the post-step frame are checked, because a body that leaves the
        // region inside the step must not look like an unoccupied yield.
        for sample in &after {
            if let Some(crossing) = sim.agent_yield_crossing(sample.id) {
                report.yielding_ticks += 1;
                if pedestrian_occupies_crossing(&scenario, &before, crossing)
                    || pedestrian_occupies_crossing(&scenario, &after, crossing)
                {
                    report.yielding_ticks_with_occupied_region += 1;
                }
            }
        }

        for (index, sample) in after.iter().enumerate() {
            let motion = sample.motion.as_ref().expect("full detail");
            if !(sample.position.is_finite()
                && sample.heading_rad.is_finite()
                && motion.speed_mps.is_finite())
            {
                report.non_finite += 1;
            }
            if motion.mode == AgentMode::Pedestrian {
                report
                    .pedestrian_first_seen
                    .entry(sample.id.get())
                    .or_insert(tick);
                let advanced = progress
                    .insert(sample.id.get(), motion.path_distance_m)
                    .is_none_or(|previous| (motion.path_distance_m - previous).abs() >= 1e-4);
                let entry = stall.entry(sample.id.get()).or_insert(0);
                if advanced {
                    *entry = 0;
                } else {
                    *entry += 1;
                    report.max_pedestrian_stall_ticks =
                        report.max_pedestrian_stall_ticks.max(*entry);
                }
            }
            for other in &after[index + 1..] {
                let other_motion = other.motion.as_ref().expect("full detail");
                let clearance_m = pair_clearance_m(sample, other);
                match (motion.mode, other_motion.mode) {
                    (AgentMode::Pedestrian, AgentMode::Pedestrian) => report
                        .pedestrian_pair
                        .observe(clearance_m, tick, sample.id.get(), other.id.get()),
                    (AgentMode::Vehicle, AgentMode::Vehicle) => report.vehicle_pair.observe(
                        clearance_m,
                        tick,
                        sample.id.get(),
                        other.id.get(),
                    ),
                    _ => report.cross_mode.observe(
                        clearance_m,
                        tick,
                        sample.id.get(),
                        other.id.get(),
                    ),
                }
                if clearance_m >= 0.0 {
                    continue;
                }
                report.overlaps += 1;
                if motion.mode == other_motion.mode {
                    continue;
                }
                report.cross_mode_overlaps += 1;
                let (pedestrian, vehicle) = if motion.mode == AgentMode::Pedestrian {
                    (sample, other)
                } else {
                    (other, sample)
                };
                let pedestrian_motion = pedestrian.motion.as_ref().expect("full detail");
                let vehicle_motion = vehicle.motion.as_ref().expect("full detail");
                let (Some(prior_pedestrian), Some(prior_vehicle)) = (
                    before.iter().find(|prior| prior.id == pedestrian.id),
                    before.iter().find(|prior| prior.id == vehicle.id),
                ) else {
                    continue;
                };
                let clearance_before_m = circle_box_clearance(
                    prior_pedestrian.position,
                    pedestrian_motion.body_length_m * 0.5,
                    prior_vehicle.position,
                    prior_vehicle.heading_rad,
                    vehicle_motion.body_length_m,
                    vehicle_motion.body_width_m,
                );
                if clearance_before_m < 0.0 {
                    continue;
                }
                // The pedestrian stepped into the vehicle's pre-step body if its
                // new position already overlapped where the vehicle was;
                // anything else is the vehicle moving onto the pedestrian.
                let stepped_into_it = circle_box_clearance(
                    pedestrian.position,
                    pedestrian_motion.body_length_m * 0.5,
                    prior_vehicle.position,
                    prior_vehicle.heading_rad,
                    vehicle_motion.body_length_m,
                    vehicle_motion.body_width_m,
                ) < 0.0;
                if !stepped_into_it {
                    report.vehicle_initiated += 1;
                }
            }
        }
    }

    report.live_pedestrians = agents(&sim)
        .iter()
        .filter(|sample| sample.motion.as_ref().expect("full detail").mode == AgentMode::Pedestrian)
        .map(|sample| sample.id.get())
        .collect();
    let summary = sim.finish();
    report.dropped = summary.dropped();
    report.spawned_total = summary.spawned();
    report.despawned_total = summary.despawned();
    report
}

/// Whether any pedestrian body in `frame` overlaps the crossing's region ring.
fn pedestrian_occupies_crossing(
    scenario: &CompiledScenario,
    frame: &[AgentSample],
    crossing: CrossingId,
) -> bool {
    let Some(ring) = scenario
        .crossing(crossing)
        .and_then(|crossing| scenario.region(crossing.region()))
        .map(|region| region.polygon().ring())
    else {
        return false;
    };
    frame.iter().any(|sample| {
        let motion = sample.motion.as_ref().expect("full detail");
        motion.mode == AgentMode::Pedestrian
            && circle_overlaps_ring(ring, sample.position, motion.body_length_m * 0.5)
    })
}

/// A one-line summary of the lowest clearance a sweep observed.
fn describe(minimum: Minimum) -> String {
    if minimum.set {
        format!(
            "{:.4} m at tick {} between agents {} and {}",
            minimum.clearance_m, minimum.tick, minimum.first, minimum.second
        )
    } else {
        "never observed (a mode was absent)".to_owned()
    }
}

/// A synthetic vehicle sample, so the signed box-box measure can be unit
/// checked without running the kernel.
fn vehicle_sample(
    id: u32,
    position: DVec2,
    heading_rad: f64,
    length_m: f64,
    width_m: f64,
) -> AgentSample {
    AgentSample {
        id: AgentId::from_index(id as usize),
        position,
        heading_rad,
        motion: Some(MotionSample {
            body_kind: AgentMode::Vehicle.body_kind(),
            segments: Vec::new(),
            mode: AgentMode::Vehicle,
            speed_mps: 0.0,
            path: PathId::from_index(0),
            path_distance_m: 0.0,
            body_length_m: length_m,
            body_width_m: width_m,
            route: None,
            profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            decision: None,
            pedestrian_decision: None,
            yield_crossing: None,
            route_state: None,
        }),
    }
}

/// The vehicle-vehicle arm must be able to fail on a real overlap: a signed
/// box-box measure is negative when two aligned boxes overlap, zero when they
/// touch, and positive when they are clear. The old vertex-clamp form read
/// exactly `0.0` for the overlapping case, so the `>= 0.0` assertion could never
/// detect it.
#[test]
fn box_box_clearance_detects_an_overlap() {
    let first = vehicle_sample(0, DVec2::ZERO, 0.0, 4.0, 2.0);
    // Centres 3 m apart: the 4 m-long boxes overlap by 1 m along x.
    let overlapping = vehicle_sample(1, DVec2::new(3.0, 0.0), 0.0, 4.0, 2.0);
    assert!(
        box_box_clearance(&first, &overlapping) < 0.0,
        "an aligned overlap must be negative, got {}",
        box_box_clearance(&first, &overlapping)
    );
    // Centres exactly 4 m apart: the faces touch, the closest non-overlapping
    // state, which is not an overlap.
    let touching = vehicle_sample(2, DVec2::new(4.0, 0.0), 0.0, 4.0, 2.0);
    assert_eq!(box_box_clearance(&first, &touching), 0.0);
    // Centres 5 m apart: a 1 m gap.
    let clear = vehicle_sample(3, DVec2::new(5.0, 0.0), 0.0, 4.0, 2.0);
    assert!((box_box_clearance(&first, &clear) - 1.0).abs() < 1e-12);
    // A rotated 90-degree box also overlaps when it reaches the first box.
    let turned = vehicle_sample(
        4,
        DVec2::new(2.5, 0.0),
        std::f64::consts::FRAC_PI_2,
        4.0,
        2.0,
    );
    assert!(box_box_clearance(&first, &turned) < 0.0);
}

/// One sweep of the benchmark's declared seed range, shared by every gate
/// assertion below so the expensive 24 x 4000-tick run is paid once rather than
/// once per assertion family.
fn sweep_reports() -> Vec<RunReport> {
    (0..SEEDS)
        .map(|seed| run_report(BENCHMARK, seed, GATE_TICKS))
        .collect()
}

/// Every seed's run completes under nominal demand without non-finite state,
/// unresolved overlaps, or route deadlock.
fn assert_completion_without_nan_overlap_or_deadlock(reports: &[RunReport]) {
    for (seed, report) in reports.iter().enumerate() {
        assert_eq!(
            report.non_finite, 0,
            "seed {seed}: non-finite agent state was observed"
        );
        assert!(
            report.saw_both_modes_in_one_frame,
            "seed {seed}: both modes must share one observation"
        );
        assert_eq!(
            report.overlaps,
            0,
            "seed {seed}: an unresolved overlap was observed; cross-mode minimum separation {}",
            describe(report.cross_mode)
        );
        assert_eq!(
            report.cross_mode_overlaps,
            0,
            "seed {seed}: a pedestrian and a vehicle overlapped; cross-mode minimum separation {}",
            describe(report.cross_mode)
        );
        assert_eq!(
            report.vehicle_initiated, 0,
            "seed {seed}: a vehicle moved onto a pedestrian"
        );
        assert!(
            report.max_pedestrian_stall_ticks < DEADLOCK_STALL_TICKS,
            "seed {seed}: a pedestrian held still for {} ticks (route deadlock)",
            report.max_pedestrian_stall_ticks
        );
        assert_eq!(report.dropped, 0, "seed {seed}: nominal demand was shed");
        assert!(report.spawned_total > 0 && report.despawned_total > 0);
        assert!(
            report.vehicles_spawned > 0
                && report.vehicles_despawned > 0
                && report.pedestrians_spawned > 0
                && report.pedestrians_despawned > 0,
            "seed {seed}: both modes must be admitted and complete through one event stream \
             ({} vehicles and {} pedestrians spawned, {} and {} despawned)",
            report.vehicles_spawned,
            report.pedestrians_spawned,
            report.vehicles_despawned,
            report.pedestrians_despawned,
        );
        // Route completion, not just presence: a pedestrian observed well before
        // the end must have left on its own before the end. The window is wider
        // than the longest legitimate lifetime (walk time, a full don't-walk
        // wait, and queueing), so a still-live pedestrian inside it is a route
        // deadlock.
        for (id, first_seen) in &report.pedestrian_first_seen {
            if *first_seen < GATE_TICKS - COMPLETION_WINDOW_TICKS {
                assert!(
                    !report.live_pedestrians.contains(id),
                    "seed {seed}: pedestrian {id} first seen at tick {first_seen} never \
                     completed its route"
                );
            }
        }
        assert!(
            report.yield_begins > 0,
            "seed {seed}: no vehicle ever yielded to an occupied crossing"
        );
    }
}

/// The minimum surface separation between bodies of every pair kind stays
/// positive (or non-negative for vehicle pairs) across the whole sweep, and no
/// vehicle ever moves onto a pedestrian.
fn assert_positive_minimum_separation(reports: &[RunReport]) {
    let mut cross_mode = Minimum::unset();
    let mut pedestrian_pair = Minimum::unset();
    let mut vehicle_pair = Minimum::unset();
    let mut vehicle_initiated = 0;
    for (seed, report) in reports.iter().enumerate() {
        assert!(
            report.cross_mode.set && report.pedestrian_pair.set && report.vehicle_pair.set,
            "seed {seed}: a body pair of every kind must be observed"
        );
        assert!(
            report.cross_mode.clearance_m > 0.0,
            "seed {seed}: cross-mode bodies touched: {}",
            describe(report.cross_mode)
        );
        assert!(
            report.pedestrian_pair.clearance_m >= 0.0 && report.vehicle_pair.clearance_m >= 0.0,
            "seed {seed}: same-mode bodies overlapped"
        );
        vehicle_initiated += report.vehicle_initiated;
        for (incumbent, observed) in [
            (&mut cross_mode, report.cross_mode),
            (&mut pedestrian_pair, report.pedestrian_pair),
            (&mut vehicle_pair, report.vehicle_pair),
        ] {
            incumbent.observe(
                observed.clearance_m,
                observed.tick,
                observed.first,
                observed.second,
            );
        }
    }
    assert_eq!(
        vehicle_initiated, 0,
        "the fixture must never let a vehicle move onto a pedestrian"
    );
    assert!(
        cross_mode.clearance_m > 0.0,
        "the cross-mode sweep must keep a strictly positive minimum separation, got {}",
        describe(cross_mode)
    );
    // Two pedestrians keep the controller's contact margin; two vehicles may
    // come to rest exactly touching, because the anti-overlap cap holds a
    // follower's front bumper at the leader's rear rather than short of it. Both
    // therefore only require no penetration.
    assert!(
        pedestrian_pair.clearance_m > 0.0,
        "pedestrian bodies must keep a strictly positive minimum separation, got {}",
        describe(pedestrian_pair)
    );
    assert!(
        vehicle_pair.clearance_m >= 0.0,
        "vehicle bodies must never overlap, got {}",
        describe(vehicle_pair)
    );
    // The measured minima are this fixture's headline numbers, so report them:
    // `cargo test -p hekate-sim --test mixed_interaction -- --nocapture` prints
    // the evidence a verification pass can compare against.
    println!(
        "mixed_interaction_v1 over {SEEDS} seeds x {GATE_TICKS} ticks: \
         minimum separation cross-mode {}, pedestrian-pedestrian {}, vehicle-vehicle {}",
        describe(cross_mode),
        describe(pedestrian_pair),
        describe(vehicle_pair),
    );
}

#[test]
fn the_mixed_benchmark_shares_one_world_one_rule_and_one_event_stream() {
    // Cars and pedestrians share the continuous world, the clock, the snapshot,
    // the authored crossing rule, and the typed event stream. The vehicle-side
    // occupancy query reads the shared deterministic spatial index; see
    // `crates/hekate-sim/src/index.rs` for that seam's own unit evidence.
    let report = run_report(BENCHMARK, 3, GATE_TICKS);
    assert!(
        report.saw_both_modes_in_one_frame,
        "both modes must appear in one snapshot on one clock"
    );
    assert!(
        report.vehicles_spawned > 0
            && report.pedestrians_spawned > 0
            && report.vehicles_despawned > 0
            && report.pedestrians_despawned > 0,
        "one event stream must carry both modes: {} vehicles and {} pedestrians spawned, \
         {} and {} despawned",
        report.vehicles_spawned,
        report.pedestrians_spawned,
        report.vehicles_despawned,
        report.pedestrians_despawned,
    );
    assert!(
        report.yield_begins > 0 && report.yielding_ticks > 0,
        "the shared `yield` rule must make a vehicle react to a pedestrian body"
    );
    // The authored rule makes a vehicle yield exactly while a pedestrian body
    // occupies the crossing region: the yield state and the occupancy that
    // caused it must agree. The boundary tick is allowed to disagree, because
    // the state is recomputed before the step integrates.
    assert!(
        report.yielding_ticks_with_occupied_region * 10 >= report.yielding_ticks * 9,
        "only {}/{} yielding ticks had a pedestrian in the crossing region",
        report.yielding_ticks_with_occupied_region,
        report.yielding_ticks
    );
    assert!(
        report.yield_begins_with_occupied_region * 10 >= report.yield_begins * 9,
        "only {}/{} yield transitions began with a pedestrian in the region",
        report.yield_begins_with_occupied_region,
        report.yield_begins
    );

    // Both models of the run are named through the interface seam.
    let mut sim = sim(BENCHMARK, 3);
    sim.step();
    assert_eq!(
        sim.controller_models(),
        ControllerModelNames {
            vehicle: "idm",
            narrow: "idm-narrow",
            pedestrian: "waypoint",
        }
    );
}

#[test]
fn the_mixed_benchmark_reproduces_for_the_same_seed() {
    fn trace(seed: u64) -> Vec<String> {
        let mut sim = sim(BENCHMARK, seed);
        let mut frames = Vec::new();
        for _ in 0..1200 {
            let events: Vec<Event> = sim.step().events().to_vec();
            let mut frame = format!("{events:?}");
            for sample in agents(&sim) {
                let motion = sample.motion.as_ref().expect("full detail");
                frame.push_str(&format!(
                    " {}:{:?}:{:?}:{:.12}:{:.12}:{:?}",
                    sample.id.get(),
                    motion.mode,
                    sample.position,
                    motion.speed_mps,
                    motion.path_distance_m,
                    sim.agent_pedestrian_decision(sample.id).map(|d| d.action),
                ));
            }
            frames.push(frame);
        }
        frames
    }
    assert_eq!(trace(7), trace(7));
    assert_ne!(trace(7), trace(8), "different seeds must diverge");
}

/// Slice C: the same one stream must carry the typed safety records of both
/// modes, in the documented within-tick order, deterministically. These are the
/// slice-E observables of every seed in the sweep, so the gate covers the whole
/// benchmark rather than one run.
fn assert_one_ordered_safety_stream_for_both_modes(reports: &[RunReport]) {
    let mut all_kinds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (seed, report) in reports.iter().enumerate() {
        assert_eq!(
            report.out_of_order_ticks, 0,
            "seed {seed}: a tick's records left the documented order"
        );
        assert!(
            report.vehicle_safety_events > 0 && report.pedestrian_safety_events > 0,
            "seed {seed}: one stream must carry both modes' safety records \
             ({} vehicle, {} pedestrian)",
            report.vehicle_safety_events,
            report.pedestrian_safety_events,
        );
        all_kinds.extend(report.safety_event_kinds.keys().cloned());
        if seed == 0 {
            println!(
                "mixed_interaction_v1 seed 0: {} vehicle and {} pedestrian safety records, \
                 kinds {:?}",
                report.vehicle_safety_events,
                report.pedestrian_safety_events,
                report.safety_event_kinds,
            );
        }
    }
    // The sweep as a whole exercises every record family; an individual seed
    // need not, because a body contact depends on the arrival draw.
    for kind in [
        "Collision",
        "NearMiss",
        "Entry",
        "Exit",
        "Queue",
        "ControlTransition",
    ] {
        assert!(
            all_kinds.contains(kind),
            "the sweep never produced a {kind} record; saw {all_kinds:?}"
        );
    }
}

/// The mixed benchmark's slice-E gate over every seed of the declared sweep.
///
/// The sweep is the expensive part of this gate, so the three assertion
/// families above read one sweep rather than each running its own.
#[test]
#[ignore = "slow: 24-seed x 4000-tick mixed-interaction safety sweep; run scripts/run-test-harness.sh"]
fn the_mixed_benchmark_gate_holds_over_every_declared_seed() {
    let reports = sweep_reports();
    assert_completion_without_nan_overlap_or_deadlock(&reports);
    assert_positive_minimum_separation(&reports);
    assert_one_ordered_safety_stream_for_both_modes(&reports);
}
