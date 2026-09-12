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
    Applied, Overlay, PresentationController, RendererBackend, RestartMode, SceneBody,
    SceneGeometry, Speed, ViewCommand, Viewport, load_scenario,
};
use tangle_sim::{Event, RunConfig, Simulation, Snapshot, SnapshotDetail};
use tangle_viewer::CurrentFrame;

/// Scenario used when no path is passed on the command line.
const DEFAULT_SCENARIO: &str = "scenarios/walking/walking_guide_v1.json5";

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

/// Shared mesh and material handles. Every car reuses these; there is no
/// per-agent mesh or material allocation.
#[derive(Resource)]
struct AgentAssets {
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
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
        mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        material: materials.add(Color::srgb(0.85, 0.87, 0.92)),
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
             G geometry    V vectors    click a car to inspect    esc clear",
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
            let output = state.sim.step();
            for event in output.events() {
                match event {
                    Event::Spawned { .. } => spawned += 1,
                    Event::Despawned { .. } => despawned += 1,
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

/// Create, move, and retire car entities from the current frame.
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
            let entity = commands
                .spawn((
                    AgentVisual,
                    Mesh2d(assets.mesh.clone()),
                    MeshMaterial2d(assets.material.clone()),
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
            |body| describe_agent(&scenario.0, body),
        ),
        None => "Click a car to inspect it.".to_owned(),
    };
}

fn describe_agent(scenario: &CompiledScenario, body: &SceneBody) -> String {
    let mut out = format!(
        "Agent #{id}\n\
         position  ({x:.2}, {y:.2}) m\n\
         heading   {heading:.1}°\n",
        id = body.id,
        x = body.position.x,
        y = body.position.y,
        heading = body.heading_rad.to_degrees(),
    );

    if let Some(speed) = body.speed_mps {
        let path = body
            .path
            .and_then(|id| scenario.id_map().path_name(id))
            .unwrap_or("<unknown>");
        out.push_str(&format!(
            "speed     {speed:.2} m/s\n\
             path      {path}\n\
             distance  {distance:.2} m\n\
             body      {length:.2} x {width:.2} m\n",
            distance = body.path_distance_m.unwrap_or(0.0),
            length = body.length_m,
            width = body.width_m,
        ));
        out.push_str("intent    hold constant speed along the guide path\n");
    }

    out.push_str("decision  none yet (Increment 0 has no decisions)");
    out
}
