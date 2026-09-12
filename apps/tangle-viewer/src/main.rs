//! Bevy top-down viewer for Tangle.
//!
//! The viewer owns one [`Simulation`] and drives it only in whole fixed steps.
//! Presentation time is accumulated in [`PresentationClock`], so pause, speed,
//! and frame rate choose which ticks are observed but never change kernel
//! results. Rendering reads the lossy [`Snapshot`] observer view and casts the
//! kernel's `f64` metres into Bevy `f32` transforms at this boundary only.
//!
//! This is the only crate permitted to depend on Bevy.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use tangle_model::CompiledScenario;
use tangle_sim::{AgentSample, Event, RunConfig, Simulation, Snapshot, SnapshotDetail};
use tangle_viewer::{PresentationClock, Speed, load_scenario};

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
    let state = ViewerState {
        sim,
        clock: PresentationClock::new(step_secs),
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
        .init_resource::<Visuals>()
        .init_resource::<Overlays>()
        .init_resource::<Inspector>()
        .add_systems(Startup, (setup, draw_help_text).chain())
        .add_systems(
            Update,
            (
                controls,
                advance_simulation,
                sync_agents,
                camera_controls,
                click_select,
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

/// Live kernel, presentation clock, and the two snapshots interpolated between.
#[derive(Resource)]
struct ViewerState {
    sim: Simulation,
    clock: PresentationClock,
    prev: Snapshot,
    curr: Snapshot,
    seed: u64,
    spawned: u64,
    despawned: u64,
}

impl ViewerState {
    /// Rebuild the kernel for `seed`, preserving playback speed and pause state.
    fn restart(&mut self, scenario: &CompiledScenario, seed: u64) {
        let Ok(sim) = Simulation::new(scenario.clone(), RunConfig::new(seed)) else {
            return;
        };
        let snapshot = sim.snapshot(SnapshotDetail::Full);

        let mut clock = PresentationClock::new(sim.config().step().as_secs());
        clock.set_speed(self.clock.speed());
        clock.set_paused(self.clock.is_paused());

        self.seed = seed;
        self.sim = sim;
        self.clock = clock;
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

/// Which optional debug overlays are enabled.
#[derive(Resource)]
struct Overlays {
    geometry: bool,
    vectors: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            geometry: true,
            vectors: false,
        }
    }
}

/// The agent selected by the most recent click, if any.
#[derive(Resource, Default)]
struct Inspector {
    selected: Option<usize>,
}

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
    scenario: Res<Scenario>,
    window: Single<&Window>,
) {
    let (min, max) =
        scenario_bounds(&scenario.0).unwrap_or((-Vec2::splat(20.0), Vec2::splat(20.0)));
    let center = (min + max) * 0.5;
    let span = (max - min).max(Vec2::splat(1.0));
    let size = window.size().max(Vec2::splat(1.0));
    let scale = (span.x / size.x).max(span.y / size.y) * 1.25;

    commands.spawn((
        Camera2d,
        Transform::from_xyz(center.x, center.y, 999.0),
        Projection::Orthographic(OrthographicProjection {
            scale,
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

/// Keyboard controls for playback, restart, and overlays.
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ViewerState>,
    scenario: Res<Scenario>,
    mut overlays: ResMut<Overlays>,
    mut inspector: ResMut<Inspector>,
) {
    if keys.just_pressed(KeyCode::Space) {
        state.clock.toggle_paused();
    }
    if keys.just_pressed(KeyCode::Period) {
        state.clock.request_single_tick();
    }
    if keys.just_pressed(KeyCode::Digit1) {
        state.clock.set_speed(Speed::Real);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        state.clock.set_speed(Speed::Fast);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        state.clock.set_speed(Speed::Maximum);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let seed = state.seed;
        state.restart(&scenario.0, seed);
        inspector.selected = None;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        let seed = state.seed.wrapping_add(1);
        state.restart(&scenario.0, seed);
        inspector.selected = None;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        overlays.geometry = !overlays.geometry;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        overlays.vectors = !overlays.vectors;
    }
    if keys.just_pressed(KeyCode::Escape) {
        inspector.selected = None;
    }
}

/// Take the whole steps the presentation clock buys this frame.
fn advance_simulation(time: Res<Time>, mut state: ResMut<ViewerState>) {
    let ticks = state.clock.advance(f64::from(time.delta_secs()));
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

/// Create, move, and retire car entities from the current snapshot.
fn sync_agents(
    mut commands: Commands,
    state: Res<ViewerState>,
    assets: Res<AgentAssets>,
    mut visuals: ResMut<Visuals>,
    mut transforms: Query<&mut Transform, With<AgentVisual>>,
) {
    let alpha = state.clock.alpha();
    let mut live = Vec::with_capacity(state.curr.agents().len());

    for sample in state.curr.agents() {
        let id = sample.id.get() as usize;
        let (position, heading) = interpolate(state.prev.agents(), sample, alpha);
        let (length, width) = sample.motion.map_or((4.5_f64, 1.8_f64), |motion| {
            (motion.body_length_m, motion.body_width_m)
        });

        let transform = Transform {
            translation: Vec3::new(position.x as f32, position.y as f32, 1.0),
            rotation: Quat::from_rotation_z(heading as f32),
            scale: Vec3::new(length as f32, width as f32, 1.0),
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

/// Pan with WASD and zoom with the wheel.
fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };

    let Projection::Orthographic(orthographic) = &mut *projection else {
        return;
    };

    if scroll.delta.y != 0.0 {
        orthographic.scale = (orthographic.scale * (1.0 - scroll.delta.y * 0.1)).clamp(0.02, 64.0);
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
        let delta = direction.normalize() * 40.0 * orthographic.scale * time.delta_secs();
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}

/// Click the car nearest the cursor to select it for inspection.
fn click_select(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform, &Projection)>,
    state: Res<ViewerState>,
    mut inspector: ResMut<Inspector>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let (camera, transform, projection) = *camera;
    let Ok(world) = camera.viewport_to_world_2d(transform, cursor) else {
        return;
    };
    let scale = match projection {
        Projection::Orthographic(orthographic) => orthographic.scale,
        _ => 1.0,
    };

    let mut best: Option<(usize, f32)> = None;
    for sample in state.curr.agents() {
        let point = Vec2::new(sample.position.x as f32, sample.position.y as f32);
        let distance = point.distance(world);
        if best.is_none_or(|(_, current)| distance < current) {
            best = Some((sample.id.get() as usize, distance));
        }
    }

    inspector.selected = match best {
        Some((id, distance)) if distance <= 12.0 * scale => Some(id),
        _ => None,
    };
}

/// Draw the guide path and portals each frame.
fn draw_geometry(scenario: Res<Scenario>, overlays: Res<Overlays>, mut gizmos: Gizmos) {
    if !overlays.geometry {
        return;
    }

    let path_color = Color::srgb(0.24, 0.82, 0.44);
    for path in scenario.0.paths() {
        for pair in path.points().windows(2) {
            gizmos.line_2d(
                Vec2::new(pair[0].x as f32, pair[0].y as f32),
                Vec2::new(pair[1].x as f32, pair[1].y as f32),
                path_color,
            );
        }
    }

    let portal_color = Color::srgb(0.96, 0.74, 0.22);
    for portal in scenario.0.portals() {
        let position = portal.position();
        let heading = portal.heading();
        let half_width = portal.width_m() * 0.5;
        // The portal gate spans the path: perpendicular to its inward heading.
        let (sin, cos) = heading.sin_cos();
        let start = Vec2::new(
            (position.x + sin * half_width) as f32,
            (position.y - cos * half_width) as f32,
        );
        let end = Vec2::new(
            (position.x - sin * half_width) as f32,
            (position.y + cos * half_width) as f32,
        );
        gizmos.line_2d(start, end, portal_color);

        // A short inward arrow shows the direction of travel through the gate.
        let tip = Vec2::new(
            (position.x + cos * 3.0) as f32,
            (position.y + sin * 3.0) as f32,
        );
        gizmos.arrow_2d(
            Vec2::new(position.x as f32, position.y as f32),
            tip,
            portal_color,
        );
    }
}

/// Draw per-agent velocity arrows and highlight the selected agent.
fn draw_agent_overlays(
    state: Res<ViewerState>,
    overlays: Res<Overlays>,
    inspector: Res<Inspector>,
    mut gizmos: Gizmos,
) {
    if overlays.vectors {
        let vector_color = Color::srgb(0.35, 0.7, 1.0);
        for sample in state.curr.agents() {
            let Some(motion) = sample.motion else {
                continue;
            };
            let (sin, cos) = sample.heading_rad.sin_cos();
            let start = Vec2::new(sample.position.x as f32, sample.position.y as f32);
            let end = Vec2::new(
                start.x + cos as f32 * motion.speed_mps as f32,
                start.y + sin as f32 * motion.speed_mps as f32,
            );
            gizmos.arrow_2d(start, end, vector_color);
        }
    }

    if let Some(selected) = inspector.selected
        && let Some(sample) = state
            .curr
            .agents()
            .iter()
            .find(|sample| sample.id.get() as usize == selected)
    {
        let point = Vec2::new(sample.position.x as f32, sample.position.y as f32);
        gizmos.circle_2d(point, 4.0, Color::srgb(1.0, 0.55, 0.2));
    }
}

/// Refresh the status strip.
fn update_status_text(
    state: Res<ViewerState>,
    inspector: Res<Inspector>,
    mut query: Query<&mut Text, With<StatusText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let time = state.curr.time();
    let paused = if state.clock.is_paused() {
        "paused"
    } else {
        "running"
    };
    let selected = inspector
        .selected
        .map_or_else(|| "none".to_owned(), |id| format!("#{id}"));

    text.0 = format!(
        "Tangle — {scenario}\n\
         sim {seconds:.2} s   tick {tick}\n\
         seed {seed}   speed {speed}   {paused}\n\
         agents {alive}   spawned {spawned}   despawned {despawned}   selected {selected}",
        scenario = state.curr.scenario_id(),
        seconds = time.seconds(),
        tick = time.tick(),
        seed = state.seed,
        speed = state.clock.speed().label(),
        alive = state.curr.agents().len(),
        spawned = state.spawned,
        despawned = state.despawned,
    );
}

/// Render the inspector panel for the selected agent.
fn update_inspector_text(
    scenario: Res<Scenario>,
    state: Res<ViewerState>,
    inspector: Res<Inspector>,
    mut query: Query<&mut Text, With<InspectorText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    text.0 = match inspector.selected {
        Some(id) => state
            .curr
            .agents()
            .iter()
            .find(|sample| sample.id.get() as usize == id)
            .map_or_else(
                || format!("Agent #{id} is no longer alive."),
                |sample| describe_agent(&scenario.0, sample),
            ),
        None => "Click a car to inspect it.".to_owned(),
    };
}

fn describe_agent(scenario: &CompiledScenario, sample: &AgentSample) -> String {
    let mut out = format!(
        "Agent #{id}\n\
         position  ({x:.2}, {y:.2}) m\n\
         heading   {heading:.1}°\n",
        id = sample.id.get(),
        x = sample.position.x,
        y = sample.position.y,
        heading = sample.heading_rad.to_degrees(),
    );

    if let Some(motion) = sample.motion {
        let path = scenario
            .id_map()
            .path_name(motion.path)
            .unwrap_or("<unknown>");
        out.push_str(&format!(
            "speed     {speed:.2} m/s\n\
             path      {path}\n\
             distance  {distance:.2} m\n\
             body      {length:.2} x {width:.2} m\n",
            speed = motion.speed_mps,
            distance = motion.path_distance_m,
            length = motion.body_length_m,
            width = motion.body_width_m,
        ));
        out.push_str("intent    hold constant speed along the guide path\n");
    }

    out.push_str("decision  none yet (Increment 0 has no decisions)");
    out
}

/// World-space bounds of every path vertex and portal, padded by portal width.
fn scenario_bounds(scenario: &CompiledScenario) -> Option<(Vec2, Vec2)> {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    let mut any = false;

    let mut include = |x: f64, y: f64, pad: f64| {
        let point = Vec2::new(x as f32, y as f32);
        let pad = Vec2::splat(pad as f32);
        min = min.min(point - pad);
        max = max.max(point + pad);
        any = true;
    };

    for path in scenario.paths() {
        for point in path.points() {
            include(point.x, point.y, 0.0);
        }
    }
    for portal in scenario.portals() {
        include(
            portal.position().x,
            portal.position().y,
            portal.width_m() * 0.5,
        );
    }

    any.then_some((min, max))
}

/// Position and heading to render for `sample`, blended from its previous
/// state by `alpha`.
fn interpolate(previous: &[AgentSample], sample: &AgentSample, alpha: f64) -> (glam::DVec2, f64) {
    if alpha <= 0.0 {
        return (sample.position, sample.heading_rad);
    }
    match previous.iter().find(|prior| prior.id == sample.id) {
        Some(prior) => (
            prior.position.lerp(sample.position, alpha),
            lerp_angle(prior.heading_rad, sample.heading_rad, alpha),
        ),
        None => (sample.position, sample.heading_rad),
    }
}

/// Interpolate headings along the shortest arc so wraparound does not spin.
fn lerp_angle(from: f64, to: f64, alpha: f64) -> f64 {
    let mut delta = (to - from) % std::f64::consts::TAU;
    if delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    } else if delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    from + delta * alpha
}
