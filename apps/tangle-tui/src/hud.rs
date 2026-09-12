//! Shared status and inspector text for terminal backends.
//!
//! Both the character-cell backend and the Kitty graphics backend show the same
//! two status lines and the same inspector/help footer. The text lives here so
//! the two backends cannot drift: [`Hud`] turns one [`SceneFrame`] plus the run
//! totals the shared frame does not carry into those three strings.

use std::sync::Arc;

use tangle_model::CompiledScenario;
use tangle_present::{SceneBody, SceneFrame};

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
            status: ["Tangle".to_owned(), String::new()],
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
                "Tangle {scenario}   sim {seconds:.2} s   tick {tick}   speed {speed}   {paused}",
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
                |body| self.describe(body),
            ),
            None => "\
space pause   . step   1/2/3 speed   r restart   n next seed   \
WASD/arrows pan   +/- zoom   tab select   g geometry   v vectors   \
esc clear   q quit"
                .to_owned(),
        }
    }

    /// The same agent fields the Bevy inspector shows, on one line.
    fn describe(&self, body: &SceneBody) -> String {
        let mut out = format!(
            "Agent #{id}   position ({x:.2}, {y:.2}) m   heading {heading:.1}°",
            id = body.id,
            x = body.position.x,
            y = body.position.y,
            heading = body.heading_rad.to_degrees(),
        );

        if let Some(speed) = body.speed_mps {
            let path = body
                .path
                .and_then(|id| self.scenario.id_map().path_name(id))
                .unwrap_or("<unknown>");
            out.push_str(&format!(
                "   speed {speed:.2} m/s   path {path}   distance {distance:.2} m   \
                 body {length:.2} x {width:.2} m   intent hold constant speed along the guide path",
                distance = body.path_distance_m.unwrap_or(0.0),
                length = body.length_m,
                width = body.width_m,
            ));
        }

        out.push_str("   decision none yet (Increment 0 has no decisions)");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use glam::DVec2;
    use tangle_model::parse_scenario_source;
    use tangle_present::{FrameStatus, Overlays, SceneGeometry, Speed, Viewport};
    use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

    fn scenario() -> Arc<CompiledScenario> {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
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
                .map(|sample| tangle_present::SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
        }
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
        assert!(first.contains("Tangle walking"));
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
        assert!(hud.selected());
    }
}
