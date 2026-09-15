//! Safety-overlay projection over a real event stream.
//!
//! The overlays a renderer draws must be a pure function of the projected
//! frame: the same run must project byte-identical markers, emphasis, and
//! occupancy, and every derived value must be supported by the records the
//! frame carries. These tests drive the shared controller over a checked-in
//! benchmark and assert exactly that, with no GUI.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec2;
use hekate_model::CompiledScenario;
use hekate_present::{
    EventParticipants, PresentationController, SceneFrame, SceneGeometry, Viewport, load_scenario,
};
use hekate_sim::{EventKind, RunConfig, Simulation, SnapshotDetail};

/// A seed of the mixed benchmark whose 800 ticks exercise every record family
/// except the collision family; collisions are rare enough that the benchmark
/// sweep is their evidence, not this test.
const SEED: u64 = 3;
const TICKS: u64 = 800;
const STEP_SECS: f64 = 0.05;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn mixed() -> Arc<CompiledScenario> {
    let path = repo_path("scenarios/benchmarks/mixed_interaction_v1.json5");
    Arc::new(load_scenario(&path).expect("mixed benchmark loads"))
}

/// Everything a backend draws from one frame's safety overlays.
fn describe(frame: &SceneFrame) -> String {
    let markers: Vec<String> = frame
        .safety_markers()
        .into_iter()
        .map(|marker| {
            format!(
                "{}@{:.6},{:.6}",
                marker.kind() as u8,
                marker.position().x,
                marker.position().y
            )
        })
        .collect();
    let emphasis: Vec<String> = frame
        .body_emphasis()
        .into_iter()
        .map(|(agent, emphasis)| format!("{agent}:{}", emphasis.label()))
        .collect();
    let occupancy: Vec<String> = frame
        .occupied_regions()
        .iter()
        .map(|region| {
            let occupants: Vec<String> = region.occupants().iter().map(usize::to_string).collect();
            format!("{}[{}]", region.region().get(), occupants.join(","))
        })
        .collect();
    format!(
        "tick {} markers [{}] emphasis [{}] occupancy [{}]",
        frame.tick,
        markers.join(" "),
        emphasis.join(" "),
        occupancy.join(" ")
    )
}

/// Run the benchmark, folding every step's records into the controller, and
/// return one projection description per tick plus every record observed.
fn observe(ticks: u64) -> (Vec<String>, Vec<EventKind>) {
    let scenario = mixed();
    let mut sim = Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("builds");
    let geometry = SceneGeometry::from_scenario(&scenario);
    let mut controller =
        PresentationController::new(geometry, STEP_SECS, Viewport::new(DVec2::ZERO, 1.0));

    let mut observed = Vec::new();
    let mut described = Vec::new();
    let mut previous = sim.snapshot(SnapshotDetail::Full);
    for _ in 0..ticks {
        let output = sim.step();
        controller.observe_events(output.time().tick(), output.events());
        observed.extend(output.events().iter().map(|event| event.kind()));
        let current = sim.snapshot(SnapshotDetail::Full);
        let frame = controller.project(&previous, &current);
        described.push(describe(&frame));
        previous = current;
    }
    (described, observed)
}

#[test]
fn the_same_run_projects_byte_identical_safety_overlays() {
    let (first, observed) = observe(TICKS);
    let (second, _) = observe(TICKS);
    assert_eq!(first, second, "the overlay projection is not deterministic");

    // The window must actually contain records, or the test proves nothing.
    let kinds: BTreeSet<EventKind> = observed.into_iter().collect();
    for kind in [
        EventKind::NearMiss,
        EventKind::Entry,
        EventKind::Exit,
        EventKind::Queue,
        EventKind::ControlTransition,
    ] {
        assert!(kinds.contains(&kind), "no {kind:?} record in {TICKS} ticks");
    }
    assert!(
        first.iter().any(|frame| frame.contains("near_miss")),
        "no frame emphasized a near miss"
    );
    assert!(
        first.iter().any(|frame| frame.contains("queue")),
        "no frame emphasized a standing body"
    );
    assert!(
        first.iter().any(|frame| frame.contains('[')),
        "no frame carried an occupancy"
    );
}

#[test]
fn every_derived_overlay_is_supported_by_the_frame() {
    let scenario = mixed();
    let mut sim = Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("builds");
    let geometry = SceneGeometry::from_scenario(&scenario);
    let mut controller =
        PresentationController::new(geometry, STEP_SECS, Viewport::new(DVec2::ZERO, 1.0));

    let mut markers_seen = 0_usize;
    let mut occupancy_seen = 0_usize;
    let mut linked = 0_usize;
    let mut emphasis_labels: BTreeMap<&str, usize> = BTreeMap::new();
    let mut modes: BTreeSet<&str> = BTreeSet::new();
    let mut previous = sim.snapshot(SnapshotDetail::Full);
    for _ in 0..TICKS {
        let output = sim.step();
        controller.observe_events(output.time().tick(), output.events());
        let current = sim.snapshot(SnapshotDetail::Full);
        let frame = controller.project(&previous, &current);
        previous = current;

        // A marker is anchored on its region's centre or on the bodies it
        // links, and every marker is backed by a record the frame carries.
        for marker in frame.safety_markers() {
            markers_seen += 1;
            let participants = marker.participants();
            assert!(
                !participants.agents().is_empty(),
                "marker {marker:?} names no body"
            );
            assert!(
                frame.safety.events().contains(marker.record()),
                "marker {marker:?} is not in the frame's window"
            );
            if let Some(region) = participants.region() {
                assert_eq!(
                    Some(marker.position()),
                    frame.geometry.region_center(region),
                    "a region marker sits at its region centre"
                );
                assert!(frame.region_points(region).is_some());
            } else {
                let anchors: Vec<DVec2> = participants
                    .agents()
                    .iter()
                    .filter_map(|agent| frame.body(*agent).map(|body| body.position))
                    .collect();
                assert!(!anchors.is_empty(), "marker {marker:?} has no live body");
                let (min, max) = anchors.iter().fold(
                    (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
                    |(min, max), point| (min.min(*point), max.max(*point)),
                );
                assert!(
                    marker.position().cmpge(min).all() && marker.position().cmple(max).all(),
                    "marker {marker:?} is outside its participants"
                );
            }
        }

        // Emphasis names live bodies, at most once each, ascending by agent.
        let mut last: Option<usize> = None;
        for (agent, emphasis) in frame.body_emphasis() {
            assert!(frame.body(agent).is_some(), "emphasis on a dead body");
            assert!(last < Some(agent), "emphasis is not ascending by agent");
            last = Some(agent);
            *emphasis_labels.entry(emphasis.label()).or_default() += 1;

            // An inspector link resolves to the records the frame carries, and
            // a record's other participants are bodies the frame draws or
            // records. A queue or controller state outlives the marker window,
            // so an emphasized body need not have a link.
            for link in frame.events_involving(agent) {
                linked += 1;
                let participants = EventParticipants::of(link.event());
                assert!(participants.agents().contains(&agent));
                assert!(frame.safety.events().contains(&link));
                for other in participants.others(agent) {
                    assert!(
                        frame.body(other).is_some()
                            || frame.safety.events().iter().any(|record| {
                                EventParticipants::of(record.event())
                                    .agents()
                                    .contains(&other)
                            }),
                        "link target {other} is neither drawn nor recorded"
                    );
                }
            }
        }

        // Occupancy names regions the geometry has and bodies that are alive,
        // ascending by region key.
        occupancy_seen += frame.occupied_regions().len();
        assert!(
            frame
                .occupied_regions()
                .windows(2)
                .all(|pair| pair[0].region() < pair[1].region()),
            "occupancy is not ascending by region"
        );
        for region in frame.occupied_regions() {
            assert!(frame.region_points(region.region()).is_some());
            assert!(!region.occupants().is_empty());
            assert!(
                region.occupants().windows(2).all(|pair| pair[0] < pair[1]),
                "occupants are not ascending"
            );
            for agent in region.occupants() {
                assert!(frame.body(*agent).is_some(), "occupant {agent} is dead");
            }
        }

        for body in &frame.bodies {
            modes.insert(body.mode.label());
        }
    }

    assert!(markers_seen > 0, "no markers were projected");
    assert!(occupancy_seen > 0, "no occupancy was projected");
    assert!(linked > 0, "no frame carried an inspector link");
    assert!(
        emphasis_labels.contains_key("near_miss") && emphasis_labels.contains_key("queue"),
        "expected near-miss and queue emphasis, saw {emphasis_labels:?}"
    );
    assert_eq!(
        modes,
        BTreeSet::from(["pedestrian", "vehicle"]),
        "both modes project through the same frames"
    );
}
