//! The Bevy backend opens the checked-in Increment 2 fixtures.
//!
//! `hekate-viewer`'s `CurrentFrame` is the Bevy graphics backend: it captures
//! each projected `SceneFrame` and the ECS systems draw from that capture alone.
//! This suite drives the two Increment 2 fixtures through the same version-2
//! loader the viewer uses, feeds the shared projection to that backend, and
//! asserts the capture is the frame carrying the route-relative overlays — and
//! that the viewer's own `body_visuals` shape path resolves every body of the
//! run. The exact overlay values are pinned by
//! `crates/hekate-present/tests/inc2_fixture_overlays.rs`'s golden, which the
//! terminal suite also renders.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::DVec2;
use hekate_model::CompiledScenario;
use hekate_present::{
    BodyShape, PresentationController, RendererBackend, SceneFrame, SceneGeometry, Viewport,
    load_scenario,
};
use hekate_sim::{AgentId, RunConfig, Simulation, SnapshotDetail};
use hekate_viewer::{CurrentFrame, body_visuals};

const SEED: u64 = 0;
/// The pinned Standard step both fixtures run at.
const STEP_SECS: f64 = 0.05;
/// A tick inside the passing fixture's one overtaking interval, which the
/// shared golden pins as opening at tick 712.
const PASS_TICK: u64 = 800;
/// Ticks that place the occupied corridor's adjacent pair, record the entry,
/// and hold the interval the encounter leaves open.
const WRONG_WAY_TICKS: u64 = 600;
/// Ticks the Phase 1 absence run steps: five seconds, while the scenario's
/// population is still on its guide paths.
const PHASE_1_TICKS: u64 = 100;

/// The passing fixture, relative to the repository root.
const PASSING_FIXTURE: &str = "scenarios/phase2/inc2/narrow_passing_v2.json5";
/// The contextual wrong-way fixture, relative to the repository root.
const WRONG_WAY_FIXTURE: &str = "scenarios/phase2/inc2/narrow_wrong_way_v2.json5";
/// The Phase 1 scenario: no facility, so no route state and no overlay.
const PHASE_1_FIXTURE: &str = "scenarios/walking/walking_guide_v1.json5";

/// The outbound guide path of the fixture's occupied opposing corridor (`d`).
const OCCUPIED_GUIDE: &str = "guide_d";
/// The leader arc-length and bumper gap windows the TAS-131 driver turns its
/// rider in, in metres.
const OCCUPIED_LEAD_WINDOW: (f64, f64) = (25.0, 48.0);
const OCCUPIED_GAP_WINDOW: (f64, f64) = (8.0, 18.0);

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn load(relative: &str) -> Arc<CompiledScenario> {
    Arc::new(
        load_scenario(&repo_path(relative))
            .unwrap_or_else(|error| panic!("{relative} did not load: {error}")),
    )
}

/// Drive `scenario` for `ticks` steps, feeding every projected frame to the
/// Bevy backend and returning the frame it captured last.
fn draw_into_bevy(
    scenario: &Arc<CompiledScenario>,
    ticks: u64,
    mut request: impl FnMut(&mut Simulation) -> bool,
) -> SceneFrame {
    let mut sim = Simulation::new((**scenario).clone(), RunConfig::new(SEED))
        .expect("the fixture builds a simulation");
    let mut controller = PresentationController::new(
        SceneGeometry::from_scenario(scenario),
        STEP_SECS,
        Viewport::new(DVec2::ZERO, 1.0),
    );
    let mut backend = CurrentFrame::default();
    let mut previous = sim.snapshot(SnapshotDetail::Full);
    let mut pending = true;
    for _ in 0..ticks {
        let step = sim.step();
        let tick = step.time().tick();
        controller.observe_events(tick, step.events());
        if pending {
            pending = !request(&mut sim);
        }
        let current = sim.snapshot(SnapshotDetail::Full);
        let frame = controller.project(&previous, &current);
        backend
            .draw(&frame)
            .expect("the Bevy backend captures the frame");
        assert_eq!(
            backend.get(),
            Some(&frame),
            "the Bevy backend must capture exactly the projected frame"
        );
        previous = current;
    }
    backend.get().expect("the backend captured a frame").clone()
}

/// Every body of `frame` resolves to at least one visual, and a capsule body
/// draws the rectangle and two cap circles the shared shape decision describes.
fn assert_every_body_draws(frame: &SceneFrame) {
    assert!(
        !frame.bodies.is_empty(),
        "the fixture put no body on the road"
    );
    for body in &frame.bodies {
        let visuals = body_visuals(body);
        assert!(
            !visuals.is_empty(),
            "body {} resolved to no visual",
            body.id
        );
        let capsule = body
            .shapes()
            .iter()
            .any(|shape| matches!(shape, BodyShape::Capsule { .. }));
        if capsule {
            assert_eq!(
                visuals.len(),
                3,
                "a capsule body draws its rectangle and two caps"
            );
        }
    }
}

/// The passing fixture's frames reach the Bevy backend with the corridor,
/// target offset, predicted gap, and maneuver overlays the shared layer
/// derives.
#[test]
fn the_bevy_backend_captures_the_passing_fixtures_route_relative_overlays() {
    let scenario = load(PASSING_FIXTURE);
    assert_eq!(
        scenario.schema_version(),
        2,
        "the fixture is a version 2 run"
    );
    let frame = draw_into_bevy(&scenario, PASS_TICK, |_| false);
    assert_eq!(frame.tick, PASS_TICK);

    let maneuver = frame
        .maneuver_overlays()
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            panic!("the passing fixture must carry its overtaking interval at tick {PASS_TICK}")
        });
    let agent = maneuver.agent();
    assert!(
        frame
            .corridors()
            .iter()
            .any(|overlay| overlay.agent() == agent),
        "the maneuvering body carries its usable corridor"
    );
    assert!(
        frame
            .target_offsets()
            .iter()
            .any(|overlay| overlay.agent() == agent),
        "the maneuvering body carries its fixed target offset"
    );
    assert!(
        frame
            .predicted_gaps()
            .iter()
            .any(|overlay| overlay.agent() == agent),
        "the maneuvering body carries its predicted gap"
    );
    assert!(
        frame.wrong_way_overlays().is_empty(),
        "the passing fixture authors no opposing traversal"
    );
    assert_every_body_draws(&frame);
}

/// The occupied opposing fixture's frames reach the Bevy backend with the
/// wrong-way rule state of the turned rider.
#[test]
fn the_bevy_backend_captures_the_occupied_opposing_fixtures_wrong_way_state() {
    let scenario = load(WRONG_WAY_FIXTURE);
    let guide = scenario
        .paths()
        .iter()
        .position(|path| path.name() == OCCUPIED_GUIDE)
        .unwrap_or_else(|| panic!("the fixture authors a guide path '{OCCUPIED_GUIDE}'"));
    let frame = draw_into_bevy(&scenario, WRONG_WAY_TICKS, move |sim| {
        let Some(lead) = occupied_lead(sim, guide) else {
            return false;
        };
        assert!(
            sim.request_wrong_way_entry(lead),
            "the occupied corridor's leading rider can request the entry"
        );
        true
    });

    let wrong_way = frame
        .wrong_way_overlays()
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            panic!("the occupied opposing fixture must carry its open wrong-way interval")
        });
    assert!(
        frame
            .corridors()
            .iter()
            .any(|overlay| overlay.agent() == wrong_way.agent()),
        "the turned rider carries the occupied corridor's band"
    );
    assert_eq!(
        frame.tick, WRONG_WAY_TICKS,
        "the interval the encounter leaves open is the one the run end closes"
    );
    assert_every_body_draws(&frame);
}

/// A Phase 1 run captures no route-relative overlay: the Bevy backend draws the
/// same bodies and geometry it always did.
#[test]
fn the_bevy_backend_captures_no_route_relative_overlay_for_a_phase_1_run() {
    let scenario = load(PHASE_1_FIXTURE);
    assert_eq!(
        scenario.schema_version(),
        1,
        "the Phase 1 fixture stays a version 1 document"
    );
    let frame = draw_into_bevy(&scenario, PHASE_1_TICKS, |_| false);
    assert!(frame.corridors().is_empty());
    assert!(frame.target_offsets().is_empty());
    assert!(frame.predicted_gaps().is_empty());
    assert!(frame.maneuver_overlays().is_empty());
    assert!(frame.wrong_way_overlays().is_empty());
    assert_every_body_draws(&frame);
}

/// The leading body of the occupied opposing corridor's nearest adjacent pair,
/// or `None` while the corridor's two demand sources have not placed one.
fn occupied_lead(sim: &Simulation, guide: usize) -> Option<AgentId> {
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let mut bodies: Vec<(AgentId, f64, f64)> = snapshot
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            (motion.path.index() == guide).then_some((
                sample.id,
                motion.path_distance_m,
                motion.body_length_m,
            ))
        })
        .collect();
    bodies.sort_by(|left, right| left.1.total_cmp(&right.1));
    bodies.windows(2).find_map(|pair| {
        let (occupancy, lead) = (pair[0], pair[1]);
        let gap_m = lead.1 - occupancy.1 - (lead.2 + occupancy.2) * 0.5;
        let in_window = (OCCUPIED_LEAD_WINDOW.0..=OCCUPIED_LEAD_WINDOW.1).contains(&lead.1)
            && (OCCUPIED_GAP_WINDOW.0..=OCCUPIED_GAP_WINDOW.1).contains(&gap_m);
        in_window.then_some(lead.0)
    })
}
