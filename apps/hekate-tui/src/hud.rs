//! Shared status and inspector text for terminal backends.
//!
//! Both the character-cell backend and the Kitty graphics backend show the same
//! two status lines and the same inspector/help footer. The text lives here so
//! the two backends cannot drift: [`Hud`] turns one [`SceneFrame`] plus the run
//! totals the shared frame does not carry into those three strings.

use std::sync::Arc;

use hekate_model::CompiledScenario;
use hekate_present::{
    SceneBody, SceneFrame, corridor_summary, decision_summary, event_summary, intent_summary,
    maneuver_summary, predicted_gap_summary, profile_summary, target_offset_summary,
    wrong_way_summary,
};

use crate::backend::RunInfo;

/// The status and footer text a terminal backend overlays on a frame.
pub struct Hud {
    scenario: Arc<CompiledScenario>,
    run: RunInfo,
    status: [String; 2],
    footer: String,
    selected: bool,
}

impl Hud {
    /// A HUD for `scenario`, with placeholder text until the first frame.
    pub fn new(scenario: Arc<CompiledScenario>) -> Self {
        Self {
            scenario,
            run: RunInfo::default(),
            status: ["Hekate".to_owned(), String::new()],
            footer: String::new(),
            selected: false,
        }
    }

    /// Update the run totals shown on the status line.
    pub fn set_run_info(&mut self, run: RunInfo) {
        self.run = run;
    }

    /// Current run totals.
    pub const fn run_info(&self) -> RunInfo {
        self.run
    }

    /// Recompute the status lines and footer for `frame`.
    pub fn update(&mut self, frame: &SceneFrame) {
        self.status = self.build_status(frame);
        self.footer = self.build_footer(frame);
        self.selected = frame.status.selection.is_some();
    }

    /// The two status lines last computed by [`Self::update`].
    pub const fn status_lines(&self) -> &[String; 2] {
        &self.status
    }

    /// The inspector or help line last computed by [`Self::update`].
    pub fn footer_line(&self) -> &str {
        &self.footer
    }

    /// Whether the last frame had a selection, which colors the footer.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// The two status lines for `frame`, including run totals.
    fn build_status(&self, frame: &SceneFrame) -> [String; 2] {
        let paused = if frame.status.paused {
            "paused"
        } else {
            "running"
        };
        let selected = frame
            .status
            .selection
            .map_or_else(|| "none".to_owned(), |id| format!("#{id}"));
        [
            format!(
                "Hekate {scenario}   sim {seconds:.2} s   tick {tick}   speed {speed}   {paused}",
                scenario = frame.scenario_id,
                seconds = frame.time_seconds,
                tick = frame.tick,
                speed = frame.status.speed.label(),
            ),
            format!(
                "seed {seed}   agents {alive}   spawned {spawned}   \
                 despawned {despawned}   selected {selected}",
                seed = self.run.seed,
                alive = frame.status.agents,
                spawned = self.run.spawned,
                despawned = self.run.despawned,
            ),
        ]
    }

    /// The inspector for the selected agent, or the control legend otherwise.
    fn build_footer(&self, frame: &SceneFrame) -> String {
        match frame.status.selection {
            Some(id) => frame.body(id).map_or_else(
                || format!("Agent #{id} is no longer alive."),
                |body| self.describe(body, frame),
            ),
            None => "\
space pause   . step   1/2/3 speed   r restart   n next seed   \
WASD/arrows pan   +/- zoom   tab select   g geometry   v vectors   b safety   \
c corridor   t target   p gap   m maneuver   o wrong-way   esc clear   q quit"
                .to_owned(),
        }
    }

    /// The same agent fields the Bevy inspector shows, on one line, followed by
    /// the recent records the body took part in.
    fn describe(&self, body: &SceneBody, frame: &SceneFrame) -> String {
        let mut out = format!(
            "Agent #{id} ({mode})   position ({x:.2}, {y:.2}) m   heading {heading:.1}°",
            id = body.id,
            mode = body.mode.label(),
            x = body.position.x,
            y = body.position.y,
            heading = body.heading_rad.to_degrees(),
        );

        if let Some(speed) = body.speed_mps {
            let path = body
                .path
                .and_then(|id| self.scenario.id_map().path_name(id))
                .unwrap_or("<unknown>");
            let route = match body.route {
                Some(id) => self
                    .scenario
                    .id_map()
                    .movement_name(id)
                    .unwrap_or("<unknown>"),
                None => "none (static population)",
            };
            out.push_str(&format!(
                "   speed {speed:.2} m/s   path {path}   distance {distance:.2} m   \
                 body {length:.2} x {width:.2} m   route {route}   \
                 profile {profile}   intent {intent}",
                distance = body.path_distance_m.unwrap_or(0.0),
                length = body.length_m,
                width = body.width_m,
                profile = profile_summary(body.profile),
                intent = intent_summary(body.profile),
            ));
        }

        out.push_str(&format!("   decision {}", decision_summary(body.decision)));

        // The route-relative overlays the frame derives for this body: the same
        // shared summaries the Bevy inspector shows. A body with no route state
        // (Phase 1, Increment 1) carries none, so its inspector is unchanged.
        if body.route_state.is_some() {
            let corridor = frame
                .corridors()
                .into_iter()
                .find(|overlay| overlay.agent() == body.id);
            let target = frame
                .target_offsets()
                .into_iter()
                .find(|overlay| overlay.agent() == body.id);
            let gap = frame
                .predicted_gaps()
                .into_iter()
                .find(|overlay| overlay.agent() == body.id);
            let maneuver = frame
                .maneuver_overlays()
                .into_iter()
                .find(|overlay| overlay.agent() == body.id);
            let wrong_way = frame
                .wrong_way_overlays()
                .into_iter()
                .find(|overlay| overlay.agent() == body.id);
            out.push_str(&format!(
                "   corridor {corridor}   target {target}   gap {gap}   \
                 maneuver {maneuver}   rule {rule}",
                corridor = corridor_summary(corridor.as_ref()),
                target = target_offset_summary(target.as_ref()),
                gap = predicted_gap_summary(gap.as_ref()),
                maneuver = maneuver_summary(maneuver.as_ref()),
                rule = wrong_way_summary(wrong_way.as_ref()),
            ));
        }

        // The event links: what the body was recently part of. The footer is
        // one line, so only the most recent records are named.
        let links = frame.events_involving(body.id);
        if let Some(latest) = links.last() {
            out.push_str(&format!("   link {}", event_summary(latest)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use glam::DVec2;
    use hekate_model::parse_scenario_source;
    use hekate_model::{
        FacilityId, MovementDirection, NominalDirection, PermissionEffect, TacticKind,
    };
    use hekate_present::{
        FrameStatus, Overlays, SafetyOverlay, SceneGeometry, Speed, TacticalOverlay, Viewport,
        load_scenario,
    };
    use hekate_sim::{
        AgentId, Event, ManeuverEdge, ManeuverReasonCode, ManeuverState, PassSide, RunConfig,
        Simulation, SnapshotDetail, ViolationKind, WrongWayReason,
    };

    fn scenario() -> Arc<CompiledScenario> {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
             movements: [ { id: 'through', from: 'west', to: 'east', path: 'guide', \
             priority: 0 } ], \
             population: { vehicle_count: 1, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        Arc::new(CompiledScenario::compile(source).expect("scenario compiles"))
    }

    fn frame(selection: Option<usize>) -> SceneFrame {
        let compiled = scenario();
        let sim = Simulation::new((*compiled).clone(), RunConfig::new(4)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 1.25,
            tick: 25,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Fast,
                paused: true,
                selection,
            },
            viewport: Viewport::new(DVec2::ZERO, 0.5),
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: snapshot
                .agents()
                .iter()
                .map(|sample| hekate_present::SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
            tactical: TacticalOverlay::default(),
        }
    }

    /// A frame over a checked-in version-2 fixture held long enough for both
    /// narrow modes to spawn, carrying one body's route state and an open
    /// maneuver and opposing-traversal interval.
    fn tactical_frame() -> (std::sync::Arc<CompiledScenario>, SceneFrame) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scenarios/phase2/inc1/narrow_isolated_straight_v2.json5");
        let compiled = load_scenario(&path).expect("fixture loads");
        let mut sim = Simulation::new(compiled.clone(), RunConfig::new(0)).expect("builds");
        for _ in 0..400 {
            sim.step();
        }
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let mut body = SceneBody::project(&[], &snapshot.agents()[0], 0.0);
        let state = body
            .route_state
            .as_mut()
            .expect("the fixture run carries route state");
        state.maneuver_state = ManeuverState::Committed;
        state.target_offset_m = Some(0.8);
        state.predicted_min_clearance_m = Some(0.9);
        state.target_clearance_m = Some(0.5);
        state.perceived_rule = Some(PermissionEffect::Prohibit);
        state.opposing_direction = Some(MovementDirection::Reverse);

        let mut tactical = TacticalOverlay::default();
        tactical.observe(
            snapshot.time().tick(),
            &[
                Event::Maneuver {
                    agent: AgentId::from_index(0),
                    kind: TacticKind::Overtake,
                    from: ManeuverState::Preparing,
                    to: ManeuverState::Committed,
                    edge: ManeuverEdge::Committed,
                    partner: Some(AgentId::from_index(1)),
                    source_facility: FacilityId::from_index(1),
                    target_facility: None,
                    target_offset_m: 0.8,
                    side: PassSide::Left,
                    reason: ManeuverReasonCode::SlowerLeader,
                },
                Event::OpposingTraversal {
                    agent: AgentId::from_index(0),
                    facility: FacilityId::from_index(1),
                    movement: None,
                    direction: MovementDirection::Reverse,
                    nominal_direction: NominalDirection::Forward,
                    perceived_rule: Some(PermissionEffect::Prohibit),
                    reason: WrongWayReason::NoncompliantChoice,
                    violating: true,
                    entering: true,
                },
            ],
        );

        let frame = SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: snapshot.time().seconds(),
            tick: snapshot.time().tick(),
            status: FrameStatus {
                agents: 1,
                speed: Speed::Real,
                paused: true,
                selection: Some(body.id),
            },
            viewport: Viewport::new(body.position, 0.5),
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: vec![body],
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
            tactical,
        };
        (Arc::new(compiled), frame)
    }

    /// The inspector names the same route-relative identifiers the Bevy
    /// inspector does, from the same shared summaries.
    #[test]
    fn the_inspector_reports_the_same_route_relative_fields_as_the_bevy_viewer() {
        let (compiled, frame) = tactical_frame();
        let hud = Hud::new(compiled);
        let text = hud.describe(frame.body(0).expect("body is alive"), &frame);

        assert!(
            text.contains("corridor #0  facility 1  usable corridor"),
            "{text}"
        );
        assert!(text.contains("clearance 0.50 m"), "{text}");
        assert!(
            text.contains("target #0  target offset 0.80 m (left)  from 0.00 m"),
            "{text}"
        );
        assert!(
            text.contains("gap #0  predicted min clearance 0.90 m  target 0.50 m  margin 0.40 m"),
            "{text}"
        );
        assert!(
            text.contains(
                "maneuver #0  maneuver committed  tactic overtake  edge committed  \
             reason slower_leader  partner #1  target 0.80 m (left)  facility 1 -> none"
            ),
            "{text}"
        );
        assert!(
            text.contains("rule #0  opposing facility 1  movement none  direction reverse"),
            "{text}"
        );
        assert!(text.contains("violating true"), "{text}");
    }

    /// The footer legend names every overlay toggle.
    #[test]
    fn the_footer_legend_names_every_overlay_toggle() {
        let mut hud = Hud::new(scenario());
        hud.update(&frame(None));
        let legend = hud.footer_line();
        for key in [
            "g geometry",
            "v vectors",
            "b safety",
            "c corridor",
            "t target",
            "p gap",
            "m maneuver",
            "o wrong-way",
        ] {
            assert!(legend.contains(key), "the legend omits '{key}': {legend}");
        }
    }

    /// The same frame with `events` folded in at its own tick.
    fn frame_with_events(selection: Option<usize>, events: &[Event]) -> SceneFrame {
        let mut frame = frame(selection);
        let mut safety = SafetyOverlay::new(40);
        safety.observe(frame.tick, events);
        frame.safety = safety;
        frame
    }

    #[test]
    fn the_hud_reports_tick_seed_speed_and_run_totals() {
        let mut hud = Hud::new(scenario());
        hud.set_run_info(RunInfo {
            seed: 4,
            spawned: 3,
            despawned: 1,
        });
        hud.update(&frame(None));
        let [first, second] = hud.status_lines();
        assert!(first.contains("Hekate walking"));
        assert!(first.contains("tick 25"));
        assert!(first.contains("speed 4x"));
        assert!(first.contains("paused"));
        assert!(second.contains("seed 4"));
        assert!(second.contains("spawned 3"));
        assert!(second.contains("despawned 1"));
        assert!(hud.footer_line().contains("space pause"));
        assert!(!hud.selected());
    }

    #[test]
    fn the_inspector_reports_the_same_fields_as_the_bevy_viewer() {
        let mut hud = Hud::new(scenario());
        hud.update(&frame(Some(0)));
        let footer = hud.footer_line();
        assert!(footer.starts_with("Agent #0"));
        assert!(footer.contains("position ("));
        assert!(footer.contains("speed 12.00 m/s"));
        assert!(footer.contains("path guide"));
        assert!(footer.contains("body 4.50 x 1.80 m"));
        // The static population has no route or sampled profile, so the
        // inspector says what it actually does instead of asserting IDM.
        assert!(
            footer.contains("route none (static population)"),
            "{footer}"
        );
        assert!(
            footer.contains("profile none (static population)"),
            "{footer}"
        );
        assert!(
            footer.contains("intent hold constant speed along the guide path"),
            "{footer}"
        );
        assert!(footer.contains("decision none (movement is not signal-controlled)"));
        assert!(hud.selected());
    }

    #[test]
    fn the_inspector_reports_the_latest_decision_reason() {
        use hekate_model::{MovementId, PathId, SignalColor};
        use hekate_sim::{ComplianceReason, SignalAction, VehicleProfile};

        let hud = Hud::new(scenario());
        let body = SceneBody {
            id: 0,
            position: DVec2::new(30.0, 0.0),
            heading_rad: 0.0,
            length_m: 4.0,
            width_m: 2.0,
            mode: hekate_sim::AgentMode::Vehicle,
            body_kind: hekate_sim::AgentMode::Vehicle.body_kind(),
            segments: Vec::new(),
            speed_mps: Some(0.0),
            path: Some(PathId::from_index(0)),
            path_distance_m: Some(30.0),
            route: Some(MovementId::from_index(0)),
            profile: Some(VehicleProfile {
                desired_speed_mps: 10.0,
                length_m: 4.0,
                width_m: 2.0,
                time_gap_s: 1.5,
                max_accel_mps2: 2.0,
                comfortable_brake_mps2: 3.0,
                compliance: 0.5,
            }),
            decision: Some(hekate_sim::ComplianceDecision {
                action: SignalAction::Stop,
                reason: ComplianceReason::CompliantStop,
                color: SignalColor::Red,
                stop_line_gap_m: 2.0,
                required_decel_mps2: 0.0,
            }),
            route_state: None,
        };
        let events = [
            Event::NearMiss {
                agent: AgentId::from_index(0),
                other: AgentId::from_index(3),
                clearance_m: 0.5,
                entering: true,
            },
            Event::Violation {
                agent: AgentId::from_index(0),
                kind: ViolationKind::RanRedLight,
            },
        ];
        let frame = frame_with_events(Some(0), &events);
        let text = hud.describe(&body, &frame);
        assert!(text.contains("Agent #0 (vehicle)"), "{text}");
        assert!(text.contains("decision stop (compliant stop)"), "{text}");
        assert!(text.contains("head red"), "{text}");
        // A demand vehicle reports its route, sampled profile, and IDM intent.
        assert!(text.contains("route through"), "{text}");
        assert!(text.contains("profile v0 10.00 m/s"), "{text}");
        assert!(text.contains("compliance 0.50"), "{text}");
        assert!(
            text.contains("intent follow IDM under sampled profile bounds"),
            "{text}"
        );
        // The footer names the most recent record the body took part in, with
        // the body it was about.
        assert!(
            text.contains("link t25 violation  #0  ran_red_light"),
            "{text}"
        );
        let nearest = hud.describe(&body, &frame_with_events(Some(0), &events[..1]));
        assert!(
            nearest.contains("link t25 near miss began  #0 + #3  clearance 0.50 m"),
            "{nearest}"
        );
    }
}
