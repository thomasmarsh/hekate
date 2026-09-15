//! The checked-in Increment 2 fixtures through the shared presentation layer.
//!
//! `hekate_present::load_scenario` is the only load path either viewer uses, so
//! the two fixtures this file opens are exactly the runs both backends render:
//! `scenarios/phase2/inc2/narrow_passing_v2.json5`, Increment 2's passing
//! reference, and `scenarios/phase2/inc2/narrow_wrong_way_v2.json5`, whose
//! occupied opposing corridor is the wrong-way case. Both open as native
//! version-2 documents, project their facilities, and drive the frame's five
//! route-relative overlays from the run's own records and samples:
//!
//! - the passing fixture carries the usable corridor, the target offset, the
//!   predicted gap, and the maneuver interval of the one executed `pass` its own
//!   demand and eligibility tactics produce, and no opposing traversal;
//! - the occupied opposing corridor carries the usable corridor and the
//!   wrong-way rule state, opened by the entry request the kernel evaluates on
//!   the pair the fixture's two demand sources place — the same request the
//!   TAS-131 driver records, because a checked-in scenario authors none.
//!
//! Three properties are asserted here. First, the five primitives are present
//! with the identifiers the contract names, across the two runs. Second, each
//! fixture's projection matches the checked-in golden frame for frame, so a
//! replayed overlay stream is byte-identical to the recorded one. Third, a
//! Phase 1 run carries none of the five, and an Increment 1 run carries none of
//! the four the sparse or interval families need. (An Increment 1 version-2 body
//! does carry route state on a compiled band, so the corridor derived from that
//! sample is present; that is the corridor primitive's own contract, not a leak,
//! and its inspector carries no target, gap, maneuver, or rule field.)
//!
//! The exact ticks and inspector text are pinned by
//! `tests/golden/present/inc2_tactical_fixtures.seed0.txt`, which this suite
//! writes with `UPDATE_GOLDENS=1 cargo test -p hekate-present --test
//! inc2_fixture_overlays`; inspect that diff before committing it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::DVec2;
use hekate_model::{
    CompiledScenario, FacilityId, MovementDirection, NominalDirection, PermissionEffect, TacticKind,
};
use hekate_present::{
    PresentationController, SceneFrame, SceneGeometry, Viewport, corridor_summary, load_scenario,
    maneuver_summary, predicted_gap_summary, target_offset_summary, wrong_way_summary,
};
use hekate_sim::{
    AgentId, ManeuverEdge, ManeuverReasonCode, PassSide, RunConfig, Simulation, SnapshotDetail,
    WrongWayReason,
};

/// The pinned benchmark seed the fixtures' schedules are documented at.
const SEED: u64 = 0;
/// The pinned Standard step the benchmark matrix runs every fixture at.
const STEP_SECS: f64 = 0.05;
/// Ticks that carry the passing fixture from its first corridor, through the
/// one overtaking interval its sparse demand produces, to the run end.
const PASSING_TICKS: u64 = 1900;
/// Ticks that place the occupied corridor's adjacent pair, record the entry,
/// and hold the interval the encounter leaves open to the run end.
const WRONG_WAY_TICKS: u64 = 600;
/// How long the Phase 1 and Increment 1 absence runs step, so a late interval
/// would still be observed.
const ABSENCE_TICKS: u64 = 400;

/// The passing fixture, relative to the repository root.
const PASSING_FIXTURE: &str = "scenarios/phase2/inc2/narrow_passing_v2.json5";
/// The contextual wrong-way fixture, relative to the repository root.
const WRONG_WAY_FIXTURE: &str = "scenarios/phase2/inc2/narrow_wrong_way_v2.json5";
/// The Phase 1 scenario: no facility, so no route state and no overlay.
const PHASE_1_FIXTURE: &str = "scenarios/walking/walking_guide_v1.json5";
/// An Increment 1 version-2 fixture: route state, no Increment 2 policy.
const INCREMENT_1_FIXTURE: &str = "scenarios/phase2/inc1/narrow_isolated_straight_v2.json5";

/// The outbound guide path of the fixture's occupied opposing corridor (`d`).
const OCCUPIED_GUIDE: &str = "guide_d";
/// The leader arc-length window the TAS-131 driver turns its rider in, in
/// metres. The fixture documents its first pair at 25.2 m.
const OCCUPIED_LEAD_WINDOW: (f64, f64) = (25.0, 48.0);
/// The bumper gap window of that same pair, in metres: the fixture places it at
/// 9.8 m.
const OCCUPIED_GAP_WINDOW: (f64, f64) = (8.0, 18.0);

/// The five route-relative overlay families, in the order both backends draw
/// them.
const FAMILIES: [&str; 5] = [
    "corridor",
    "target_offset",
    "predicted_gap",
    "maneuver",
    "wrong_way",
];

/// The tick, agent, and frame of one overlay landmark.
struct Landmark {
    tick: u64,
    agent: usize,
    frame: SceneFrame,
}

/// One driven run: every overlay-bearing frame described canonically, the first
/// frame each family appeared in, and the final frame.
struct DrivenRun {
    /// One canonical line per overlay-bearing tick, in tick order.
    stream: Vec<String>,
    /// First landmark of each overlay family that ever appeared.
    first: BTreeMap<&'static str, Landmark>,
    /// Last frame an open maneuver interval was present.
    last_maneuver: Option<Landmark>,
    /// The run's final frame.
    last_frame: SceneFrame,
    /// Completed ticks.
    ticks: u64,
}

impl DrivenRun {
    /// Fold one projected frame into the run's observations.
    fn observe(&mut self, tick: u64, frame: &SceneFrame) {
        let corridors = frame.corridors();
        let targets = frame.target_offsets();
        let gaps = frame.predicted_gaps();
        let maneuvers = frame.maneuver_overlays();
        let wrong_way = frame.wrong_way_overlays();
        let firsts: [(&'static str, Option<usize>); 5] = [
            ("corridor", corridors.first().map(|overlay| overlay.agent())),
            (
                "target_offset",
                targets.first().map(|overlay| overlay.agent()),
            ),
            ("predicted_gap", gaps.first().map(|overlay| overlay.agent())),
            ("maneuver", maneuvers.first().map(|overlay| overlay.agent())),
            (
                "wrong_way",
                wrong_way.first().map(|overlay| overlay.agent()),
            ),
        ];
        let mut carried = false;
        for (family, agent) in firsts {
            let Some(agent) = agent else { continue };
            carried = true;
            self.first.entry(family).or_insert_with(|| Landmark {
                tick,
                agent,
                frame: frame.clone(),
            });
        }
        if let Some(maneuver) = maneuvers.first() {
            self.last_maneuver = Some(Landmark {
                tick,
                agent: maneuver.agent(),
                frame: frame.clone(),
            });
        }
        if carried {
            self.stream.push(describe(tick, frame));
        }
    }

    /// The first frame of `family` appeared in.
    fn landmark(&self, family: &str) -> &Landmark {
        self.first
            .get(family)
            .unwrap_or_else(|| panic!("the run never carried the '{family}' overlay"))
    }

    /// Whether the run carried `family` at all.
    fn carried(&self, family: &str) -> bool {
        self.first.contains_key(family)
    }
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Load `relative` through the version-negotiating loader.
fn load(relative: &str) -> Arc<CompiledScenario> {
    Arc::new(
        load_scenario(&repo_path(relative))
            .unwrap_or_else(|error| panic!("{relative} did not load: {error}")),
    )
}

/// Load `relative` through the version-negotiating v2 loader, asserting the
/// document the presenter opened is a native version 2.
fn load_version_2(relative: &str) -> Arc<CompiledScenario> {
    let scenario = load(relative);
    assert_eq!(
        scenario.schema_version(),
        2,
        "{relative} is not a native version 2 document"
    );
    scenario
}

/// Drive `scenario` through the shared controller for `ticks` steps, folding
/// each step's records and describing every frame that carried an overlay.
///
/// `request` is called after each step until it returns `true`, so a fixture
/// that authors no request can record the one entry its case needs.
fn drive(
    scenario: &Arc<CompiledScenario>,
    ticks: u64,
    mut request: impl FnMut(&mut Simulation) -> bool,
) -> DrivenRun {
    let mut sim = Simulation::new((**scenario).clone(), RunConfig::new(SEED))
        .expect("the fixture builds a simulation");
    let mut controller = PresentationController::new(
        SceneGeometry::from_scenario(scenario),
        STEP_SECS,
        Viewport::new(DVec2::ZERO, 1.0),
    );
    let mut previous = sim.snapshot(SnapshotDetail::Full);
    let mut run = DrivenRun {
        stream: Vec::new(),
        first: BTreeMap::new(),
        last_maneuver: None,
        last_frame: controller.project(&previous, &previous),
        ticks: 0,
    };
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
        run.observe(tick, &frame);
        run.last_frame = frame;
        run.ticks = tick;
        previous = current;
    }
    run
}

/// The canonical description of one overlay-bearing frame: every primitive's
/// identifiers and values, so a replay difference is readable.
fn describe(tick: u64, frame: &SceneFrame) -> String {
    let corridors: Vec<String> = frame
        .corridors()
        .iter()
        .map(|overlay| {
            format!(
                "#{} {:.6}..{:.6} m on facility {}",
                overlay.agent(),
                overlay.d_min_m(),
                overlay.d_max_m(),
                overlay.facility().get()
            )
        })
        .collect();
    let targets: Vec<String> = frame
        .target_offsets()
        .iter()
        .map(|overlay| {
            format!(
                "#{} {:.6} m ({}) from {:.6} m",
                overlay.agent(),
                overlay.offset_m(),
                overlay.side().label(),
                overlay.from_m()
            )
        })
        .collect();
    let gaps: Vec<String> = frame
        .predicted_gaps()
        .iter()
        .map(|overlay| {
            format!(
                "#{} {:.6} m target {} margin {}",
                overlay.agent(),
                overlay.predicted_min_clearance_m(),
                optional_metres(overlay.target_clearance_m()),
                optional_metres(overlay.margin_m())
            )
        })
        .collect();
    let maneuvers: Vec<String> = frame
        .maneuver_overlays()
        .iter()
        .map(|overlay| {
            format!(
                "#{} {} {} {} {} partner {} facility {}",
                overlay.agent(),
                overlay.state().label(),
                overlay.kind().label(),
                overlay.edge().label(),
                overlay.reason().label(),
                optional_id(overlay.partner()),
                overlay.source_facility().get()
            )
        })
        .collect();
    let wrong_way: Vec<String> = frame
        .wrong_way_overlays()
        .iter()
        .map(|overlay| {
            format!(
                "#{} facility {} movement {} {} nominal {} rule {} reason {} violating {}",
                overlay.agent(),
                overlay.facility().get(),
                optional_id(overlay.movement().map(|movement| movement.get() as usize)),
                overlay.direction().label(),
                overlay.nominal_direction().label(),
                overlay.perceived_rule().map_or("none", |rule| rule.label()),
                overlay.reason().label(),
                overlay.violating()
            )
        })
        .collect();
    format!(
        "tick {tick}  corridor [{corridors}]  target [{targets}]  gap [{gaps}]  \
         maneuver [{maneuvers}]  wrong-way [{wrong_way}]",
        corridors = corridors.join(", "),
        targets = targets.join(", "),
        gaps = gaps.join(", "),
        maneuvers = maneuvers.join(", "),
        wrong_way = wrong_way.join(", "),
    )
}

/// Scalar text for an optional value in metres.
fn optional_metres(value: Option<f64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| format!("{value:.6} m"))
}

/// Text for an optional body or dense id.
fn optional_id(value: Option<usize>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| format!("#{value}"))
}

/// The five overlay summaries both backends append for a selected body, at
/// `agent`'s own values in `frame`: the same shared functions the Bevy inspector
/// and the terminal footer call.
fn inspector(frame: &SceneFrame, agent: usize) -> String {
    let corridor = frame
        .corridors()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let target = frame
        .target_offsets()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let gap = frame
        .predicted_gaps()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let maneuver = frame
        .maneuver_overlays()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let rule = frame
        .wrong_way_overlays()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    format!(
        "  corridor {corridor}\n  target {target}\n  gap {gap}\n  maneuver {maneuver}\n  \
         rule {rule}\n",
        corridor = corridor_summary(corridor.as_ref()),
        target = target_offset_summary(target.as_ref()),
        gap = predicted_gap_summary(gap.as_ref()),
        maneuver = maneuver_summary(maneuver.as_ref()),
        rule = wrong_way_summary(rule.as_ref()),
    )
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

/// Step the passing fixture: its own demand and eligibility tactics produce the
/// one overtaking interval, so no request is recorded.
fn drive_passing() -> DrivenRun {
    drive(&load_version_2(PASSING_FIXTURE), PASSING_TICKS, |_| false)
}

/// Step the wrong-way fixture and record the occupied corridor's entry once its
/// adjacent pair is inside the fixture's stopping distance.
fn drive_occupied_corridor() -> DrivenRun {
    let scenario = load_version_2(WRONG_WAY_FIXTURE);
    let guide = scenario
        .paths()
        .iter()
        .position(|path| path.name() == OCCUPIED_GUIDE)
        .unwrap_or_else(|| panic!("the fixture authors a guide path '{OCCUPIED_GUIDE}'"));
    drive(&scenario, WRONG_WAY_TICKS, move |sim| {
        let Some(lead) = occupied_lead(sim, guide) else {
            return false;
        };
        assert!(
            sim.request_wrong_way_entry(lead),
            "the occupied corridor's leading rider can request the entry"
        );
        true
    })
}

/// The two fixtures together carry all five route-relative overlays, opened
/// through the version-2 loader, with the identifiers the contract names.
#[test]
fn both_increment_2_fixtures_carry_the_five_route_relative_overlays() {
    let passing = drive_passing();
    let wrong_way = drive_occupied_corridor();

    // The passing fixture: corridor, target offset, predicted gap, and the
    // maneuver interval of its one executed pass.
    for family in ["corridor", "target_offset", "predicted_gap", "maneuver"] {
        assert!(
            passing.carried(family),
            "the passing fixture carried no '{family}' overlay"
        );
    }
    assert!(
        !passing.carried("wrong_way"),
        "the passing fixture authors no opposing traversal"
    );

    let maneuver = passing.landmark("maneuver");
    let overlay = &maneuver.frame.maneuver_overlays()[0];
    assert_eq!(overlay.agent(), maneuver.agent);
    assert_eq!(overlay.kind(), TacticKind::Pass);
    assert_eq!(overlay.edge(), ManeuverEdge::Attempted);
    assert_eq!(overlay.reason(), ManeuverReasonCode::SlowerLeader);
    assert_eq!(overlay.state().label(), "preparing");
    assert!(overlay.partner().is_some(), "the pass names its leader");
    assert_eq!(overlay.side(), PassSide::Left);
    assert!(
        overlay.target_offset_m() > 0.0,
        "a left-side pass targets a positive offset, got {}",
        overlay.target_offset_m()
    );
    assert!(
        overlay.opened_tick() <= maneuver.tick,
        "the interval opens at or before the frame it appears in"
    );
    let target = &maneuver.frame.target_offsets()[0];
    assert_eq!(target.agent(), maneuver.agent);
    assert_eq!(target.side(), PassSide::Left);
    assert_eq!(target.from_m(), 0.0, "the pass starts from the lane centre");
    assert!(
        target.offset_m() > target.from_m(),
        "the target displaces to the left of the lane centre"
    );
    let gap = &maneuver.frame.predicted_gaps()[0];
    assert_eq!(gap.agent(), maneuver.agent);
    let agent_corridor = maneuver
        .frame
        .corridors()
        .into_iter()
        .find(|corridor| corridor.agent() == maneuver.agent)
        .expect("the maneuvering body carries its own corridor");
    assert_eq!(
        gap.target_clearance_m(),
        agent_corridor.clearance_m(),
        "the gap is measured against the body's own corridor clearance"
    );
    assert!(gap.horizon_s().is_some(), "the pass declares a horizon");
    assert!(
        gap.margin_m().is_some_and(|margin| margin > 0.0),
        "the committed pass predicts a clearance above its target"
    );

    // The corridor is the band inset by the body and its resolved clearance.
    let corridor = passing.landmark("corridor");
    assert_eq!(
        corridor.agent, 0,
        "the leader is the first body on the band"
    );
    let corridor_overlay = &corridor.frame.corridors()[0];
    assert!(
        corridor_overlay.d_min_m() < corridor_overlay.d_max_m(),
        "the usable corridor is a non-empty interval"
    );
    assert!(corridor_overlay.clearance_m().is_some());

    // The maneuver interval's last frame is the returning edge, so the fold
    // closes on the edge that returns the passer to following.
    let close = passing
        .last_maneuver
        .as_ref()
        .expect("the passing fixture carries a maneuver interval");
    assert!(
        close.tick >= maneuver.tick,
        "the interval's last frame cannot precede its first"
    );
    assert_eq!(
        close.frame.maneuver_overlays()[0].state().label(),
        "returning",
        "the last interval frame is the returning edge"
    );

    // The occupied opposing corridor: the wrong-way rule state of the turned
    // rider, and the corridor it travels inside.
    let scenario = load_version_2(WRONG_WAY_FIXTURE);
    let facility = |name: &str| {
        scenario
            .facilities()
            .iter()
            .position(|facility| facility.name() == name)
            .unwrap_or_else(|| panic!("the fixture authors a facility '{name}'"))
    };
    let movement = |name: &str| {
        scenario
            .movements()
            .iter()
            .find(|movement| movement.name() == name)
            .unwrap_or_else(|| panic!("the fixture authors a movement '{name}'"))
            .id()
    };
    let open = wrong_way.landmark("wrong_way");
    let overlay = &open.frame.wrong_way_overlays()[0];
    assert_eq!(overlay.agent(), open.agent);
    assert_eq!(overlay.facility(), FacilityId::from_index(facility("d")));
    assert_eq!(
        overlay.movement(),
        Some(movement("through_d")),
        "the turned rider entered on the corridor's own movement"
    );
    assert_eq!(overlay.direction(), MovementDirection::Reverse);
    assert_eq!(overlay.nominal_direction(), NominalDirection::Forward);
    assert_eq!(
        overlay.perceived_rule(),
        Some(PermissionEffect::Permit),
        "the occupied corridor authors the same permit statement as `a`"
    );
    assert_eq!(overlay.reason(), WrongWayReason::LegalPermission);
    assert!(
        !overlay.violating(),
        "a permitted opposing traversal is never a violation"
    );
    assert_eq!(overlay.opened_tick(), open.tick);
    assert!(
        open.frame
            .corridors()
            .iter()
            .any(|corridor| corridor.agent() == open.agent),
        "the turned rider carries the occupied corridor's band too"
    );
    // The interval the encounter leaves open is the one the run end closes: the
    // fixture documents that the pair meets and cannot complete the traversal.
    let ended = wrong_way.last_frame.wrong_way_overlays();
    assert_eq!(
        ended.len(),
        1,
        "the wrong-way interval stays open to the run end: {ended:?}"
    );
    assert_eq!(ended[0].agent(), open.agent);
    assert!(
        wrong_way.ticks > open.tick,
        "the run steps on after the interval opens"
    );
    assert!(
        !wrong_way.carried("maneuver"),
        "the occupied corridor records no lateral maneuver; it waits and brakes"
    );

    // Across the two runs, each of the five families appeared.
    let mut carried: Vec<&str> = FAMILIES
        .into_iter()
        .filter(|family| passing.carried(family) || wrong_way.carried(family))
        .collect();
    carried.sort_unstable();
    let mut expected = FAMILIES.to_vec();
    expected.sort_unstable();
    assert_eq!(
        carried, expected,
        "the two fixtures must carry all five overlays between them"
    );
}

// The same-fixture determinism this suite used to re-drive
// (`replaying_a_fixture_projects_an_identical_overlay_stream`) is owned by
// `the_fixture_overlays_match_the_checked_in_golden`: it pins every overlay
// frame's exact text against
// `tests/golden/present/inc2_tactical_fixtures.seed0.txt`, which is strictly
// stronger than comparing two in-process projections of the same fixture.

/// A Phase 1 run carries none of the five overlays, and an Increment 1 run
/// carries none of the four a target offset, prediction, or open interval
/// produces.
#[test]
fn phase_1_and_increment_1_runs_carry_no_sparse_or_interval_overlay() {
    let phase_one_scenario = load(PHASE_1_FIXTURE);
    assert_eq!(
        phase_one_scenario.schema_version(),
        1,
        "the Phase 1 fixture stays a version-1 document"
    );
    let phase_one = drive(&phase_one_scenario, ABSENCE_TICKS, |_| false);
    assert!(
        phase_one.stream.is_empty(),
        "the Phase 1 run drew overlays: {:?}",
        phase_one.stream
    );
    for family in FAMILIES {
        assert!(
            !phase_one.carried(family),
            "a Phase 1 run carried a '{family}' overlay"
        );
    }

    let increment_one = drive(&load_version_2(INCREMENT_1_FIXTURE), ABSENCE_TICKS, |_| {
        false
    });
    for family in ["target_offset", "predicted_gap", "maneuver", "wrong_way"] {
        assert!(
            !increment_one.carried(family),
            "an Increment 1 run carried a '{family}' overlay"
        );
    }
    // An Increment 1 version-2 body carries route state on a compiled band, so
    // its corridor is present: the primitive is defined for exactly that sample,
    // and the rest of its inspector is empty.
    let corridor = increment_one.landmark("corridor");
    assert!(corridor.frame.corridors()[0].clearance_m().is_none());
    assert!(corridor.frame.corridors()[0].horizon_s().is_none());
    assert!(corridor.frame.target_offsets().is_empty());
    assert!(corridor.frame.predicted_gaps().is_empty());
    assert!(corridor.frame.maneuver_overlays().is_empty());
    assert!(corridor.frame.wrong_way_overlays().is_empty());
}

/// The overlays' exact ticks and inspector text match the checked-in golden, so
/// both backends' parity tests compare against one shared artifact.
#[test]
fn the_fixture_overlays_match_the_checked_in_golden() {
    let passing = drive_passing();
    let wrong_way = drive_occupied_corridor();
    let mut golden = String::from("Increment 2 tactical overlays: seed 0, step 0.05 s\n");
    golden.push_str(&format!("\n{PASSING_FIXTURE}\n"));
    golden.push_str(&section(&passing, "corridor"));
    golden.push_str(&section(&passing, "maneuver"));
    let close = passing
        .last_maneuver
        .as_ref()
        .expect("the passing fixture carries a maneuver interval");
    golden.push_str(&format!(
        "  last maneuver frame at tick {} for #{}\n",
        close.tick, close.agent
    ));
    golden.push_str(&inspector(&close.frame, close.agent));
    golden.push_str(&format!(
        "\n{WRONG_WAY_FIXTURE} (occupied opposing corridor)\n"
    ));
    golden.push_str(&section(&wrong_way, "wrong_way"));
    golden.push_str(&format!(
        "  run end at tick {} for #{}\n",
        wrong_way.ticks,
        wrong_way.landmark("wrong_way").agent
    ));
    golden.push_str(&inspector(
        &wrong_way.last_frame,
        wrong_way.landmark("wrong_way").agent,
    ));
    check_text(
        "tests/golden/present/inc2_tactical_fixtures.seed0.txt",
        &golden,
    );
}

/// One golden section: the frame a family opened in, and its inspector text.
fn section(run: &DrivenRun, family: &str) -> String {
    let landmark = run.landmark(family);
    let mut text = format!(
        "  {family} opened at tick {} for #{}\n",
        landmark.tick, landmark.agent
    );
    text.push_str(&inspector(&landmark.frame, landmark.agent));
    text
}

/// Compare `actual` with a checked-in golden, writing it instead when
/// `UPDATE_GOLDENS` is set.
fn check_text(relative: &str, actual: &str) {
    let path = repo_path(relative);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden has a parent")).expect("create dir");
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read golden '{}': {error}\nrun: UPDATE_GOLDENS=1 cargo test -p \
             hekate-present --test inc2_fixture_overlays",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "golden '{}' changed; regenerate it and inspect the diff",
        path.display()
    );
}
