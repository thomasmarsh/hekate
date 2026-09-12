//! Bevy top-down viewer for Tangle.
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
use tangle_model::CompiledScenario;
use tangle_present::{
    Applied, BodyEmphasis, EventParticipants, Overlay, PresentationController, RendererBackend,
    RestartMode, SafetyMarker, SceneBody, SceneFrame, SceneGeometry, Speed, ViewCommand, Viewport,
    decision_summary, event_summary, intent_summary, load_scenario, profile_summary,
};
use tangle_sim::{AgentMode, Event, RunConfig, Simulation, Snapshot, SnapshotDetail};
use tangle_viewer::CurrentFrame;

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
                title: "Tangle — Phase 1 walking skeleton".to_owned(),
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

/// Shared mesh and material handles. Every vehicle reuses one mesh and
/// material and every pedestrian another; there is no per-agent mesh or
/// material allocation.
#[derive(Resource)]
struct AgentAssets {
    vehicle_mesh: Handle<Mesh>,
    pedestrian_mesh: Handle<Mesh>,
    vehicle_material: Handle<ColorMaterial>,
    pedestrian_material: Handle<ColorMaterial>,
}

/// Maps an agent's stable id to its rendered entity so entities are reused
/// across frames and despawns rather than recreated every frame.
#[derive(Resource, Default)]
struct Visuals(HashMap<usize, Entity>);

/// Marks a rendered car entity.
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
        // A vehicle body is a box scaled to its length and width; a pedestrian
        // body is the circle its reported diameter inscribes.
        vehicle_mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        pedestrian_mesh: meshes.add(Circle::new(0.5)),
        vehicle_material: materials.add(Color::srgb(0.85, 0.87, 0.92)),
        pedestrian_material: materials.add(Color::srgb(0.65, 0.90, 0.75)),
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
             G geometry    V vectors    B safety    click a body to inspect    esc clear",
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
                    | Event::ControlTransition { .. } => {}
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
/// An entity keeps the mesh and material of the mode it spawned with: an
/// agent's mode is fixed for the life of its slot, so a body never has to
/// change either.
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

    let mut live = Vec::with_capacity(frame.bodies.len());
    for body in &frame.bodies {
        let id = body.id;
        let transform = Transform {
            translation: Vec3::new(body.position.x as f32, body.position.y as f32, 1.0),
            rotation: Quat::from_rotation_z(body.heading_rad as f32),
            scale: Vec3::new(body.length_m as f32, body.width_m as f32, 1.0),
        };

        if let Some(&entity) = visuals.0.get(&id) {
            if let Ok(mut current) = transforms.get_mut(entity) {
                *current = transform;
            }
        } else {
            let (mesh, material) = match body.mode {
                AgentMode::Vehicle => {
                    (assets.vehicle_mesh.clone(), assets.vehicle_material.clone())
                }
                AgentMode::Pedestrian => (
                    assets.pedestrian_mesh.clone(),
                    assets.pedestrian_material.clone(),
                ),
            };
            let entity = commands
                .spawn((
                    AgentVisual,
                    Mesh2d(mesh),
                    MeshMaterial2d(material),
                    transform,
                ))
                .id();
            visuals.0.insert(id, entity);
        }
        live.push(id);
    }

    visuals.0.retain(|id, entity| {
        if live.contains(id) {
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

/// One shape the safety overlay draws, in world metres.
///
/// The draw system turns these into `Gizmos` calls. Keeping the derivation a
/// pure function of the frame is what lets the overlay be tested without a Bevy
/// context and keeps every backend on the same shapes.
#[derive(Debug, Clone, PartialEq)]
enum SafetyOverlayShape {
    /// A closed ring: consecutive points joined, the last back to the first.
    Ring { points: Vec<DVec2>, color: Color },
    /// A circle at `centre` of `radius_m` world metres.
    Circle {
        centre: DVec2,
        radius_m: f32,
        color: Color,
    },
}

/// The shapes the frame's safety overlay draws, in draw order.
///
/// Occupied regions first, then emphasized bodies, then the selected body's
/// record participants, and markers last so a conflict sits above the bodies
/// that caused it. Empty when the safety overlay is off, and a record or body
/// the frame does not carry contributes nothing.
fn safety_overlay_shapes(frame: &SceneFrame) -> Vec<SafetyOverlayShape> {
    let mut shapes = Vec::new();
    if !frame.overlays.safety {
        return shapes;
    }

    // A region somebody is in is redrawn over the authored ring.
    for region in frame.occupied_regions() {
        let Some(points) = frame.region_points(region.region()) else {
            continue;
        };
        shapes.push(SafetyOverlayShape::Ring {
            points: points.to_vec(),
            color: OCCUPIED_REGION_COLOR,
        });
    }

    // An emphasized body gets a ring in the color of its strongest style.
    for (agent, emphasis) in frame.body_emphasis() {
        let Some(body) = frame.body(agent) else {
            continue;
        };
        shapes.push(SafetyOverlayShape::Circle {
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
                    shapes.push(SafetyOverlayShape::Circle {
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
        shapes.push(SafetyOverlayShape::Circle {
            centre: marker.position(),
            radius_m: marker_radius(marker),
            color: marker_color(marker),
        });
    }

    shapes
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

    for shape in safety_overlay_shapes(frame) {
        match shape {
            SafetyOverlayShape::Ring { points, color } => {
                for index in 0..points.len() {
                    gizmos.line_2d(
                        to_vec(points[index]),
                        to_vec(points[(index + 1) % points.len()]),
                        color,
                    );
                }
            }
            SafetyOverlayShape::Circle {
                centre,
                radius_m,
                color,
            } => {
                gizmos.circle_2d(to_vec(centre), radius_m, color);
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
fn marker_color(marker: SafetyMarker) -> Color {
    use tangle_sim::EventKind;
    match marker.kind() {
        EventKind::Collision => Color::srgb(0.95, 0.25, 0.25),
        EventKind::NearMiss => Color::srgb(0.98, 0.73, 0.15),
        EventKind::Violation => Color::srgb(0.85, 0.35, 0.95),
        EventKind::Entry | EventKind::Exit => Color::srgb(0.98, 0.62, 0.18),
        EventKind::Queue => Color::srgb(0.35, 0.75, 0.95),
        EventKind::Yielded | EventKind::ControlTransition => Color::srgb(0.55, 0.85, 0.55),
        EventKind::Spawned | EventKind::Despawned => Color::WHITE,
    }
}

/// World radius of one event marker: a bigger ring for a heavier record.
fn marker_radius(marker: SafetyMarker) -> f32 {
    use tangle_sim::EventKind;
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
        "Tangle — {scenario}\n\
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

    // The event links: what the body was recently part of, and who else was in
    // it. A reader follows an id to the other body's inspector.
    let links = frame.events_involving(body.id);
    if !links.is_empty() {
        out.push_str("\nlinks");
        for record in links.iter().take(MAX_INSPECTOR_LINKS) {
            out.push_str(&format!("\n  {}", event_summary(*record)));
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

    use tangle_model::{CrossingId, parse_scenario_source};
    use tangle_present::{FrameStatus, Overlays, SafetyOverlay, Viewport};
    use tangle_sim::{AgentId, RegionKey};

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
        }
    }

    /// World position of one live body.
    fn body_at(frame: &SceneFrame, id: usize) -> DVec2 {
        frame.body(id).expect("body is alive").position
    }

    /// A circle shape, for a readable expectation.
    fn circle(centre: DVec2, radius_m: f32, color: Color) -> SafetyOverlayShape {
        SafetyOverlayShape::Circle {
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
            SafetyOverlayShape::Ring {
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
                    matches!(shape, SafetyOverlayShape::Circle { color, .. } if *color == LINK_COLOR)
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
}
