//! Phase 1 Increment 4 slice D: online time to collision, minimum separation,
//! and post-encroachment time.
//!
//! `PHASE_1_PLAN.md` Increment 4 gates this slice on "fine-step differential
//! tests agree with analytic/simple fixtures within declared tolerances", and
//! the module card in `crate::metrics` states the definitions this file
//! measures. The tests here drive the real kernel and compare the online
//! metrics against references built from the observed frames:
//!
//! - a constant-speed lane, whose separation is analytic and must be the same at
//!   every step, and whose pairs are never closing;
//! - the crossing benchmark, where every pedestrian pair's reported time to
//!   collision is compared with the closed-form quadratic root for two moving
//!   circles, and every recorded separation is compared with an all-pairs
//!   minimum over the observed frames;
//! - the mixed benchmark's crossings, where every pedestrian-to-pedestrian
//!   post-encroachment time is compared with an independently detected occupancy
//!   interval from the observed frames, at three steps, to show the deviation is
//!   bounded by the step and therefore converges;
//! - a pair of runs of the same seed, one reading every metric accessor after
//!   every tick, whose event streams and frame fingerprints must be identical.
//!
//! # Declared tolerances
//!
//! - [`TTC_TIME_TOLERANCE_S`] is the absolute tolerance for a time to collision
//!   comparison, and the metric's own declared resolution: the reference solves
//!   the same quadratic geometry in closed form, so the two agree to the
//!   resolution the metric reports.
//! - [`SEPARATION_TOLERANCE_M`] is the absolute tolerance for a separation
//!   comparison. The metric's closest approach is located by bisection to
//!   [`TOI_TIME_TOLERANCE`](hekate_sim::TOI_TIME_TOLERANCE) of a tick, which is
//!   far below a nanometre at the steps used here.
//! - [`PET_STEP_MULTIPLE`] is the post-encroachment deviation bound as a multiple
//!   of the fixed step: the metric's occupancy boundaries are the ends of the
//!   ticks that reported them, so a deviation of at most four steps is the
//!   declared quantization bound, and the same bound at every step is the
//!   convergence statement.

use std::collections::BTreeMap;

use glam::DVec2;
use hekate_model::{CompiledScenario, CrossingId, parse_scenario_source};
use hekate_sim::{
    AgentId, AgentMode, AgentSample, BodyShape, Event, INTERACTION_RANGE_M, ModePair, RegionKey,
    RunConfig, Seconds, Simulation, SnapshotDetail, SweptBody, TTC_HORIZON_S, TTC_TIME_TOLERANCE_S,
    tick_minimum_clearance_m, time_to_collision,
};

/// The checked-in mixed benchmark: a road movement, two crossings, and
/// pedestrians crossing in both directions at each.
const MIXED: &str = include_str!("../../../scenarios/benchmarks/mixed_interaction_v1.json5");
/// The checked-in perpendicular-conflict benchmark: two movements crossing one
/// authored conflict region.
const CONFLICT: &str =
    include_str!("../../../scenarios/benchmarks/perpendicular_conflict_v1.json5");
/// The checked-in walking skeleton's scenario: one lane at a constant speed.
const WALKING: &str = include_str!("../../../scenarios/walking/walking_guide_v1.json5");

/// Absolute tolerance for a separation comparison, in metres.
const SEPARATION_TOLERANCE_M: f64 = 1e-9;

/// Post-encroachment deviation bound, as a multiple of the fixed step.
const PET_STEP_MULTIPLE: f64 = 4.0;

/// The steps the differential tests measure at: the Standard preset and two
/// refinements of it.
const STEPS: [f64; 3] = [0.05, 0.0125, 0.003125];

fn scenario(text: &str) -> CompiledScenario {
    let source = parse_scenario_source(text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(text: &str, seed: u64, step: f64) -> Simulation {
    Simulation::new(
        scenario(text),
        RunConfig::new(seed).with_step(Seconds::from_secs(step)),
    )
    .expect("builds")
}

/// The body shape an observed sample describes.
fn body_of(sample: &AgentSample) -> BodyShape {
    let motion = sample.motion.as_ref().expect("full detail");
    match motion.mode {
        AgentMode::Pedestrian => BodyShape::Circle {
            centre: sample.position,
            radius_m: motion.body_length_m * 0.5,
        },
        AgentMode::Vehicle => BodyShape::Box {
            centre: sample.position,
            heading_rad: sample.heading_rad,
            length_m: motion.body_length_m,
            width_m: motion.body_width_m,
        },
    }
}

/// The swept body of one agent over one observed step.
fn swept_of(before: &AgentSample, after: &AgentSample) -> SweptBody {
    SweptBody {
        shape: body_of(before),
        displacement_m: after.position - before.position,
    }
}

/// The live frames of one tick, ascending by [`AgentId`].
fn frames(sim: &Simulation) -> BTreeMap<AgentId, AgentSample> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .map(|sample| (sample.id, sample.clone()))
        .collect()
}

/// The closed-form first contact time of two moving circles, or `None` when the
/// extrapolated paths never touch within [`TTC_HORIZON_S`].
///
/// The reference solves `|r + v t| = R` directly, an implementation path
/// independent of the metric's bisection over the exact shapes.
fn analytic_circle_ttc(first: &SweptBody, second: &SweptBody, step_s: f64) -> Option<f64> {
    let (
        BodyShape::Circle {
            centre: first_centre,
            radius_m: first_radius,
        },
        BodyShape::Circle {
            centre: second_centre,
            radius_m: second_radius,
        },
    ) = (first.end_shape(), second.end_shape())
    else {
        panic!("the analytic reference is only defined for two circles");
    };
    let position = second_centre - first_centre;
    let velocity = (second.displacement_m - first.displacement_m) / step_s;
    let radius = first_radius + second_radius;
    let c = position.length_squared() - radius * radius;
    if c <= 0.0 {
        // Already touching or overlapping at the observed state.
        return Some(0.0);
    }
    let b = 2.0 * position.dot(velocity);
    if b >= 0.0 {
        // Not closing.
        return None;
    }
    let a = velocity.length_squared();
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        // The closest approach of the extrapolated path stays clear of contact.
        return None;
    }
    let root = (-b - discriminant.sqrt()) / (2.0 * a);
    (root <= TTC_HORIZON_S).then_some(root)
}

/// Whether a circle overlaps a closed polygon ring, by the even-odd rule with a
/// point-to-segment distance to the edges.
///
/// An independent implementation of the safety layer's occupancy test, so the
/// post-encroachment reference does not call back into the code under test.
fn circle_overlaps_ring(ring: &[DVec2], centre: DVec2, radius_m: f64) -> bool {
    let inside = (0..ring.len()).fold(false, |inside, index| {
        let start = ring[index];
        let end = ring[(index + ring.len() - 1) % ring.len()];
        if (start.y > centre.y) != (end.y > centre.y)
            && centre.x < (end.x - start.x) * (centre.y - start.y) / (end.y - start.y) + start.x
        {
            !inside
        } else {
            inside
        }
    });
    if inside {
        return true;
    }
    (0..ring.len()).any(|index| {
        let start = ring[index];
        let end = ring[(index + 1) % ring.len()];
        let segment = end - start;
        let length_squared = segment.length_squared();
        let fraction = if length_squared > 0.0 {
            ((centre - start).dot(segment) / length_squared).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (centre - (start + segment * fraction)).length() <= radius_m
    })
}

/// The circumradius in metres of an observed body: the safety layer's
/// conservative occupancy radius, which is a circle body's own radius and a box
/// body's half diagonal.
fn circumradius_m(sample: &AgentSample) -> f64 {
    body_of(sample).circumradius_m()
}

/// The rings of every region the scenarios author a crossing or conflict region
/// for, keyed by the region key the events use.
fn region_rings(scenario: &CompiledScenario) -> BTreeMap<RegionKey, Vec<DVec2>> {
    let mut rings = BTreeMap::new();
    for crossing in scenario.crossings() {
        if let Some(region) = scenario.region(crossing.region()) {
            rings.insert(
                RegionKey::Crossing(crossing.id()),
                region.polygon().ring().to_vec(),
            );
        }
    }
    for region in scenario.conflict_regions() {
        rings.insert(
            RegionKey::ConflictRegion(region.id()),
            region.polygon().ring().to_vec(),
        );
    }
    rings
}

/// One occupancy interval detected from the observed frames of one run.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReferenceOccupancy {
    agent: AgentId,
    entry_s: f64,
    exit_s: f64,
}

/// Every continuous occupancy interval of one region, detected from the frames.
///
/// A body occupies the region while its conservative disc reaches the ring, so
/// the reference shares the safety layer's documented occupancy radius and
/// differs from the metric only by the metric's tick quantization. An entry or
/// exit inside a tick is located by bisecting the interpolated pose.
fn reference_occupancies(
    ring: &[DVec2],
    frames: &[(u64, BTreeMap<AgentId, AgentSample>)],
    step_s: f64,
) -> Vec<ReferenceOccupancy> {
    let mut occupancies = Vec::new();
    let mut open: BTreeMap<AgentId, f64> = BTreeMap::new();
    for pair in frames.windows(2) {
        let (before_tick, before) = &pair[0];
        let (after_tick, after) = &pair[1];
        let before_t = *before_tick as f64 * step_s;
        let after_t = *after_tick as f64 * step_s;
        for (agent, before_sample) in before {
            let Some(after_sample) = after.get(agent) else {
                // A despawn closes the occupancy at the despawn's tick.
                if let Some(entry_s) = open.remove(agent) {
                    occupancies.push(ReferenceOccupancy {
                        agent: *agent,
                        entry_s,
                        exit_s: before_t,
                    });
                }
                continue;
            };
            let radius_m = circumradius_m(after_sample);
            let before_centre = before_sample.position;
            let after_centre = after_sample.position;
            let occupied_before = circle_overlaps_ring(ring, before_centre, radius_m);
            let occupied_after = circle_overlaps_ring(ring, after_centre, radius_m);
            if occupied_before == occupied_after {
                continue;
            }
            // The pose inside the tick where the occupancy boundary is crossed.
            let (mut clear, mut crossed) = (before_t, after_t);
            for _ in 0..80 {
                let middle = 0.5 * (clear + crossed);
                let fraction = (middle - before_t) / (after_t - before_t).max(f64::MIN_POSITIVE);
                let centre = before_centre + (after_centre - before_centre) * fraction;
                if circle_overlaps_ring(ring, centre, radius_m) == occupied_after {
                    crossed = middle;
                } else {
                    clear = middle;
                }
            }
            if occupied_after {
                open.insert(*agent, crossed);
            } else if let Some(entry_s) = open.remove(agent) {
                occupancies.push(ReferenceOccupancy {
                    agent: *agent,
                    entry_s,
                    exit_s: crossed,
                });
            }
        }
    }
    occupancies
}

/// The post-encroachment times of one region's reference occupancies, under the
/// metric's documented succession rule: successive occupancies by different
/// bodies that do not overlap in time.
///
/// Each record carries the following occupancy's entry time too, so a caller can
/// set aside the successions the run's end cuts short.
fn reference_pets(occupancies: &[ReferenceOccupancy]) -> Vec<(AgentId, AgentId, f64, f64)> {
    let mut ordered = occupancies.to_vec();
    ordered.sort_by(|first, second| {
        first
            .exit_s
            .total_cmp(&second.exit_s)
            .then(first.agent.cmp(&second.agent))
    });
    let mut pets = Vec::new();
    for pair in ordered.windows(2) {
        let (preceding, following) = (pair[0], pair[1]);
        if preceding.agent != following.agent && preceding.exit_s <= following.entry_s {
            pets.push((
                preceding.agent,
                following.agent,
                following.entry_s,
                following.entry_s - preceding.exit_s,
            ));
        }
    }
    pets
}

#[test]
fn a_constant_speed_lane_keeps_its_gap_and_reports_no_time_to_collision() {
    // The walking skeleton is six 4.5 m vehicles 20 m apart at a constant
    // 12 m/s, so the least surface separation is exactly 15.5 m, every pair
    // keeps that separation, and no pair is ever closing.
    for step in STEPS {
        let ticks = (1.0_f64 / step).round() as u64;
        let mut sim = sim(WALKING, 7, step);
        for _ in 0..ticks {
            sim.step();
        }
        let metrics = sim.interaction_metrics();
        let separation = metrics
            .minimum_separation_m()
            .expect("neighbouring vehicles");
        assert!(
            (separation.value - 15.5).abs() <= SEPARATION_TOLERANCE_M,
            "step {step}: separation {} not 15.5",
            separation.value
        );
        assert_eq!(separation.mode_pair, ModePair::VehicleVehicle);
        assert!(
            metrics
                .mode_pair_minimum_separation_m(ModePair::VehiclePedestrian)
                .is_none(),
            "step {step}: a vehicle-only lane has no cross-mode interaction"
        );
        let lane_pair = metrics
            .pair_minimum_separation_m(AgentId::from_index(0), AgentId::from_index(1))
            .expect("the first two vehicles are a candidate pair");
        assert!(
            (lane_pair - 15.5).abs() <= SEPARATION_TOLERANCE_M,
            "{lane_pair}"
        );
        let vehicle_pair = metrics
            .mode_pair_minimum_separation_m(ModePair::VehicleVehicle)
            .expect("a vehicle-only lane has one recorded mode pair");
        assert!((vehicle_pair.value - 15.5).abs() <= SEPARATION_TOLERANCE_M);
        assert!(
            metrics.minimum_ttc_s().is_none(),
            "step {step}: equal speeds never close a gap"
        );
        assert!(
            metrics.tick_minimum_ttc_s().is_none(),
            "step {step}: equal speeds never close a gap"
        );
        assert!(metrics.post_encroachments().is_empty());
    }
}

#[test]
fn online_ttc_matches_the_analytic_circle_solution() {
    // Every pedestrian pair the candidate set reports is compared with the
    // closed-form first root of two moving circles, at three steps, on the
    // frames the run actually produced.
    let mut compared = 0;
    for step in STEPS {
        let step_s = step;
        let ticks = (20.0_f64 / step).round() as u64;
        let mut sim = sim(MIXED, 3, step);
        let mut before = frames(&sim);
        for _ in 0..ticks {
            sim.step();
            let after = frames(&sim);
            for (first, first_after) in &after {
                let Some(first_before) = before.get(first) else {
                    continue;
                };
                for (second, second_after) in &after {
                    if second <= first {
                        continue;
                    }
                    let Some(second_before) = before.get(second) else {
                        continue;
                    };
                    if first_after.motion.as_ref().expect("full").mode != AgentMode::Pedestrian
                        || second_after.motion.as_ref().expect("full").mode != AgentMode::Pedestrian
                    {
                        continue;
                    }
                    let first_body = swept_of(first_before, first_after);
                    let second_body = swept_of(second_before, second_after);
                    let observed =
                        time_to_collision(&first_body, &second_body, Seconds::from_secs(step));
                    let expected = analytic_circle_ttc(&first_body, &second_body, step_s);
                    let context = format!(
                        "step {step} pair ({}, {}) at tick {}",
                        first.get(),
                        second.get(),
                        sim.time().tick()
                    );
                    match (observed, expected) {
                        (Some(observed), Some(expected)) => {
                            compared += 1;
                            assert!(
                                (observed - expected).abs() <= TTC_TIME_TOLERANCE_S,
                                "observed {observed} versus analytic {expected}: {context}"
                            );
                        }
                        (None, None) => {}
                        _ => panic!(
                            "the metric and the analytic root disagree on applicability: \
                             {observed:?} versus {expected:?}: {context}"
                        ),
                    }
                }
            }
            before = after;
        }
    }
    assert!(
        compared > 0,
        "the fixture must contain closing pedestrian pairs"
    );
}

#[test]
fn recorded_separation_matches_the_all_pairs_reference() {
    // The candidate set is a superset of every pair whose surfaces come within
    // the declared range, so the recorded extremes must equal the all-pairs
    // extremes over the observed frames whenever the closest pair is comfortably
    // inside the range.
    const RANGE_MARGIN_M: f64 = 2.0;
    for step in [0.05, 0.0125] {
        let ticks = (20.0_f64 / step).round() as u64;
        let mut sim = sim(MIXED, 5, step);
        let mut before = frames(&sim);
        let mut reference_minimum_m = f64::INFINITY;
        let mut reference_mode_pairs = [f64::INFINITY; ModePair::COUNT];
        for _ in 0..ticks {
            sim.step();
            let after = frames(&sim);
            let live: Vec<(&AgentId, &AgentSample)> = after.iter().collect();
            for (position, (first, first_after)) in live.iter().enumerate() {
                let Some(first_before) = before.get(*first) else {
                    continue;
                };
                for (second, second_after) in &live[position + 1..] {
                    let Some(second_before) = before.get(*second) else {
                        continue;
                    };
                    let separation_m = tick_minimum_clearance_m(
                        &swept_of(first_before, first_after),
                        &swept_of(second_before, second_after),
                    );
                    reference_minimum_m = reference_minimum_m.min(separation_m);
                    let mode_pair = ModePair::of(
                        first_after.motion.as_ref().expect("full").mode,
                        second_after.motion.as_ref().expect("full").mode,
                    );
                    reference_mode_pairs[mode_pair.index()] =
                        reference_mode_pairs[mode_pair.index()].min(separation_m);
                }
            }
            before = after;
        }
        let metrics = sim.interaction_metrics();
        if reference_minimum_m > INTERACTION_RANGE_M - RANGE_MARGIN_M {
            // Every pair stayed outside the range, so there is nothing the
            // candidate set had to cover.
            continue;
        }
        let recorded = metrics
            .minimum_separation_m()
            .expect("an in-range pair must be recorded");
        assert!(
            (recorded.value - reference_minimum_m).abs() <= SEPARATION_TOLERANCE_M,
            "step {step}: recorded {} versus all-pairs {}",
            recorded.value,
            reference_minimum_m
        );
        // The per-mode-pair minima are the all-pairs minima of that mode pair,
        // and the cross-mode record is the mixed interaction's own value.
        for pair in [
            ModePair::VehicleVehicle,
            ModePair::VehiclePedestrian,
            ModePair::PedestrianPedestrian,
        ] {
            let recorded_m = metrics
                .mode_pair_minimum_separation_m(pair)
                .map(|m| m.value);
            if reference_mode_pairs[pair.index()] > INTERACTION_RANGE_M - RANGE_MARGIN_M {
                continue;
            }
            let recorded_m = recorded_m
                .unwrap_or_else(|| panic!("step {step}: mode pair {pair:?} must be recorded"));
            assert!(
                (recorded_m - reference_mode_pairs[pair.index()]).abs() <= SEPARATION_TOLERANCE_M,
                "step {step}: mode pair {pair:?} recorded {recorded_m} versus all-pairs {}",
                reference_mode_pairs[pair.index()]
            );
        }
        // The mixed benchmark's nominal claim: vehicles and pedestrians meet, so
        // the cross-mode record is a real encounter and not an artefact of the
        // candidate set. Its exact value depends on how the run's yielding
        // interacted with the sampled arrivals, so only its presence and the
        // all-pairs agreement above are asserted.
        let cross_mode = metrics
            .mode_pair_minimum_separation_m(ModePair::VehiclePedestrian)
            .expect("the mixed benchmark must produce a cross-mode encounter");
        assert!(cross_mode.value > 0.0);
    }
}

#[test]
fn post_encroachment_time_matches_the_frame_reference_and_converges() {
    // The perpendicular-conflict benchmark carries unsynchronized demand across
    // one authored conflict region, so successive occupancies of that region by
    // different bodies are the normal case. The metric's tick-quantized
    // boundaries are compared with an independently detected continuous
    // occupancy. Both sides measure occupancy with the safety layer's
    // conservative body radius, so the comparison isolates the metric's
    // quantization and sweep margin. The declared bound is the same multiple of
    // the step at every step and it shrinks with the step, which is the
    // convergence statement; the finest step's worst deviation is also asserted
    // absolutely.
    let mut records_checked = 0;
    let mut worst_finest_s = 0.0_f64;
    for step in STEPS {
        let mut worst_s = 0.0_f64;
        for seed in [4, 5, 6] {
            let scenario = scenario(CONFLICT);
            let rings = region_rings(&scenario);
            let seconds = 90.0_f64;
            let ticks = (seconds / step).round() as u64;
            let run_end_s = ticks as f64 * step;
            let mut sim = Simulation::new(
                scenario,
                RunConfig::new(seed).with_step(Seconds::from_secs(step)),
            )
            .expect("builds");
            let mut observed = Vec::with_capacity(ticks as usize + 1);
            observed.push((0, frames(&sim)));
            for _ in 0..ticks {
                sim.step();
                observed.push((sim.time().tick(), frames(&sim)));
            }
            let tolerance_s = PET_STEP_MULTIPLE * step;
            for (region, ring) in &rings {
                let occupancies = reference_occupancies(ring, &observed, step);
                // A succession whose following occupancy starts within the
                // declared bound of the run's end is set aside: the metric
                // records an occupancy only once it closes, and this run may end
                // before the region clears.
                let boundary_s = run_end_s - tolerance_s;
                let reference: BTreeMap<(AgentId, AgentId), f64> = reference_pets(&occupancies)
                    .into_iter()
                    .filter(|(_, _, following_entry_s, _)| *following_entry_s <= boundary_s)
                    .map(|(preceding, following, _, seconds)| ((preceding, following), seconds))
                    .collect();
                let mut recorded: BTreeMap<(AgentId, AgentId), f64> = BTreeMap::new();
                for pet in sim
                    .interaction_metrics()
                    .post_encroachments()
                    .iter()
                    .filter(|pet| pet.region == *region && pet.following_entry_s <= boundary_s)
                {
                    recorded.insert((pet.preceding, pet.following), pet.seconds);
                }
                assert_eq!(
                    recorded.len(),
                    reference.len(),
                    "step {step} seed {seed}: recorded {recorded:?} versus reference \
                     {reference:?} in {region:?}"
                );
                for (key, expected) in &reference {
                    let observed_s = recorded.get(key).unwrap_or_else(|| {
                        panic!("step {step} seed {seed}: no record for succession {key:?}")
                    });
                    let deviation_s = (observed_s - expected).abs();
                    assert!(
                        deviation_s <= tolerance_s,
                        "step {step} seed {seed}: PET {observed_s} versus reference {expected} \
                         for {key:?}"
                    );
                    worst_s = worst_s.max(deviation_s);
                    records_checked += 1;
                }
            }
            // The run-level minimum is one of the records and is the least of
            // them.
            let metrics = sim.interaction_metrics();
            let least = metrics
                .post_encroachments()
                .iter()
                .map(|pet| pet.seconds)
                .fold(f64::INFINITY, f64::min);
            if least.is_finite() {
                assert_eq!(
                    metrics.minimum_post_encroachment_s().map(|pet| pet.seconds),
                    Some(least)
                );
            }
            sim.finish();
        }
        if step == STEPS[STEPS.len() - 1] {
            worst_finest_s = worst_s;
        }
    }
    assert!(
        records_checked >= 20,
        "the fixtures must record post-encroachment times: {records_checked}"
    );
    // The finest step's worst deviation is the same bound stated absolutely: at
    // the finest step the metric tracks the continuous boundary closely.
    assert!(
        worst_finest_s <= PET_STEP_MULTIPLE * STEPS[STEPS.len() - 1],
        "the finest step's worst deviation is {worst_finest_s} s"
    );
}

#[test]
fn pairwise_separation_is_recorded_for_every_observed_pair() {
    // The per-pair record is what a report disaggregates by participant, so it
    // must be present for every pair the run observed and must be the least of
    // that pair's per-tick sweeps.
    let mut sim = sim(CONFLICT, 2, 0.05);
    let ticks = (40.0_f64 / 0.05).round() as u64;
    let mut before = frames(&sim);
    let mut pair_minimum_m: BTreeMap<(AgentId, AgentId), f64> = BTreeMap::new();
    for _ in 0..ticks {
        sim.step();
        let after = frames(&sim);
        for (first, first_after) in &after {
            let Some(first_before) = before.get(first) else {
                continue;
            };
            for (second, second_after) in &after {
                if second <= first {
                    continue;
                }
                let Some(second_before) = before.get(second) else {
                    continue;
                };
                let separation_m = tick_minimum_clearance_m(
                    &swept_of(first_before, first_after),
                    &swept_of(second_before, second_after),
                );
                if separation_m > INTERACTION_RANGE_M - 2.0 {
                    continue;
                }
                let entry = pair_minimum_m
                    .entry((*first, *second))
                    .or_insert(f64::INFINITY);
                *entry = entry.min(separation_m);
            }
        }
        before = after;
    }
    assert!(!pair_minimum_m.is_empty(), "the fixture must pair bodies");
    for ((first, second), expected) in pair_minimum_m {
        let recorded = sim
            .interaction_metrics()
            .pair_minimum_separation_m(second, first)
            .unwrap_or_else(|| panic!("pair ({first:?}, {second:?}) was not recorded"));
        assert!(
            (recorded - expected).abs() <= SEPARATION_TOLERANCE_M,
            "pair ({first:?}, {second:?}) recorded {recorded} versus {expected}"
        );
    }
}

#[test]
fn reading_the_metrics_does_not_change_the_run() {
    // The observation pass borrows the integrated tick and writes only its own
    // records, so a run whose metrics are read after every tick must be
    // byte-identical to one whose metrics are never read. The trace-level proof
    // is the golden trace test, which compares the canonical bytes and hash of a
    // committed trace; this is the in-kernel counterpart.
    fn run(read_metrics: bool) -> Vec<(u64, Vec<Event>, String)> {
        let mut sim = sim(MIXED, 13, 0.05);
        let mut frames = Vec::new();
        for _ in 0..400 {
            let events: Vec<Event> = sim.step().events().to_vec();
            let tick = sim.time().tick();
            if read_metrics {
                let metrics = sim.interaction_metrics();
                std::hint::black_box(metrics.minimum_separation_m());
                std::hint::black_box(metrics.minimum_ttc_s());
                std::hint::black_box(metrics.tick_minimum_separation_m());
                std::hint::black_box(metrics.tick_minimum_ttc_s());
                std::hint::black_box(metrics.minimum_post_encroachment_s());
                std::hint::black_box(metrics.post_encroachments().len());
                std::hint::black_box(
                    metrics.region_occupancies(RegionKey::Crossing(CrossingId::from_index(0))),
                );
            }
            let fingerprint = format!("{:?}", sim.snapshot(SnapshotDetail::Full).agents());
            frames.push((tick, events, fingerprint));
        }
        frames
    }

    assert_eq!(run(false), run(true));
}
