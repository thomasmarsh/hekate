//! Bevy top-down viewer for Hekate.
//!
//! The viewer owns one [`Simulation`] and drives it only in whole fixed steps.
//! All playback state lives in the shared [`PresentationController`], so pause,
//! speed, and frame rate choose which ticks are observed but never change
//! kernel results. Rendering consumes backend-agnostic [`SceneFrame`]s and
//! casts the presentation layer's `f64` metres into Bevy `f32` transforms at
//! this boundary only.
//!
//! This is the only crate permitted to depend on Bevy.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use glam::DVec2;
use hekate_model::CompiledScenario;
use hekate_present::{
    Applied, BodyEmphasis, EventParticipants, Overlay, PresentationController, RendererBackend,
    RestartMode, SafetyMarker, SceneBody, SceneFrame, SceneGeometry, Speed, ViewCommand, Viewport,
    corridor_summary, decision_summary, event_summary, intent_summary, load_scenario,
    maneuver_summary, predicted_gap_summary, profile_summary, target_offset_summary,
    wrong_way_summary,
};
use hekate_sim::{Event, ManeuverState, RunConfig, Simulation, Snapshot, SnapshotDetail};
use hekate_viewer::{BodyMesh, CurrentFrame, body_visuals};

/// Scenario used when no path is passed on the command line.
const DEFAULT_SCENARIO: &str = "scenarios/walking/walking_guide_v1.json5";
/// Event links the inspector lists before it summarizes the rest.
const MAX_INSPECTOR_LINKS: usize = 5;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_SCENARIO.to_owned()));
    let seed: u64 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    let scenario = match load_scenario(&path) {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let sim = match Simulation::new(scenario.clone(), RunConfig::new(seed)) {
        Ok(sim) => sim,
        Err(error) => {
            eprintln!("error: cannot start simulation: {error}");
            std::process::exit(1);
        }
    };

    let step_secs = sim.config().step().as_secs();
    let initial = sim.snapshot(SnapshotDetail::Full);
    let geometry = SceneGeometry::from_scenario(&scenario);
    let controller =
        PresentationController::new(geometry, step_secs, Viewport::new(DVec2::ZERO, 1.0));
    let first_frame = controller.project(&initial, &initial);

    let state = ViewerState {
        sim,
        controller,
        prev: initial.clone(),
        curr: initial,
        seed,
        spawned: 0,
        despawned: 0,
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Hekate — Phase 1 walking skeleton".to_owned(),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.07)))
        .insert_resource(Scenario(scenario))
        .insert_resource(state)
        .insert_resource(CurrentFrame::new(first_frame))
        .init_resource::<Visuals>()
        .add_systems(Startup, (setup, draw_help_text).chain())
        .add_systems(
            Update,
            (
                controls,
                camera_input,
                advance_simulation,
                sync_camera,
                project_scene,
                click_select,
                sync_agents,
                draw_geometry,
                draw_agent_overlays,
                draw_safety_overlays,
                draw_tactical_overlays,
                update_status_text,
                update_inspector_text,
            )
                .chain(),
        )
        .run();
}

/// The authored scenario, retained so the run can be restarted with a new seed.
#[derive(Resource)]
struct Scenario(CompiledScenario);

/// Live kernel, the shared presentation controller, and the two snapshots
/// interpolated between.
#[derive(Resource)]
struct ViewerState {
    sim: Simulation,
    controller: PresentationController,
    prev: Snapshot,
    curr: Snapshot,
    seed: u64,
    spawned: u64,
    despawned: u64,
}

impl ViewerState {
    /// Rebuild the kernel for `seed`, preserving playback speed and pause state
    /// and clearing the selection.
    fn restart(&mut self, scenario: &CompiledScenario, seed: u64) {
        let Ok(sim) = Simulation::new(scenario.clone(), RunConfig::new(seed)) else {
            return;
        };
        let snapshot = sim.snapshot(SnapshotDetail::Full);

        self.controller.reset_clock(sim.config().step().as_secs());
        self.controller.clear_selection();

        self.seed = seed;
        self.sim = sim;
        self.prev = snapshot.clone();
        self.curr = snapshot;
        self.spawned = 0;
        self.despawned = 0;
    }
}

/// Shared mesh and material handles. Every box shape reuses one mesh and
/// material and every circle shape another; there is no per-agent mesh or
/// material allocation.
#[derive(Resource)]
struct AgentAssets {
    box_mesh: Handle<Mesh>,
    circle_mesh: Handle<Mesh>,
    box_material: Handle<ColorMaterial>,
    circle_material: Handle<ColorMaterial>,
}

/// Identifies one rendered visual of one agent, so a body carrying ordered
/// segments reuses one entity per segment across frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BodyVisualKey {
    agent: usize,
    visual: usize,
}

/// Maps an agent's stable id and visual index to its rendered entity so
/// entities are reused across frames and despawns rather than recreated every
/// frame.
#[derive(Resource, Default)]
struct Visuals(HashMap<BodyVisualKey, Entity>);

/// Marks a rendered body entity.
#[derive(Component)]
struct AgentVisual;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct InspectorText;

/// Build the camera, shared agent assets, and the status/inspector UI.
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut state: ResMut<ViewerState>,
    window: Single<&Window>,
) {
    let bounds = state
        .controller
        .geometry()
        .bounds()
        .unwrap_or((DVec2::splat(-20.0), DVec2::splat(20.0)));
    let size = window.size().max(Vec2::splat(1.0));
    state
        .controller
        .set_viewport(Viewport::fit(bounds, (size.x as f64, size.y as f64), 1.25));
    let viewport = state.controller.viewport();
    let center = viewport.center();

    commands.spawn((
        Camera2d,
        Transform::from_xyz(center.x as f32, center.y as f32, 999.0),
        Projection::Orthographic(OrthographicProjection {
            scale: viewport.scale() as f32,
            ..OrthographicProjection::default_2d()
        }),
    ));

    commands.insert_resource(AgentAssets {
        // A box shape is a unit rectangle scaled to its length and width; a
        // circle shape is the unit-diameter circle its radius inscribes.
        box_mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        circle_mesh: meshes.add(Circle::new(0.5)),
        box_material: materials.add(Color::srgb(0.85, 0.87, 0.92)),
        circle_material: materials.add(Color::srgb(0.65, 0.90, 0.75)),
    });

    commands.spawn((
        StatusText,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.88, 0.93, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8.0),
            left: px(8.0),
            ..default()
        },
    ));

    commands.spawn((
        InspectorText,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.95, 0.85, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8.0),
            right: px(8.0),
            ..default()
        },
    ));
}

/// Static control legend in the bottom-left corner.
fn draw_help_text(mut commands: Commands) {
    commands.spawn((
        Text::new(
            "space pause/resume    . single tick    1/2/3 speed 1x/4x/max\n\
             WASD pan    mouse wheel zoom    R restart    N next seed\n\
             G geometry    V vectors    B safety    C corridor    T target    P gap    \
             M maneuver    O wrong-way    click a body to inspect    esc clear",
        ),
        TextFont::from_font_size(12.0),
        TextColor(Color::srgb(0.62, 0.68, 0.78)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(8.0),
            left: px(8.0),
            ..default()
        },
    ));
}

/// Translate keyboard input into shared view commands.
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ViewerState>,
    scenario: Res<Scenario>,
    frame: Res<CurrentFrame>,
) {
    let Some(frame) = frame.get() else {
        return;
    };

    let mut commands = Vec::new();
    if keys.just_pressed(KeyCode::Space) {
        commands.push(ViewCommand::TogglePause);
    }
    if keys.just_pressed(KeyCode::Period) {
        commands.push(ViewCommand::SingleStep);
    }
    if keys.just_pressed(KeyCode::Digit1) {
        commands.push(ViewCommand::SetSpeed(Speed::Real));
    }
    if keys.just_pressed(KeyCode::Digit2) {
        commands.push(ViewCommand::SetSpeed(Speed::Fast));
    }
    if keys.just_pressed(KeyCode::Digit3) {
        commands.push(ViewCommand::SetSpeed(Speed::Maximum));
    }
    if keys.just_pressed(KeyCode::KeyR) {
        commands.push(ViewCommand::Restart(RestartMode::SameSeed));
    }
    if keys.just_pressed(KeyCode::KeyN) {
        commands.push(ViewCommand::Restart(RestartMode::NextSeed));
    }
    if keys.just_pressed(KeyCode::KeyG) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::Geometry));
    }
    if keys.just_pressed(KeyCode::KeyV) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::Vectors));
    }
    if keys.just_pressed(KeyCode::KeyB) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::Safety));
    }
    if keys.just_pressed(KeyCode::KeyC) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::Corridor));
    }
    if keys.just_pressed(KeyCode::KeyT) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::TargetOffset));
    }
    if keys.just_pressed(KeyCode::KeyP) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::PredictedGap));
    }
    if keys.just_pressed(KeyCode::KeyM) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::Maneuver));
    }
    if keys.just_pressed(KeyCode::KeyO) {
        commands.push(ViewCommand::ToggleOverlay(Overlay::WrongWay));
    }
    if keys.just_pressed(KeyCode::Escape) {
        commands.push(ViewCommand::ClearSelection);
    }

    for command in commands {
        if let Applied::Restart(mode) = state.controller.apply(&command, frame) {
            let seed = match mode {
                RestartMode::SameSeed => state.seed,
                RestartMode::NextSeed => state.seed.wrapping_add(1),
            };
            state.restart(&scenario.0, seed);
        }
    }
}

/// Translate camera keys and wheel input into shared view commands.
fn camera_input(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    mut state: ResMut<ViewerState>,
    frame: Res<CurrentFrame>,
) {
    let Some(frame) = frame.get() else {
        return;
    };

    if scroll.delta.y != 0.0 {
        let factor = 1.0 - f64::from(scroll.delta.y) * 0.1;
        state.controller.apply(&ViewCommand::Zoom(factor), frame);
    }

    let mut direction = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction != Vec2::ZERO {
        let scale = state.controller.viewport().scale() as f32;
        let delta = direction.normalize() * 40.0 * scale * time.delta_secs();
        state.controller.apply(
            &ViewCommand::Pan(DVec2::new(delta.x as f64, delta.y as f64)),
            frame,
        );
    }
}

/// Take the whole steps the presentation clock buys this frame.
fn advance_simulation(time: Res<Time>, mut state: ResMut<ViewerState>) {
    let ticks = state.controller.advance(f64::from(time.delta_secs()));
    for _ in 0..ticks {
        state.prev = state.curr.clone();
        let mut spawned = 0_u64;
        let mut despawned = 0_u64;
        {
            // Split the borrow so the step's records reach the controller while
            // the step output is still alive.
            let ViewerState {
                sim, controller, ..
            } = &mut *state;
            let output = sim.step();
            controller.observe_events(output.time().tick(), output.events());
            for event in output.events() {
                match event {
                    Event::Spawned { .. } => spawned += 1,
                    Event::Despawned { .. } => despawned += 1,
                    // Safety and control records change no population counter.
                    Event::Yielded { .. }
                    | Event::Collision { .. }
                    | Event::NearMiss { .. }
                    | Event::Violation { .. }
                    | Event::Entry { .. }
                    | Event::Exit { .. }
                    | Event::Queue { .. }
                    | Event::ControlTransition { .. }
                    | Event::Maneuver { .. }
                    | Event::FacilityTransition { .. }
                    | Event::OpposingTraversal { .. }
                    | Event::ClosePass { .. } => {}
                }
            }
        }
        state.spawned += spawned;
        state.despawned += despawned;
        state.curr = state.sim.snapshot(SnapshotDetail::Full);
    }
}

/// Push the controller's viewport onto the Bevy camera.
fn sync_camera(
    state: Res<ViewerState>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };
    let Projection::Orthographic(orthographic) = &mut *projection else {
        return;
    };
    let viewport = state.controller.viewport();
    transform.translation.x = viewport.center().x as f32;
    transform.translation.y = viewport.center().y as f32;
    orthographic.scale = viewport.scale() as f32;
}

/// Project one shared scene frame through the Bevy backend.
fn project_scene(state: Res<ViewerState>, mut frame: ResMut<CurrentFrame>) {
    let scene = state.controller.project(&state.prev, &state.curr);
    frame
        .draw(&scene)
        .expect("the Bevy backend capture step is infallible");
}

/// Click the car nearest the cursor to select it for inspection.
fn click_select(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut state: ResMut<ViewerState>,
    mut frame: ResMut<CurrentFrame>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let (camera, transform) = *camera;
    let Ok(world) = camera.viewport_to_world_2d(transform, cursor) else {
        return;
    };
    let point = DVec2::new(world.x as f64, world.y as f64);

    let Some(scene) = frame.get() else {
        return;
    };
    state
        .controller
        .apply(&ViewCommand::SelectNearest(point), scene);
    let selection = state.controller.selection();
    frame.set_selection(selection);
}

/// Create, move, and retire body entities from the current frame.
///
/// Each body contributes the visuals the shared scene projection chose for it
/// ([`body_visuals`]): one box or circle, one box per ordered segment, or the
/// rectangle and two cap circles of a capsule. An entity keeps the mesh and
/// material of the shape kind it spawned with, so a body only reuses or
/// re-spawns entities as its visual list changes.
fn sync_agents(
    mut commands: Commands,
    frame: Res<CurrentFrame>,
    assets: Res<AgentAssets>,
    mut visuals: ResMut<Visuals>,
    mut transforms: Query<&mut Transform, With<AgentVisual>>,
) {
    let Some(frame) = frame.get() else {
        return;
    };

    let mut live = Vec::new();
    for body in &frame.bodies {
        for (visual, resolved) in body_visuals(body).into_iter().enumerate() {
            let key = BodyVisualKey {
                agent: body.id,
                visual,
            };
            if let Some(&entity) = visuals.0.get(&key) {
                if let Ok(mut current) = transforms.get_mut(entity) {
                    *current = resolved.transform;
                }
            } else {
                let (mesh, material) = match resolved.mesh {
                    BodyMesh::Box => (assets.box_mesh.clone(), assets.box_material.clone()),
                    BodyMesh::Circle => {
                        (assets.circle_mesh.clone(), assets.circle_material.clone())
                    }
                };
                let entity = commands
                    .spawn((
                        AgentVisual,
                        Mesh2d(mesh),
                        MeshMaterial2d(material),
                        resolved.transform,
                    ))
                    .id();
                visuals.0.insert(key, entity);
            }
            live.push(key);
        }
    }

    visuals.0.retain(|key, entity| {
        if live.contains(key) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });
}

/// Draw the scenario geometry from the current frame.
fn draw_geometry(frame: Res<CurrentFrame>, mut gizmos: Gizmos) {
    let Some(frame) = frame.get() else {
        return;
    };
    if !frame.overlays.geometry {
        return;
    }

    let to_vec = |point: DVec2| Vec2::new(point.x as f32, point.y as f32);
    let draw_ring = |gizmos: &mut Gizmos, points: &[DVec2], color: Color| {
        for index in 0..points.len() {
            let from = to_vec(points[index]);
            let to = to_vec(points[(index + 1) % points.len()]);
            gizmos.line_2d(from, to, color);
        }
    };

    let boundary_color = Color::srgb(0.43, 0.46, 0.54);
    for boundary in frame.geometry.boundaries() {
        draw_ring(&mut gizmos, boundary.points(), boundary_color);
    }

    let region_color = Color::srgb(0.25, 0.61, 0.69);
    for region in frame.geometry.regions() {
        draw_ring(&mut gizmos, region.points(), region_color);
    }

    let path_color = Color::srgb(0.24, 0.82, 0.44);
    for path in frame.geometry.paths() {
        for pair in path.points().windows(2) {
            gizmos.line_2d(to_vec(pair[0]), to_vec(pair[1]), path_color);
        }
    }

    let movement_color = Color::srgb(0.93, 0.46, 0.77);
    for movement in frame.geometry.movements() {
        for pair in movement.points().windows(2) {
            gizmos.line_2d(to_vec(pair[0]), to_vec(pair[1]), movement_color);
        }
        for endpoint in [movement.entry(), movement.exit()] {
            gizmos.circle_2d(to_vec(endpoint), 0.6, movement_color);
        }
    }

    // A facility is drawn over the region it occupies and over the guide path
    // it references, so a band and its reference path read as a facility
    // rather than as authored geometry.
    let facility_color = Color::srgb(0.47, 0.84, 0.84);
    let facility_reference_color = Color::srgb(0.77, 0.66, 0.94);
    for facility in frame.geometry.facilities() {
        draw_ring(&mut gizmos, facility.points(), facility_color);
        if let Some(reference) = facility.reference() {
            for pair in reference.points().windows(2) {
                gizmos.line_2d(to_vec(pair[0]), to_vec(pair[1]), facility_reference_color);
            }
        }
    }

    let conflict_color = Color::srgb(0.90, 0.33, 0.33);
    for conflict in frame.geometry.conflict_regions() {
        draw_ring(&mut gizmos, conflict.points(), conflict_color);
    }

    let crossing_color = Color::srgb(0.75, 0.84, 0.38);
    for crossing in frame.geometry.crossings() {
        draw_ring(&mut gizmos, crossing.points(), crossing_color);
    }

    let portal_color = Color::srgb(0.96, 0.74, 0.22);
    for portal in frame.geometry.portals() {
        let (start, end) = portal.gate();
        gizmos.line_2d(to_vec(start), to_vec(end), portal_color);

        // A short inward arrow shows the direction of travel through the gate.
        let position = portal.position();
        let tip = portal.inward_tip(3.0);
        gizmos.arrow_2d(to_vec(position), to_vec(tip), portal_color);
    }

    let rule_color = Color::srgb(0.67, 0.66, 0.94);
    for rule in frame.geometry.rules() {
        let (sin, cos) = rule.heading().sin_cos();
        let offset = DVec2::new(-sin, cos) * 2.5;
        gizmos.circle_2d(to_vec(rule.position() + offset), 0.8, rule_color);
    }

    let signal_color = Color::srgb(0.94, 0.39, 0.24);
    for signal in frame.geometry.signals() {
        for head in signal.heads() {
            let (sin, cos) = head.heading().sin_cos();
            let offset = DVec2::new(sin, -cos) * 1.5;
            let position = head.position();
            gizmos.line_2d(
                to_vec(position + offset),
                to_vec(position - offset),
                signal_color,
            );
            gizmos.circle_2d(to_vec(position), 0.5, signal_color);
        }
    }
}

/// Draw per-agent velocity arrows and highlight the selected agent.
fn draw_agent_overlays(frame: Res<CurrentFrame>, mut gizmos: Gizmos) {
    let Some(frame) = frame.get() else {
        return;
    };

    if frame.overlays.vectors {
        let vector_color = Color::srgb(0.35, 0.7, 1.0);
        for body in &frame.bodies {
            let Some(speed) = body.speed_mps else {
                continue;
            };
            let (sin, cos) = body.heading_rad.sin_cos();
            let start = Vec2::new(body.position.x as f32, body.position.y as f32);
            let end = Vec2::new(
                start.x + cos as f32 * speed as f32,
                start.y + sin as f32 * speed as f32,
            );
            gizmos.arrow_2d(start, end, vector_color);
        }
    }

    if let Some(body) = frame.selected_body() {
        let point = Vec2::new(body.position.x as f32, body.position.y as f32);
        gizmos.circle_2d(point, 4.0, Color::srgb(1.0, 0.55, 0.2));
    }
}

/// World margin the emphasis ring clears a body's longer axis by, in metres.
const EMPHASIS_RING_MARGIN_M: f64 = 0.6;
/// World margin the inspector's link ring clears a body by, in metres; wider
/// than [`EMPHASIS_RING_MARGIN_M`] so both rings are visible at once.
const LINK_RING_MARGIN_M: f64 = 1.2;
/// Color of an occupied region's overlay ring.
const OCCUPIED_REGION_COLOR: Color = Color::srgb(0.98, 0.62, 0.18);
/// Color of the inspector's link ring on a record's other participants.
const LINK_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
/// Color of a body's usable-corridor segment.
const CORRIDOR_COLOR: Color = Color::srgb(0.35, 0.86, 0.67);
/// Color of a body's target-offset segment.
const TARGET_OFFSET_COLOR: Color = Color::srgb(1.0, 0.84, 0.4);
/// Color of a predicted gap whose target clearance is met.
const PREDICTED_GAP_MARGIN_COLOR: Color = Color::srgb(0.55, 0.85, 0.55);
/// Color of a predicted gap that falls short of its target clearance.
const PREDICTED_GAP_SHORTFALL_COLOR: Color = Color::srgb(0.95, 0.25, 0.25);
/// Color of a predicted gap with no target clearance to compare against.
const PREDICTED_GAP_NEUTRAL_COLOR: Color = Color::srgb(0.62, 0.68, 0.78);
/// Color of a maneuver ring while the body is not maneuvering.
const MANEUVER_IDLE_COLOR: Color = Color::srgb(0.85, 0.87, 0.92);
/// Color of a maneuver ring while the body prepares an edge.
const MANEUVER_PREPARING_COLOR: Color = Color::srgb(0.98, 0.73, 0.15);
/// Color of a maneuver ring while the body is committed to an edge.
const MANEUVER_COMMITTED_COLOR: Color = Color::srgb(0.35, 0.7, 1.0);
/// Color of a maneuver ring while the body returns from an edge.
const MANEUVER_RETURNING_COLOR: Color = Color::srgb(0.55, 0.85, 0.55);
/// Color of a maneuver ring after the body aborts an edge.
const MANEUVER_ABORTED_COLOR: Color = Color::srgb(0.95, 0.25, 0.25);
/// Color of a wrong-way ring whose traversal violates the rule.
const WRONG_WAY_VIOLATION_COLOR: Color = Color::srgb(0.85, 0.35, 0.95);
/// Color of a wrong-way ring whose traversal the rule permits.
const WRONG_WAY_PERMITTED_COLOR: Color = Color::srgb(0.67, 0.66, 0.94);

/// One shape an overlay draws, in world metres.
///
/// The draw systems turn these into `Gizmos` calls. Keeping the derivation a
/// pure function of the frame is what lets an overlay be tested without a Bevy
/// context and keeps every backend on the same shapes.
#[derive(Debug, Clone, PartialEq)]
enum OverlayShape {
    /// A closed ring: consecutive points joined, the last back to the first.
    Ring { points: Vec<DVec2>, color: Color },
    /// A circle at `centre` of `radius_m` world metres.
    Circle {
        centre: DVec2,
        radius_m: f32,
        color: Color,
    },
    /// A straight segment from `from` to `to`.
    Segment {
        from: DVec2,
        to: DVec2,
        color: Color,
    },
}

/// The shapes the frame's safety overlay draws, in draw order.
///
/// Occupied regions first, then emphasized bodies, then the selected body's
/// record participants, and markers last so a conflict sits above the bodies
/// that caused it. Empty when the safety overlay is off, and a record or body
/// the frame does not carry contributes nothing.
fn safety_overlay_shapes(frame: &SceneFrame) -> Vec<OverlayShape> {
    let mut shapes = Vec::new();
    if !frame.overlays.safety {
        return shapes;
    }

    // A region somebody is in is redrawn over the authored ring.
    for region in frame.occupied_regions() {
        let Some(points) = frame.region_points(region.region()) else {
            continue;
        };
        shapes.push(OverlayShape::Ring {
            points: points.to_vec(),
            color: OCCUPIED_REGION_COLOR,
        });
    }

    // An emphasized body gets a ring in the color of its strongest style.
    for (agent, emphasis) in frame.body_emphasis() {
        let Some(body) = frame.body(agent) else {
            continue;
        };
        shapes.push(OverlayShape::Circle {
            centre: body.position,
            radius_m: emphasis_ring_radius(body.length_m.max(body.width_m)),
            color: emphasis_color(emphasis),
        });
    }

    // The inspector's link: the other participants of the selected body's
    // records are ringed so a reader sees who it interacted with.
    if let Some(selected) = frame.selected_body() {
        for record in frame.events_involving(selected.id) {
            for other in EventParticipants::of(record.event()).others(selected.id) {
                if let Some(body) = frame.body(other) {
                    shapes.push(OverlayShape::Circle {
                        centre: body.position,
                        radius_m: link_ring_radius(body.length_m.max(body.width_m)),
                        color: LINK_COLOR,
                    });
                }
            }
        }
    }

    // Markers last, so a conflict sits above the bodies that caused it.
    for marker in frame.safety_markers() {
        shapes.push(OverlayShape::Circle {
            centre: marker.position(),
            radius_m: marker_radius(&marker),
            color: marker_color(&marker),
        });
    }

    shapes
}

/// The shapes the frame's usable-corridor overlay draws, ascending by agent.
///
/// Each corridor is the segment of the facility band the body plus its
/// clearance may occupy, so a viewer sees the room the run steers within. The
/// interval is absolute in the body's travel frame, so the anchor at the body's
/// own offset is displaced by that offset before the interval is laid out.
fn corridor_shapes(frame: &SceneFrame) -> Vec<OverlayShape> {
    if !frame.overlays.corridor {
        return Vec::new();
    }
    frame
        .corridors()
        .into_iter()
        .map(|corridor| OverlayShape::Segment {
            from: corridor.anchor() + corridor.left() * (corridor.d_min_m() - corridor.offset_m()),
            to: corridor.anchor() + corridor.left() * (corridor.d_max_m() - corridor.offset_m()),
            color: CORRIDOR_COLOR,
        })
        .collect()
}

/// The shapes the frame's target-offset overlay draws, ascending by agent.
///
/// The segment runs from the body's own offset to the fixed offset its
/// maneuver displaces toward.
fn target_offset_shapes(frame: &SceneFrame) -> Vec<OverlayShape> {
    if !frame.overlays.target_offset {
        return Vec::new();
    }
    frame
        .target_offsets()
        .into_iter()
        .map(|target| OverlayShape::Segment {
            from: target.anchor(),
            to: target.target(),
            color: TARGET_OFFSET_COLOR,
        })
        .collect()
}

/// The shapes the frame's predicted-gap overlay draws, ascending by agent.
///
/// The circle at the body is the clearance the run predicts over its horizon,
/// colored by whether that clearance meets the mode's target.
fn predicted_gap_shapes(frame: &SceneFrame) -> Vec<OverlayShape> {
    if !frame.overlays.predicted_gap {
        return Vec::new();
    }
    frame
        .predicted_gaps()
        .into_iter()
        .map(|gap| OverlayShape::Circle {
            centre: gap.anchor(),
            radius_m: gap.predicted_min_clearance_m().max(0.0) as f32,
            color: predicted_gap_color(gap.margin_m()),
        })
        .collect()
}

/// The shapes the frame's maneuver overlay draws, ascending by agent.
///
/// A ring around each maneuvering body, colored by its lifecycle state; a body
/// the frame does not carry contributes nothing.
fn maneuver_shapes(frame: &SceneFrame) -> Vec<OverlayShape> {
    if !frame.overlays.maneuver {
        return Vec::new();
    }
    frame
        .maneuver_overlays()
        .into_iter()
        .filter_map(|maneuver| {
            let body = frame.body(maneuver.agent())?;
            Some(OverlayShape::Circle {
                centre: body.position,
                radius_m: emphasis_ring_radius(body.length_m.max(body.width_m)),
                color: maneuver_color(maneuver.state()),
            })
        })
        .collect()
}

/// The shapes the frame's wrong-way overlay draws, ascending by agent.
///
/// A ring around each body with an open opposing traversal, in the wider link
/// ring so it stays distinct from a maneuver ring, colored by whether the
/// traversal violates the rule.
fn wrong_way_shapes(frame: &SceneFrame) -> Vec<OverlayShape> {
    if !frame.overlays.wrong_way {
        return Vec::new();
    }
    frame
        .wrong_way_overlays()
        .into_iter()
        .filter_map(|wrong_way| {
            let body = frame.body(wrong_way.agent())?;
            Some(OverlayShape::Circle {
                centre: body.position,
                radius_m: link_ring_radius(body.length_m.max(body.width_m)),
                color: wrong_way_color(wrong_way.violating()),
            })
        })
        .collect()
}

/// Color of a predicted gap from its margin over the target clearance.
fn predicted_gap_color(margin_m: Option<f64>) -> Color {
    match margin_m {
        Some(margin) if margin < 0.0 => PREDICTED_GAP_SHORTFALL_COLOR,
        Some(_) => PREDICTED_GAP_MARGIN_COLOR,
        None => PREDICTED_GAP_NEUTRAL_COLOR,
    }
}

/// Color of one maneuver lifecycle state.
fn maneuver_color(state: ManeuverState) -> Color {
    match state {
        ManeuverState::Following => MANEUVER_IDLE_COLOR,
        ManeuverState::Preparing => MANEUVER_PREPARING_COLOR,
        ManeuverState::Committed => MANEUVER_COMMITTED_COLOR,
        ManeuverState::Returning => MANEUVER_RETURNING_COLOR,
        ManeuverState::Aborted => MANEUVER_ABORTED_COLOR,
    }
}

/// Color of one wrong-way ring from whether the traversal violates the rule.
fn wrong_way_color(violating: bool) -> Color {
    if violating {
        WRONG_WAY_VIOLATION_COLOR
    } else {
        WRONG_WAY_PERMITTED_COLOR
    }
}

/// World radius of the emphasis ring around a body of longest extent
/// `longest_extent_m`.
fn emphasis_ring_radius(longest_extent_m: f64) -> f32 {
    (longest_extent_m * 0.5 + EMPHASIS_RING_MARGIN_M) as f32
}

/// World radius of the inspector's link ring around a body of longest extent
/// `longest_extent_m`.
fn link_ring_radius(longest_extent_m: f64) -> f32 {
    (longest_extent_m * 0.5 + LINK_RING_MARGIN_M) as f32
}

/// Cast a world-metre position into the Bevy boundary's `f32` vector.
fn to_vec(point: DVec2) -> Vec2 {
    Vec2::new(point.x as f32, point.y as f32)
}

/// Draw the frame's safety overlays: occupied regions, emphasized bodies, the
/// participants of the selected body's records, and event markers.
///
/// Everything drawn here comes from the frame's projected safety data, so the
/// picture is a pure function of the frame the presentation layer handed over.
fn draw_safety_overlays(frame: Res<CurrentFrame>, mut gizmos: Gizmos) {
    let Some(frame) = frame.get() else {
        return;
    };

    draw_shapes(&mut gizmos, safety_overlay_shapes(frame));
}

/// Draw the frame's route-relative tactical overlays: usable corridors, target
/// offsets, predicted gaps, maneuver rings, and wrong-way rings.
///
/// Each overlay's shapes come from its own pure derivation, so the picture is a
/// pure function of the frame and one overlay flag never changes another
/// overlay's shapes.
fn draw_tactical_overlays(frame: Res<CurrentFrame>, mut gizmos: Gizmos) {
    let Some(frame) = frame.get() else {
        return;
    };

    // Corridors, then target offsets, gaps, maneuver rings, and wrong-way
    // rings, matching the declaration order of `Overlay`.
    draw_shapes(
        &mut gizmos,
        corridor_shapes(frame)
            .into_iter()
            .chain(target_offset_shapes(frame))
            .chain(predicted_gap_shapes(frame))
            .chain(maneuver_shapes(frame))
            .chain(wrong_way_shapes(frame)),
    );
}

/// Draw each shape in `shapes` into `gizmos`, in iteration order.
fn draw_shapes(gizmos: &mut Gizmos, shapes: impl IntoIterator<Item = OverlayShape>) {
    for shape in shapes {
        match shape {
            OverlayShape::Ring { points, color } => {
                for index in 0..points.len() {
                    gizmos.line_2d(
                        to_vec(points[index]),
                        to_vec(points[(index + 1) % points.len()]),
                        color,
                    );
                }
            }
            OverlayShape::Circle {
                centre,
                radius_m,
                color,
            } => {
                gizmos.circle_2d(to_vec(centre), radius_m, color);
            }
            OverlayShape::Segment { from, to, color } => {
                gizmos.line_2d(to_vec(from), to_vec(to), color);
            }
        }
    }
}

/// Color of one body emphasis.
fn emphasis_color(emphasis: BodyEmphasis) -> Color {
    match emphasis {
        BodyEmphasis::Collision => Color::srgb(0.95, 0.25, 0.25),
        BodyEmphasis::NearMiss => Color::srgb(0.98, 0.73, 0.15),
        BodyEmphasis::Violation => Color::srgb(0.85, 0.35, 0.95),
        BodyEmphasis::Queue => Color::srgb(0.35, 0.75, 0.95),
        BodyEmphasis::ControlTransition => Color::srgb(0.55, 0.85, 0.55),
    }
}

/// Color of one event marker.
fn marker_color(marker: &SafetyMarker) -> Color {
    use hekate_sim::EventKind;
    match marker.kind() {
        EventKind::Collision => Color::srgb(0.95, 0.25, 0.25),
        EventKind::NearMiss => Color::srgb(0.98, 0.73, 0.15),
        EventKind::Violation => Color::srgb(0.85, 0.35, 0.95),
        EventKind::Entry | EventKind::Exit => Color::srgb(0.98, 0.62, 0.18),
        EventKind::Queue => Color::srgb(0.35, 0.75, 0.95),
        EventKind::Yielded | EventKind::ControlTransition => Color::srgb(0.55, 0.85, 0.55),
        EventKind::Spawned | EventKind::Despawned => Color::WHITE,
        // The increment-2 maneuver and rule records are not markers
        // ([`hekate_present::is_safety_record`] does not carry them).
        EventKind::Maneuver
        | EventKind::FacilityTransition
        | EventKind::OpposingTraversal
        | EventKind::ClosePass => Color::WHITE,
    }
}

/// World radius of one event marker: a bigger ring for a heavier record.
fn marker_radius(marker: &SafetyMarker) -> f32 {
    use hekate_sim::EventKind;
    match marker.kind() {
        EventKind::Collision => 1.6,
        EventKind::NearMiss => 1.2,
        EventKind::Violation => 1.4,
        _ => 0.8,
    }
}

/// Refresh the status strip.
fn update_status_text(
    state: Res<ViewerState>,
    frame: Res<CurrentFrame>,
    mut query: Query<&mut Text, With<StatusText>>,
) {
    let (Some(frame), Ok(mut text)) = (frame.get(), query.single_mut()) else {
        return;
    };

    let paused = if frame.status.paused {
        "paused"
    } else {
        "running"
    };
    let selected = frame
        .status
        .selection
        .map_or_else(|| "none".to_owned(), |id| format!("#{id}"));

    text.0 = format!(
        "Hekate — {scenario}\n\
         sim {seconds:.2} s   tick {tick}\n\
         seed {seed}   speed {speed}   {paused}\n\
         agents {alive}   spawned {spawned}   despawned {despawned}   selected {selected}",
        scenario = frame.scenario_id,
        seconds = frame.time_seconds,
        tick = frame.tick,
        seed = state.seed,
        speed = frame.status.speed.label(),
        alive = frame.status.agents,
        spawned = state.spawned,
        despawned = state.despawned,
    );
}

/// Render the inspector panel for the selected agent.
fn update_inspector_text(
    scenario: Res<Scenario>,
    frame: Res<CurrentFrame>,
    mut query: Query<&mut Text, With<InspectorText>>,
) {
    let (Some(frame), Ok(mut text)) = (frame.get(), query.single_mut()) else {
        return;
    };

    text.0 = match frame.status.selection {
        Some(id) => frame.body(id).map_or_else(
            || format!("Agent #{id} is no longer alive."),
            |body| describe_agent(&scenario.0, body, frame),
        ),
        None => "Click a body to inspect it.".to_owned(),
    };
}

fn describe_agent(scenario: &CompiledScenario, body: &SceneBody, frame: &SceneFrame) -> String {
    let mut out = format!(
        "Agent #{id} ({mode})\n\
         position  ({x:.2}, {y:.2}) m\n\
         heading   {heading:.1}°\n",
        id = body.id,
        mode = body.mode.label(),
        x = body.position.x,
        y = body.position.y,
        heading = body.heading_rad.to_degrees(),
    );

    if let Some(speed) = body.speed_mps {
        let path = body
            .path
            .and_then(|id| scenario.id_map().path_name(id))
            .unwrap_or("<unknown>");
        let route = match body.route {
            Some(id) => scenario.id_map().movement_name(id).unwrap_or("<unknown>"),
            None => "none (static population)",
        };
        out.push_str(&format!(
            "speed     {speed:.2} m/s\n\
             path      {path}\n\
             distance  {distance:.2} m\n\
             body      {length:.2} x {width:.2} m\n\
             route     {route}\n\
             profile   {profile}\n\
             intent    {intent}\n",
            distance = body.path_distance_m.unwrap_or(0.0),
            length = body.length_m,
            width = body.width_m,
            profile = profile_summary(body.profile),
            intent = intent_summary(body.profile),
        ));
    }

    out.push_str(&format!("decision  {}", decision_summary(body.decision)));

    // The route-relative overlays the frame derives for this body: the same
    // shared summaries every backend shows. A body with no route state (Phase
    // 1, Increment 1) carries none, so its inspector is unchanged.
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
            "\ncorridor  {corridor}\n\
             target    {target}\n\
             gap       {gap}\n\
             maneuver  {maneuver}\n\
             rule      {rule}",
            corridor = corridor_summary(corridor.as_ref()),
            target = target_offset_summary(target.as_ref()),
            gap = predicted_gap_summary(gap.as_ref()),
            maneuver = maneuver_summary(maneuver.as_ref()),
            rule = wrong_way_summary(wrong_way.as_ref()),
        ));
    }

    // The event links: what the body was recently part of, and who else was in
    // it. A reader follows an id to the other body's inspector.
    let links = frame.events_involving(body.id);
    if !links.is_empty() {
        out.push_str("\nlinks");
        for record in links.iter().take(MAX_INSPECTOR_LINKS) {
            out.push_str(&format!("\n  {}", event_summary(record)));
        }
        if links.len() > MAX_INSPECTOR_LINKS {
            out.push_str(&format!(
                "\n  ... {} older",
                links.len() - MAX_INSPECTOR_LINKS
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use hekate_model::{
        BodyKind, CrossingId, FacilityId, MovementDirection, MovementId, NominalDirection, PathId,
        PermissionEffect, TacticKind, parse_scenario_source, parse_scenario_source_v2,
    };
    use hekate_present::{
        FrameStatus, Overlays, SafetyOverlay, TacticalOverlay, Viewport, load_scenario,
    };
    use hekate_sim::{
        AgentId, ManeuverEdge, ManeuverReasonCode, PassSide, RegionKey, RouteStateSample,
        WrongWayReason,
    };

    /// Two vehicles on one path through one crossing region, so a pair record
    /// and a region record both have live participants to anchor on.
    fn scenario() -> Arc<CompiledScenario> {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'cross', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'ew', points: [ { x: -20, y: 0 }, { x: 20, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'ew', end: 'start', width_m: 3.5 }, \
             { id: 'east', path: 'ew', end: 'end', width_m: 3.5 } ], \
             regions: [ { id: 'area', points: [ { x: -2, y: -4 }, { x: 2, y: -4 }, \
             { x: 2, y: 4 }, { x: -2, y: 4 } ] } ], \
             movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'ew', \
             priority: 0 } ], \
             crossings: [ { id: 'cross', region: 'area', movements: [ 'ew_through' ] } ], \
             population: { vehicle_count: 2, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        Arc::new(CompiledScenario::compile(source).expect("scenario compiles"))
    }

    /// The one crossing region of [`scenario`].
    fn crossing() -> RegionKey {
        RegionKey::Crossing(CrossingId::from_index(0))
    }

    /// A frame carrying one record of each family a backend draws, at tick 0.
    fn frame(selection: Option<usize>) -> SceneFrame {
        let compiled = scenario();
        let sim = Simulation::new((*compiled).clone(), RunConfig::new(0)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let mut safety = SafetyOverlay::new(40);
        safety.observe(
            0,
            &[
                Event::Entry {
                    agent: AgentId::from_index(0),
                    region: crossing(),
                },
                Event::Collision {
                    agent: AgentId::from_index(0),
                    other: AgentId::from_index(1),
                    clearance_m: -0.4,
                    contacting: true,
                },
                Event::NearMiss {
                    agent: AgentId::from_index(0),
                    other: AgentId::from_index(1),
                    clearance_m: 0.6,
                    entering: true,
                },
                Event::Queue {
                    agent: AgentId::from_index(1),
                    joined: true,
                },
            ],
        );
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Real,
                paused: true,
                selection,
            },
            viewport: Viewport::new(DVec2::ZERO, 1.0),
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: snapshot
                .agents()
                .iter()
                .map(|sample| SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
            safety,
            tactical: TacticalOverlay::default(),
        }
    }

    /// World position of one live body.
    fn body_at(frame: &SceneFrame, id: usize) -> DVec2 {
        frame.body(id).expect("body is alive").position
    }

    /// A circle shape, for a readable expectation.
    fn circle(centre: DVec2, radius_m: f32, color: Color) -> OverlayShape {
        OverlayShape::Circle {
            centre,
            radius_m,
            color,
        }
    }

    #[test]
    fn the_plan_draws_occupancy_emphasis_links_and_markers_from_the_frame() {
        let frame = frame(Some(0));
        let ring = frame
            .region_points(crossing())
            .expect("the crossing has a ring")
            .to_vec();
        let region_centre = frame
            .geometry
            .region_center(crossing())
            .expect("the crossing has a centre");
        let selected = body_at(&frame, 0);
        let other = body_at(&frame, 1);
        let midpoint = (selected + other) / 2.0;

        // The occupied ring, then an emphasis ring per body (contact wins over
        // the standstill of body 1), then one link ring per record the selected
        // body took part in, and markers last in record order.
        let expected = vec![
            OverlayShape::Ring {
                points: ring,
                color: Color::srgb(0.98, 0.62, 0.18),
            },
            circle(selected, 2.85, Color::srgb(0.95, 0.25, 0.25)),
            circle(other, 2.85, Color::srgb(0.95, 0.25, 0.25)),
            circle(other, 3.45, Color::srgb(1.0, 1.0, 1.0)),
            circle(other, 3.45, Color::srgb(1.0, 1.0, 1.0)),
            circle(region_centre, 0.8, Color::srgb(0.98, 0.62, 0.18)),
            circle(midpoint, 1.6, Color::srgb(0.95, 0.25, 0.25)),
            circle(midpoint, 1.2, Color::srgb(0.98, 0.73, 0.15)),
            circle(other, 0.8, Color::srgb(0.35, 0.75, 0.95)),
        ];
        assert_eq!(safety_overlay_shapes(&frame), expected);
    }

    #[test]
    fn the_plan_is_empty_while_the_safety_overlay_is_off() {
        let mut frame = frame(None);
        frame.overlays.safety = false;
        let shapes = safety_overlay_shapes(&frame);
        assert!(shapes.is_empty(), "{shapes:?}");

        frame.overlays.safety = true;
        assert!(!safety_overlay_shapes(&frame).is_empty());
    }

    #[test]
    fn the_link_ring_names_the_other_participants_of_the_selected_body() {
        let link_ring = |frame: &SceneFrame| {
            safety_overlay_shapes(frame)
                .into_iter()
                .filter(|shape| {
                    matches!(shape, OverlayShape::Circle { color, .. } if *color == LINK_COLOR)
                })
                .collect::<Vec<_>>()
        };

        // Nobody selected: no link rings, only the markers.
        assert!(link_ring(&frame(None)).is_empty());

        // Body 0 selected: each of its two pair records rings body 1, the other
        // participant.
        let selected = frame(Some(0));
        let other = body_at(&selected, 1);
        assert_eq!(
            link_ring(&selected),
            vec![circle(other, 3.45, LINK_COLOR); 2]
        );

        // Body 1 selected: the same records ring body 0.
        let selected = frame(Some(1));
        let first = body_at(&selected, 0);
        assert_eq!(
            link_ring(&selected),
            vec![circle(first, 3.45, LINK_COLOR); 2]
        );
    }

    /// The geometry pass draws each facility's occupied band and its reference
    /// path from the frame, so a version-2 fixture must carry both.
    #[test]
    fn a_version_2_frame_carries_each_facility_band_and_reference_path() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scenarios/phase2/inc1/narrow_signal_v2.json5");
        let compiled = load_scenario(&path).expect("fixture loads");
        let geometry = SceneGeometry::from_scenario(&compiled);
        assert_eq!(geometry.facilities().len(), compiled.facilities().len());
        assert!(!geometry.facilities().is_empty());
        for (projected, facility) in geometry.facilities().iter().zip(compiled.facilities()) {
            assert_eq!(projected.id(), facility.id());
            assert!(
                !projected.points().is_empty(),
                "a facility with no band ring"
            );
            let reference = projected
                .reference()
                .unwrap_or_else(|| panic!("facility '{}' has no reference path", facility.name()));
            assert_eq!(Some(reference.path()), facility.reference_path());
            assert!(
                reference.points().len() >= 2,
                "facility '{}' has a reference path with no polyline",
                facility.name()
            );
        }
    }

    /// A version-2 scenario with one 3 m facility whose reference path is the
    /// band's centreline, so a body's signed offset is its world `y`.
    fn band_scenario() -> CompiledScenario {
        const DOCUMENT: &str = "
        {
          schema_version: 2,
          id: 'bands',
          coordinate_system: { x: 'east_m', y: 'north_m' },
          paths: [
            { id: 'centerline', points: [ { x: 0.0, y: 0.0 }, { x: 100.0, y: 0.0 } ] },
          ],
          portals: [],
          regions: [
            { id: 'band', points: [ { x: 0.0, y: -1.5 }, { x: 100.0, y: -1.5 },
              { x: 100.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
          ],
          mode_templates: [
            {
              id: 'mover',
              body: { kind: 'box', length_m: { min: 4.5, max: 4.5 },
                width_m: { min: 1.8, max: 1.8 } },
              motion: 'single_body_wheeled',
              tactics: [ 'follow', 'stop', 'yield' ],
              access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
                speed_policy: { limit_mps: null } },
              occupancy: 'operator_only',
              profiles: {
                speed_mps: { min: 12.0, max: 12.0 },
                max_accel_mps2: { min: 2.0, max: 2.0 },
                comfortable_brake_mps2: { min: 3.0, max: 3.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                compliance: { min: 1.0, max: 1.0 },
              },
            },
          ],
          facilities: [
            { id: 'lane', region: 'band', reference_path: 'centerline',
              width_m: 3.0, nominal_direction: 'forward',
              access: { modes: [ 'mover' ] }, lateral_use: 'shared',
              speed_policy: { limit_mps: null } },
          ],
        }
        ";
        let source = parse_scenario_source_v2(DOCUMENT).expect("band document parses");
        CompiledScenario::compile_v2(source).expect("band document compiles")
    }

    /// A route-relative sample at offset `d_m`, with the mode's resolved
    /// clearance and horizon and every sparse field absent.
    fn route_state(d_m: f64) -> RouteStateSample {
        RouteStateSample {
            s_m: 50.0,
            d_m,
            maneuver_state: ManeuverState::Following,
            target_offset_m: None,
            target_facility: None,
            predicted_min_clearance_m: None,
            target_clearance_m: Some(0.3),
            horizon_s: Some(2.0),
            perceived_rule: None,
            opposing_direction: None,
        }
    }

    /// A body on the band's reference path, at `position`, carrying `state`.
    fn route_body(id: usize, position: DVec2, state: Option<RouteStateSample>) -> SceneBody {
        SceneBody {
            id,
            position,
            heading_rad: 0.0,
            length_m: 4.5,
            width_m: 1.8,
            mode: hekate_sim::AgentMode::Vehicle,
            body_kind: BodyKind::Box,
            segments: Vec::new(),
            speed_mps: Some(12.0),
            path: Some(PathId::from_index(0)),
            path_distance_m: Some(position.x),
            route: None,
            profile: None,
            decision: None,
            route_state: state,
        }
    }

    /// A frame over [`band_scenario`] carrying `bodies` and `tactical`.
    fn band_frame(bodies: Vec<SceneBody>, tactical: TacticalOverlay) -> SceneFrame {
        let scenario = band_scenario();
        SceneFrame {
            scenario_id: scenario.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: bodies.len(),
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport: Viewport::new(DVec2::ZERO, 1.0),
            geometry: Arc::new(SceneGeometry::from_scenario(&scenario)),
            bodies,
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
            tactical,
        }
    }

    /// A maneuver edge for `agent` at offset 0.8 m, attempted with one partner.
    fn maneuver_edge(agent: usize, to: ManeuverState, edge: ManeuverEdge) -> Event {
        Event::Maneuver {
            agent: AgentId::from_index(agent),
            kind: TacticKind::Overtake,
            from: ManeuverState::Preparing,
            to,
            edge,
            partner: Some(AgentId::from_index(1)),
            source_facility: FacilityId::from_index(0),
            target_facility: None,
            target_offset_m: 0.8,
            side: PassSide::Left,
            reason: ManeuverReasonCode::SlowerLeader,
        }
    }

    /// An entering or leaving opposing traversal for `agent` on facility 0.
    fn opposing_edge(agent: usize, entering: bool) -> Event {
        Event::OpposingTraversal {
            agent: AgentId::from_index(agent),
            facility: FacilityId::from_index(0),
            movement: Some(MovementId::from_index(0)),
            direction: MovementDirection::Reverse,
            nominal_direction: NominalDirection::Forward,
            perceived_rule: Some(PermissionEffect::Prohibit),
            reason: WrongWayReason::NoncompliantChoice,
            violating: true,
            entering,
        }
    }

    /// Assert two shape plans match to a micrometre, so a derivation on
    /// world metres is compared for geometry rather than for its last bit.
    fn assert_shapes(actual: &[OverlayShape], expected: &[OverlayShape]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
        for (actual, expected) in actual.iter().zip(expected) {
            match (actual, expected) {
                (
                    OverlayShape::Ring {
                        points: ours,
                        color: our_color,
                    },
                    OverlayShape::Ring {
                        points: theirs,
                        color: their_color,
                    },
                ) => {
                    assert_eq!((our_color, ours.len()), (their_color, theirs.len()));
                    for (ours, theirs) in ours.iter().zip(theirs) {
                        assert!((*ours - *theirs).length() < 1e-9, "{ours:?} != {theirs:?}");
                    }
                }
                (
                    OverlayShape::Circle {
                        centre: our_centre,
                        radius_m: our_radius,
                        color: our_color,
                    },
                    OverlayShape::Circle {
                        centre: their_centre,
                        radius_m: their_radius,
                        color: their_color,
                    },
                ) => {
                    assert!(
                        (*our_centre - *their_centre).length() < 1e-9
                            && (our_radius - their_radius).abs() < 1e-6
                            && our_color == their_color,
                        "{actual:?} != {expected:?}"
                    );
                }
                (
                    OverlayShape::Segment {
                        from: our_from,
                        to: our_to,
                        color: our_color,
                    },
                    OverlayShape::Segment {
                        from: their_from,
                        to: their_to,
                        color: their_color,
                    },
                ) => {
                    assert!(
                        (*our_from - *their_from).length() < 1e-9
                            && (*our_to - *their_to).length() < 1e-9
                            && our_color == their_color,
                        "{actual:?} != {expected:?}"
                    );
                }
                _ => panic!("{actual:?} != {expected:?}"),
            }
        }
    }

    /// Every route-relative overlay the frame derives draws its own shapes, in
    /// ascending agent order, from the frame's projected data alone.
    #[test]
    fn each_route_relative_overlay_derives_its_shapes_from_the_frame() {
        let mut tactical = TacticalOverlay::default();
        tactical.observe(
            7,
            &[
                maneuver_edge(0, ManeuverState::Committed, ManeuverEdge::Committed),
                opposing_edge(1, true),
            ],
        );
        let mut first = route_state(0.0);
        first.maneuver_state = ManeuverState::Committed;
        first.target_offset_m = Some(0.8);
        first.predicted_min_clearance_m = Some(0.9);
        first.target_clearance_m = Some(0.5);
        let mut second = route_state(0.2);
        second.predicted_min_clearance_m = Some(0.4);
        second.target_clearance_m = Some(0.5);
        let frame = band_frame(
            vec![
                route_body(0, DVec2::new(50.0, 0.0), Some(first)),
                route_body(1, DVec2::new(60.0, 0.2), Some(second)),
            ],
            tactical,
        );

        // The corridor is the band inset by the body and its clearance: an
        // absolute interval both bodies share, drawn from the body's own
        // offset. Both bodies sit inside the same 3 m band, so both draw the
        // same +/-0.1 m interval, each at its own position.
        assert_shapes(
            &corridor_shapes(&frame),
            &[
                OverlayShape::Segment {
                    from: DVec2::new(50.0, -0.1),
                    to: DVec2::new(50.0, 0.1),
                    color: CORRIDOR_COLOR,
                },
                OverlayShape::Segment {
                    from: DVec2::new(60.0, -0.1),
                    to: DVec2::new(60.0, 0.1),
                    color: CORRIDOR_COLOR,
                },
            ],
        );
        assert_eq!(
            target_offset_shapes(&frame),
            vec![OverlayShape::Segment {
                from: DVec2::new(50.0, 0.0),
                to: DVec2::new(50.0, 0.8),
                color: TARGET_OFFSET_COLOR,
            }]
        );
        // A met target is sage and a shortfall red, so both margins show.
        assert_eq!(
            predicted_gap_shapes(&frame),
            vec![
                circle(DVec2::new(50.0, 0.0), 0.9, PREDICTED_GAP_MARGIN_COLOR),
                circle(DVec2::new(60.0, 0.2), 0.4, PREDICTED_GAP_SHORTFALL_COLOR),
            ]
        );
        // The maneuver ring uses the live sample's state, and the wrong-way
        // ring the interval's violation, each around its own body.
        assert_eq!(
            maneuver_shapes(&frame),
            vec![circle(
                DVec2::new(50.0, 0.0),
                2.85,
                MANEUVER_COMMITTED_COLOR
            )]
        );
        assert_eq!(
            wrong_way_shapes(&frame),
            vec![circle(
                DVec2::new(60.0, 0.2),
                3.45,
                WRONG_WAY_VIOLATION_COLOR
            )]
        );
    }

    /// Every overlay flag turns off only its own shapes, and a frame whose
    /// bodies carry no route state and no open interval draws none of them.
    #[test]
    fn every_route_relative_overlay_is_flag_gated_and_absent_for_phase_1() {
        let mut tactical = TacticalOverlay::default();
        tactical.observe(
            7,
            &[
                maneuver_edge(0, ManeuverState::Committed, ManeuverEdge::Committed),
                opposing_edge(0, true),
            ],
        );
        let mut state = route_state(0.0);
        state.maneuver_state = ManeuverState::Committed;
        state.target_offset_m = Some(0.8);
        state.predicted_min_clearance_m = Some(0.9);
        let mut frame = band_frame(vec![route_body(0, DVec2::ZERO, Some(state))], tactical);
        assert!(!corridor_shapes(&frame).is_empty());

        for (flag, shapes) in [
            (
                "corridor",
                corridor_shapes as fn(&SceneFrame) -> Vec<OverlayShape>,
            ),
            ("target_offset", target_offset_shapes),
            ("predicted_gap", predicted_gap_shapes),
            ("maneuver", maneuver_shapes),
            ("wrong_way", wrong_way_shapes),
        ] {
            let before = shapes(&frame);
            assert!(!before.is_empty(), "{flag} draws nothing to gate");
            let others = |frame: &SceneFrame| {
                [
                    corridor_shapes(frame),
                    target_offset_shapes(frame),
                    predicted_gap_shapes(frame),
                    maneuver_shapes(frame),
                    wrong_way_shapes(frame),
                ]
            };
            let untouched = others(&frame);

            match flag {
                "corridor" => frame.overlays.corridor = false,
                "target_offset" => frame.overlays.target_offset = false,
                "predicted_gap" => frame.overlays.predicted_gap = false,
                "maneuver" => frame.overlays.maneuver = false,
                _ => frame.overlays.wrong_way = false,
            }
            assert!(shapes(&frame).is_empty(), "{flag} stayed on");
            let after = others(&frame);
            for (index, (before, after)) in untouched.iter().zip(after.iter()).enumerate() {
                let mine = match flag {
                    "corridor" => 0,
                    "target_offset" => 1,
                    "predicted_gap" => 2,
                    "maneuver" => 3,
                    _ => 4,
                };
                if index != mine {
                    assert_eq!(before, after, "{flag} changed overlay {index}");
                }
            }
            match flag {
                "corridor" => frame.overlays.corridor = true,
                "target_offset" => frame.overlays.target_offset = true,
                "predicted_gap" => frame.overlays.predicted_gap = true,
                "maneuver" => frame.overlays.maneuver = true,
                _ => frame.overlays.wrong_way = true,
            }
        }

        // A Phase 1 body carries no route state and no interval, so a frame
        // with the overlays on draws nothing at all.
        let phase_one = band_frame(vec![route_body(0, DVec2::ZERO, None)], {
            let mut tactical = TacticalOverlay::default();
            tactical.observe(
                7,
                &[maneuver_edge(
                    0,
                    ManeuverState::Committed,
                    ManeuverEdge::Committed,
                )],
            );
            tactical
        });
        assert!(corridor_shapes(&phase_one).is_empty());
        assert!(target_offset_shapes(&phase_one).is_empty());
        assert!(predicted_gap_shapes(&phase_one).is_empty());
        assert!(wrong_way_shapes(&phase_one).is_empty());
        // The interval's own edge names the state when the body carries no
        // sample, so the maneuver ring still draws.
        assert_eq!(
            maneuver_shapes(&phase_one),
            vec![circle(DVec2::ZERO, 2.85, MANEUVER_COMMITTED_COLOR)]
        );
    }

    /// The inspector names the route-relative identifiers the shared summaries
    /// carry: the corridor's clearance, the maneuver's partner and facility,
    /// and the rule's reason and state.
    #[test]
    fn the_inspector_names_the_route_relative_identifiers() {
        let mut tactical = TacticalOverlay::default();
        tactical.observe(
            7,
            &[
                maneuver_edge(0, ManeuverState::Committed, ManeuverEdge::Committed),
                opposing_edge(0, true),
            ],
        );
        let mut state = route_state(0.0);
        state.maneuver_state = ManeuverState::Committed;
        state.target_offset_m = Some(0.8);
        state.predicted_min_clearance_m = Some(0.9);
        state.target_clearance_m = Some(0.5);
        state.perceived_rule = Some(PermissionEffect::Prohibit);
        state.opposing_direction = Some(MovementDirection::Reverse);
        let scenario = band_scenario();
        let frame = band_frame(vec![route_body(0, DVec2::ZERO, Some(state))], tactical);
        let body = frame.body(0).expect("body is alive");

        let text = describe_agent(&scenario, body, &frame);
        assert!(
            text.contains("corridor  #0  facility 0  usable corridor"),
            "{text}"
        );
        assert!(text.contains("clearance 0.50 m"), "{text}");
        assert!(
            text.contains("target    #0  target offset 0.80 m (left)"),
            "{text}"
        );
        assert!(
            text.contains("gap       #0  predicted min clearance 0.90 m"),
            "{text}"
        );
        assert!(
            text.contains(
                "maneuver  #0  maneuver committed  tactic overtake  edge committed  \
                 reason slower_leader  partner #1  target 0.80 m (left)  facility 0 -> none"
            ),
            "{text}"
        );
        assert!(
            text.contains("rule      #0  opposing facility 0  movement 0"),
            "{text}"
        );
        assert!(text.contains("violating true"), "{text}");

        // A body with no route state gains no new lines.
        let bare = band_frame(vec![route_body(0, DVec2::ZERO, None)], {
            let mut tactical = TacticalOverlay::default();
            tactical.observe(
                7,
                &[maneuver_edge(
                    0,
                    ManeuverState::Committed,
                    ManeuverEdge::Committed,
                )],
            );
            tactical
        });
        let bare_text = describe_agent(&scenario, bare.body(0).expect("alive"), &bare);
        assert!(!bare_text.contains("corridor  "), "{bare_text}");
    }
}
