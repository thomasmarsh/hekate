//! Safety-event projection: markers, body emphasis, and region occupancy.
//!
//! The kernel's safety pass owns the authoritative open states. This module
//! folds the same edge records the kernel emits into exactly the data a
//! renderer draws: a marker window that outlives one tick, the regions bodies
//! currently occupy, and the bodies a backend should emphasize. Every
//! derivation here is a pure function of the [`SceneFrame`] a backend was
//! handed — its records, its bodies, and its geometry — so two backends cannot
//! disagree, no renderer reads simulation internals, and replaying the same
//! event stream always draws the same frame.

use std::collections::BTreeMap;

use glam::DVec2;
use tangle_sim::{ControlTransitionKind, Event, EventKind, RegionKey};

use crate::scene::SceneFrame;

/// How long a safety record's marker stays visible, in simulated seconds.
///
/// The presentation layer converts this to whole ticks with the run's step, so
/// a marker persists for the same simulated time at any frame rate, which is
/// what "long enough to inspect" means for a run that plays faster than
/// wall-clock.
pub const MARKER_LIFETIME_SECONDS: f64 = 2.0;

/// Whether `kind` is a safety record a renderer marks.
///
/// `Spawned` and `Despawned` delimit an agent's stream and are counted as
/// population by the host; every other record names a body, a pair of bodies,
/// or a region and draws a marker.
pub const fn is_safety_record(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Yielded
            | EventKind::Collision
            | EventKind::NearMiss
            | EventKind::Violation
            | EventKind::Entry
            | EventKind::Exit
            | EventKind::Queue
            | EventKind::ControlTransition
    )
}

/// One safety record a frame carries, with the tick that emitted it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameEvent {
    tick: u64,
    event: Event,
}

impl FrameEvent {
    /// A record emitted by the tick `tick`.
    pub const fn new(tick: u64, event: Event) -> Self {
        Self { tick, event }
    }

    /// Completed kernel tick that emitted the record.
    pub const fn tick(self) -> u64 {
        self.tick
    }

    /// The record itself.
    pub const fn event(self) -> Event {
        self.event
    }

    /// Which record this is, without its payload.
    pub const fn kind(self) -> EventKind {
        self.event.kind()
    }
}

/// The bodies and region one safety record connects.
///
/// This is the inspector's link from a record to the geometry it involves:
/// agent ids ascend, and a record that names authored geometry carries its
/// [`RegionKey`]. The `Crossing` variant is also the crossing key of a yield,
/// so a consumer reads one spelling for every region-bearing record. A record
/// never names the same body twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventParticipants {
    agents: Vec<usize>,
    region: Option<RegionKey>,
}

impl EventParticipants {
    /// The participants of `event`.
    pub fn of(event: Event) -> Self {
        let agents = match event {
            Event::Collision { agent, other, .. } | Event::NearMiss { agent, other, .. } => {
                // The canonical pair spelling already ascends.
                vec![agent.get() as usize, other.get() as usize]
            }
            Event::Spawned { agent, .. }
            | Event::Despawned { agent, .. }
            | Event::Yielded { agent, .. }
            | Event::Violation { agent, .. }
            | Event::Entry { agent, .. }
            | Event::Exit { agent, .. }
            | Event::Queue { agent, .. }
            | Event::ControlTransition { agent, .. }
            | Event::Maneuver { agent, .. }
            | Event::FacilityTransition { agent, .. }
            | Event::OpposingTraversal { agent, .. } => vec![agent.get() as usize],
        };
        let region = match event {
            Event::Entry { region, .. } | Event::Exit { region, .. } => Some(region),
            Event::Yielded { crossing, .. } => Some(RegionKey::Crossing(crossing)),
            _ => None,
        };
        Self { agents, region }
    }

    /// Involved agent ids, ascending.
    pub fn agents(&self) -> &[usize] {
        &self.agents
    }

    /// The region the record names, if it names one.
    pub const fn region(&self) -> Option<RegionKey> {
        self.region
    }

    /// Involved agent ids other than `agent`, ascending: the bodies an
    /// inspector jumps to when it reads a record from `agent`.
    pub fn others(&self, agent: usize) -> Vec<usize> {
        self.agents
            .iter()
            .copied()
            .filter(|id| *id != agent)
            .collect()
    }
}

/// Why a backend emphasizes one body in the frame.
///
/// The order is the precedence: a body that is in contact and also queued
/// reports `Collision`, so a renderer draws one style and draws the more
/// urgent one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyEmphasis {
    /// The body is in contact with another body.
    Collision,
    /// The body is inside the near-miss band of another body.
    NearMiss,
    /// The body performed a breach inside the marker window.
    Violation,
    /// The body is at a standstill.
    Queue,
    /// The body's recorded controller state is active: holding at a
    /// signal-controlled stop line or waiting at a signal-controlled crossing.
    ControlTransition,
}

impl BodyEmphasis {
    /// Short stable label for a legend or a test.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Collision => "collision",
            Self::NearMiss => "near_miss",
            Self::Violation => "violation",
            Self::Queue => "queue",
            Self::ControlTransition => "control_transition",
        }
    }
}

/// A region bodies currently occupy.
///
/// An occupancy opens on [`Event::Entry`] and closes on [`Event::Exit`], or on
/// the occupying agent's despawn, which the safety pass closes without an exit
/// record. Occupants ascend by agent id and regions ascend by [`RegionKey`],
/// whose variant tag keeps the crossing and conflict-region id spaces apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccupiedRegion {
    region: RegionKey,
    occupants: Vec<usize>,
}

impl OccupiedRegion {
    /// The occupied region.
    pub const fn region(&self) -> RegionKey {
        self.region
    }

    /// Occupying agent ids, ascending.
    pub fn occupants(&self) -> &[usize] {
        &self.occupants
    }
}

/// One-line inspector text for a safety record.
///
/// The presentation layer owns this wording so every backend names the same
/// record the same way: the Bevy inspector and the terminal footer both read
/// it, and neither restates a kernel payload. The line names the bodies with
/// their `#` ids and the region when the record has one, so a reader can jump
/// to a body the way [`EventParticipants`] spells it.
pub fn event_summary(record: FrameEvent) -> String {
    let event = record.event();
    let ticket = format!("t{}", record.tick());
    let body = |agent: usize| format!("#{agent}");
    match event {
        Event::Spawned {
            agent,
            path,
            distance_m,
            ..
        } => format!(
            "{ticket} spawned  {}  path {}  at {distance_m:.2} m",
            body(agent.get() as usize),
            path.get()
        ),
        Event::Despawned { agent, path, .. } => format!(
            "{ticket} despawned  {}  path {}",
            body(agent.get() as usize),
            path.get()
        ),
        Event::Yielded {
            agent,
            crossing,
            yielding,
        } => format!(
            "{ticket} yield {}  {}  crossing {}",
            if yielding { "began" } else { "ended" },
            body(agent.get() as usize),
            crossing.get()
        ),
        Event::Collision {
            agent,
            other,
            clearance_m,
            contacting,
        } => format!(
            "{ticket} contact {}  {} + {}  clearance {clearance_m:.2} m",
            if contacting { "began" } else { "ended" },
            body(agent.get() as usize),
            body(other.get() as usize)
        ),
        Event::NearMiss {
            agent,
            other,
            clearance_m,
            entering,
        } => format!(
            "{ticket} near miss {}  {} + {}  clearance {clearance_m:.2} m",
            if entering { "began" } else { "ended" },
            body(agent.get() as usize),
            body(other.get() as usize)
        ),
        Event::Violation { agent, kind } => format!(
            "{ticket} violation  {}  {}",
            body(agent.get() as usize),
            kind.label()
        ),
        Event::Entry { agent, region } => format!(
            "{ticket} entry  {}  {}",
            body(agent.get() as usize),
            region_label(region)
        ),
        Event::Exit { agent, region } => format!(
            "{ticket} exit  {}  {}",
            body(agent.get() as usize),
            region_label(region)
        ),
        Event::Queue { agent, joined } => format!(
            "{ticket} queue {}  {}",
            if joined { "joined" } else { "left" },
            body(agent.get() as usize)
        ),
        Event::ControlTransition {
            agent,
            control,
            active,
        } => format!(
            "{ticket} control  {}  {} {}",
            body(agent.get() as usize),
            control.label(),
            if active { "active" } else { "ended" }
        ),
        Event::Maneuver {
            agent,
            kind,
            from,
            to,
            edge,
            reason,
            ..
        } => format!(
            "{ticket} maneuver  {}  {}  {} -> {}  {}  reason {}",
            body(agent.get() as usize),
            kind.label(),
            from.label(),
            to.label(),
            edge.label(),
            reason.label()
        ),
        Event::FacilityTransition {
            agent,
            from_facility,
            to_facility,
            via,
            permitted,
            ..
        } => format!(
            "{ticket} transition  {}  facility {} -> {}  {}  permitted {permitted}",
            body(agent.get() as usize),
            from_facility.get(),
            to_facility.get(),
            via.label()
        ),
        Event::OpposingTraversal {
            agent,
            facility,
            reason,
            violating,
            entering,
            ..
        } => format!(
            "{ticket} opposing {}  {}  facility {}  reason {}  violating {violating}",
            if entering { "began" } else { "ended" },
            body(agent.get() as usize),
            facility.get(),
            reason.label()
        ),
    }
}

/// Name of one region key, keeping the two id spaces apart.
fn region_label(region: RegionKey) -> String {
    match region {
        RegionKey::Crossing(crossing) => format!("crossing {}", crossing.get()),
        RegionKey::ConflictRegion(id) => format!("conflict region {}", id.get()),
    }
}

/// A marker a backend draws for one safety record.
///
/// The position is the record's anchor in world metres: the ring centre of a
/// region the record names, otherwise the midpoint of the participants that
/// are alive in the frame. A record whose bodies have all left the world, and
/// whose record names no region, draws no marker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafetyMarker {
    record: FrameEvent,
    position: DVec2,
}

impl SafetyMarker {
    /// The record this marker is for.
    pub const fn record(self) -> FrameEvent {
        self.record
    }

    /// Which record this marker is, without its payload.
    pub const fn kind(self) -> EventKind {
        self.record.kind()
    }

    /// Tick that emitted the record.
    pub const fn tick(self) -> u64 {
        self.record.tick()
    }

    /// World position of the marker in metres.
    pub const fn position(self) -> DVec2 {
        self.position
    }

    /// The bodies and region this marker links, so a renderer that lets a
    /// pointer pick a marker can select the bodies it involves.
    pub fn participants(self) -> EventParticipants {
        EventParticipants::of(self.record.event())
    }
}

/// The safety data one projected frame carries.
///
/// The record window is a function of simulated time: a record is retained for
/// [`MARKER_LIFETIME_SECONDS`] converted to whole ticks at the run's step, so a
/// marker keeps its place for the same simulated time however fast the run
/// plays. Occupancy, standstill, and controller states are open from their
/// opening edge until their closing edge arrives, so they persist independently
/// of the window. The fold is deterministic: it reads only the ordered records
/// it is given, keeps no time or randomness, and orders every collection
/// ascending so two runs of the same stream project byte-identical frames.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SafetyOverlay {
    events: Vec<FrameEvent>,
    marker_lifetime_ticks: u64,
    occupied: Vec<OccupiedRegion>,
    queued: Vec<usize>,
    controlling: Vec<(usize, ControlTransitionKind)>,
}

impl SafetyOverlay {
    /// An empty window that retains records for `marker_lifetime_ticks` ticks.
    pub fn new(marker_lifetime_ticks: u64) -> Self {
        Self {
            events: Vec::new(),
            marker_lifetime_ticks,
            occupied: Vec::new(),
            queued: Vec::new(),
            controlling: Vec::new(),
        }
    }

    /// Ticks a record stays in the window.
    pub const fn marker_lifetime_ticks(&self) -> u64 {
        self.marker_lifetime_ticks
    }

    /// Fold one tick's records into the window and the open states.
    ///
    /// Call once per completed kernel step with the tick that step produced.
    /// A record older than the window at `tick` is dropped, and the fold is
    /// edge-triggered, so calling this twice with the same tick would count the
    /// same edge twice.
    pub fn observe(&mut self, tick: u64, events: &[Event]) {
        for event in events {
            if is_safety_record(event.kind()) {
                self.events.push(FrameEvent::new(tick, *event));
            }
            match *event {
                Event::Entry { agent, region } => {
                    self.open_occupancy(region, agent.get() as usize);
                }
                Event::Exit { agent, region } => {
                    self.close_occupancy(region, agent.get() as usize);
                }
                Event::Despawned { agent, .. } => {
                    // The safety pass closes a departing agent's region
                    // occupancy without an exit record, so the despawn itself
                    // clears every state the agent held.
                    self.forget_agent(agent.get() as usize);
                }
                Event::Queue { agent, joined } => {
                    let id = agent.get() as usize;
                    if joined {
                        insert_ascending(&mut self.queued, id);
                    } else {
                        self.queued.retain(|queued| *queued != id);
                    }
                }
                Event::ControlTransition {
                    agent,
                    control,
                    active,
                } => {
                    let id = agent.get() as usize;
                    self.controlling.retain(|(held, _)| *held != id);
                    if active {
                        insert_ordered(&mut self.controlling, (id, control), |(a, _), (b, _)| {
                            a.cmp(b)
                        });
                    }
                }
                Event::Spawned { .. }
                | Event::Yielded { .. }
                | Event::Collision { .. }
                | Event::NearMiss { .. }
                | Event::Violation { .. }
                | Event::Maneuver { .. }
                | Event::FacilityTransition { .. }
                | Event::OpposingTraversal { .. } => {}
            }
        }
        self.prune(tick);
    }

    /// The recorded window, oldest first.
    pub fn events(&self) -> &[FrameEvent] {
        &self.events
    }

    /// Regions bodies currently occupy, ascending by region key.
    pub fn occupied_regions(&self) -> &[OccupiedRegion] {
        &self.occupied
    }

    /// Bodies whose last queue edge joined, ascending.
    pub fn queued(&self) -> &[usize] {
        &self.queued
    }

    /// Bodies whose last controller edge is active, ascending, with the state
    /// that is active.
    pub fn controlling(&self) -> &[(usize, ControlTransitionKind)] {
        &self.controlling
    }

    /// Whether the frame carries nothing to overlay.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
            && self.occupied.is_empty()
            && self.queued.is_empty()
            && self.controlling.is_empty()
    }

    /// The same folded states with the record window filtered to `tick`, so a
    /// frame carries exactly the records inside its own marker window even when
    /// the host has not observed a newer tick.
    pub fn windowed_at(&self, tick: u64) -> Self {
        let mut windowed = self.clone();
        windowed.prune(tick);
        windowed
    }

    /// Keep exactly the records of the last `lifetime` ticks ending at `tick`.
    fn prune(&mut self, tick: u64) {
        let lifetime = self.marker_lifetime_ticks;
        self.events
            .retain(|record| record.tick <= tick && record.tick.saturating_add(lifetime) > tick);
    }

    /// Open `region` for `agent`, keeping occupants ascending.
    fn open_occupancy(&mut self, region: RegionKey, agent: usize) {
        let index = match self
            .occupied
            .binary_search_by_key(&region, |occupied| occupied.region)
        {
            Ok(index) => index,
            Err(index) => {
                self.occupied.insert(
                    index,
                    OccupiedRegion {
                        region,
                        occupants: Vec::new(),
                    },
                );
                index
            }
        };
        insert_ascending(&mut self.occupied[index].occupants, agent);
    }

    /// Close `region` for `agent`, dropping the region once nobody is in it.
    fn close_occupancy(&mut self, region: RegionKey, agent: usize) {
        let Ok(index) = self
            .occupied
            .binary_search_by_key(&region, |occupied| occupied.region)
        else {
            return;
        };
        self.occupied[index].occupants.retain(|id| *id != agent);
        if self.occupied[index].occupants.is_empty() {
            self.occupied.remove(index);
        }
    }

    /// Clear every state a departing agent held.
    fn forget_agent(&mut self, agent: usize) {
        self.queued.retain(|id| *id != agent);
        self.controlling.retain(|(id, _)| *id != agent);
        for occupied in &mut self.occupied {
            occupied.occupants.retain(|id| *id != agent);
        }
        self.occupied
            .retain(|occupied| !occupied.occupants.is_empty());
    }
}

/// Insert `value` into an ascending, duplicate-free list when it is absent.
fn insert_ascending(values: &mut Vec<usize>, value: usize) {
    insert_ordered(values, value, |a, b| a.cmp(b));
}

/// Insert `value` into a list ordered by `compare` when it is absent.
fn insert_ordered<T>(
    values: &mut Vec<T>,
    value: T,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) {
    if let Err(index) = values.binary_search_by(|probe| compare(probe, &value)) {
        values.insert(index, value);
    }
}

impl SceneFrame {
    /// Markers for the safety records inside the marker window, oldest first.
    ///
    /// A marker for a pair record sits at the midpoint of the participants that
    /// are alive in this frame, so a marker that outlives one of its bodies
    /// keeps pointing at the survivor. A region record sits at its region's
    /// ring centre, which is what an occupancy overlay highlights.
    pub fn safety_markers(&self) -> Vec<SafetyMarker> {
        self.safety
            .events()
            .iter()
            .filter_map(|record| {
                marker_anchor(self, *record).map(|position| SafetyMarker {
                    record: *record,
                    position,
                })
            })
            .collect()
    }

    /// The emphasis each emphasized body carries, ascending by agent id.
    ///
    /// At most one emphasis per body: the highest-precedence one, so a body in
    /// contact is not also reported as queued. Contact, near-miss, and
    /// violation emphases come from the records inside the marker window, so
    /// they fade with the marker; queue and control-transition emphases are the
    /// open states, so they last exactly as long as the state does.
    pub fn body_emphasis(&self) -> Vec<(usize, BodyEmphasis)> {
        let mut emphasis: BTreeMap<usize, BodyEmphasis> = BTreeMap::new();
        for record in self.safety.events() {
            let candidate = match record.kind() {
                EventKind::Collision => Some(BodyEmphasis::Collision),
                EventKind::NearMiss => Some(BodyEmphasis::NearMiss),
                EventKind::Violation => Some(BodyEmphasis::Violation),
                _ => None,
            };
            let Some(candidate) = candidate else {
                continue;
            };
            for agent in EventParticipants::of(record.event()).agents() {
                emphasis
                    .entry(*agent)
                    .and_modify(|held| *held = (*held).min(candidate))
                    .or_insert(candidate);
            }
        }
        for agent in self.safety.queued() {
            emphasis.entry(*agent).or_insert(BodyEmphasis::Queue);
        }
        for (agent, _) in self.safety.controlling() {
            emphasis
                .entry(*agent)
                .or_insert(BodyEmphasis::ControlTransition);
        }
        emphasis.into_iter().collect()
    }

    /// Regions bodies currently occupy, ascending by region key.
    pub fn occupied_regions(&self) -> &[OccupiedRegion] {
        self.safety.occupied_regions()
    }

    /// Records inside the marker window that involve `agent`, oldest first.
    ///
    /// This is the inspector's answer to "what is happening to this body": each
    /// record's [`EventParticipants`] names the bodies and region to highlight
    /// or jump to.
    pub fn events_involving(&self, agent: usize) -> Vec<FrameEvent> {
        self.safety
            .events()
            .iter()
            .copied()
            .filter(|record| {
                EventParticipants::of(record.event())
                    .agents()
                    .contains(&agent)
            })
            .collect()
    }

    /// The region ring of `region`, when the frame's geometry has it.
    pub fn region_points(&self, region: RegionKey) -> Option<&[DVec2]> {
        self.geometry.region_points(region)
    }
}

/// World anchor of the marker for `record`, or `None` when it has none.
fn marker_anchor(frame: &SceneFrame, record: FrameEvent) -> Option<DVec2> {
    let participants = EventParticipants::of(record.event());
    if let Some(region) = participants.region()
        && let Some(center) = frame.geometry.region_center(region)
    {
        return Some(center);
    }
    let alive: Vec<DVec2> = participants
        .agents()
        .iter()
        .filter_map(|agent| frame.body(*agent).map(|body| body.position))
        .collect();
    if alive.is_empty() {
        return None;
    }
    let count = alive.len() as f64;
    Some(
        alive
            .into_iter()
            .fold(DVec2::ZERO, |sum, point| sum + point)
            / count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tangle_model::{
        CompiledScenario, ConflictRegionId, CrossingId, PathId, parse_scenario_source,
    };
    use tangle_sim::{AgentId, AgentMode, DespawnReason, ViolationKind};

    use crate::scene::{FrameStatus, Overlays, SceneBody, SceneGeometry, Viewport};

    const LIFETIME_TICKS: u64 = 4;

    fn signalized() -> CompiledScenario {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'cross', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'ew', points: [ { x: -20, y: 0 }, { x: 20, y: 0 } ] }, \
             { id: 'ns', points: [ { x: 0, y: -20 }, { x: 0, y: 20 } ] } ], \
             portals: [ { id: 'west', path: 'ew', end: 'start', width_m: 3.5 }, \
             { id: 'east', path: 'ew', end: 'end', width_m: 3.5 }, \
             { id: 'south', path: 'ns', end: 'start', width_m: 3.5 }, \
             { id: 'north', path: 'ns', end: 'end', width_m: 3.5 } ], \
             regions: [ { id: 'area', points: [ { x: -2, y: -4 }, { x: 2, y: -4 }, \
             { x: 2, y: 4 }, { x: -2, y: 4 } ] } ], \
             movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'ew', priority: 0 }, \
             { id: 'ns_through', from: 'south', to: 'north', path: 'ns', priority: 1 } ], \
             crossings: [ { id: 'cross', region: 'area', movements: [ 'ew_through' ] } ], \
             conflict_regions: [ { id: 'center', points: [ { x: -1, y: -1 }, { x: 1, y: -1 }, \
             { x: 1, y: 1 }, { x: -1, y: 1 } ], movements: [ 'ew_through', 'ns_through' ] } ], \
             population: { vehicle_count: 2, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    fn body(id: usize, position: DVec2, mode: AgentMode) -> SceneBody {
        SceneBody {
            id,
            position,
            heading_rad: 0.0,
            length_m: 1.0,
            width_m: 1.0,
            mode,
            body_kind: mode.body_kind(),
            segments: Vec::new(),
            speed_mps: Some(0.0),
            path: None,
            path_distance_m: None,
            route: None,
            profile: None,
            decision: None,
        }
    }

    /// A frame carrying `events` at tick `tick`, with a geometry that has both
    /// region id spaces.
    fn projected_frame(tick: u64, bodies: Vec<SceneBody>, events: Vec<Event>) -> SceneFrame {
        let scenario = signalized();
        let mut safety = SafetyOverlay::new(LIFETIME_TICKS);
        safety.observe(tick, &events);
        SceneFrame {
            scenario_id: scenario.id().to_owned(),
            time_seconds: tick as f64 * 0.05,
            tick,
            status: FrameStatus {
                agents: bodies.len(),
                speed: crate::clock::Speed::Real,
                paused: false,
                selection: None,
            },
            viewport: Viewport::new(DVec2::ZERO, 1.0),
            geometry: Arc::new(SceneGeometry::from_scenario(&scenario)),
            bodies,
            overlays: Overlays::default(),
            safety,
        }
    }

    fn pair(kind: EventKind) -> Event {
        let agent = AgentId::from_index(0);
        let other = AgentId::from_index(1);
        match kind {
            EventKind::Collision => Event::Collision {
                agent,
                other,
                clearance_m: -0.5,
                contacting: true,
            },
            EventKind::NearMiss => Event::NearMiss {
                agent,
                other,
                clearance_m: 0.7,
                entering: true,
            },
            _ => panic!("not a pair record: {kind:?}"),
        }
    }

    #[test]
    fn participants_name_the_bodies_and_region_of_every_record() {
        let agent = AgentId::from_index(3);
        let other = AgentId::from_index(5);
        let crossing = CrossingId::from_index(1);
        let region = RegionKey::Crossing(crossing);
        let conflict = RegionKey::ConflictRegion(tangle_model::ConflictRegionId::from_index(0));
        let cases = [
            (
                Event::Spawned {
                    agent,
                    mode: AgentMode::Pedestrian,
                    path: PathId::from_index(0),
                    distance_m: 1.5,
                },
                vec![3],
                None,
            ),
            (
                Event::Despawned {
                    agent,
                    path: PathId::from_index(0),
                    reason: DespawnReason::ExitedPath,
                },
                vec![3],
                None,
            ),
            (
                Event::Yielded {
                    agent,
                    crossing,
                    yielding: true,
                },
                vec![3],
                Some(region),
            ),
            (
                Event::Collision {
                    agent,
                    other,
                    clearance_m: 0.0,
                    contacting: true,
                },
                vec![3, 5],
                None,
            ),
            (
                Event::NearMiss {
                    agent,
                    other,
                    clearance_m: 0.5,
                    entering: false,
                },
                vec![3, 5],
                None,
            ),
            (
                Event::Violation {
                    agent,
                    kind: ViolationKind::RanRedLight,
                },
                vec![3],
                None,
            ),
            (Event::Entry { agent, region }, vec![3], Some(region)),
            (Event::Exit { agent, region }, vec![3], Some(region)),
            (
                Event::Entry {
                    agent,
                    region: conflict,
                },
                vec![3],
                Some(conflict),
            ),
            (
                Event::Queue {
                    agent,
                    joined: true,
                },
                vec![3],
                None,
            ),
            (
                Event::ControlTransition {
                    agent,
                    control: ControlTransitionKind::SignalStop,
                    active: true,
                },
                vec![3],
                None,
            ),
        ];
        for (event, agents, region) in cases {
            let participants = EventParticipants::of(event);
            assert_eq!(participants.agents(), agents.as_slice(), "{event:?}");
            assert_eq!(participants.region(), region, "{event:?}");
        }
        // The pair spelling ascends even when the arguments are reversed.
        let reversed = Event::Collision {
            agent: other,
            other: agent,
            clearance_m: 0.0,
            contacting: false,
        };
        assert_eq!(EventParticipants::of(reversed).agents(), &[5, 3]);
    }

    #[test]
    fn others_are_the_jump_targets_of_one_record() {
        let participants = EventParticipants::of(pair(EventKind::Collision));
        assert_eq!(participants.others(0), vec![1]);
        assert_eq!(participants.others(1), vec![0]);
        // An agent outside the record has every participant as a jump target.
        assert_eq!(participants.others(7), vec![0, 1]);
    }

    #[test]
    fn the_fold_opens_and_closes_occupancy_queue_and_control_states() {
        let crossing = RegionKey::Crossing(CrossingId::from_index(0));
        let conflict = RegionKey::ConflictRegion(tangle_model::ConflictRegionId::from_index(0));
        let agent = AgentId::from_index(0);
        let other = AgentId::from_index(1);
        let mut safety = SafetyOverlay::new(LIFETIME_TICKS);
        safety.observe(
            10,
            &[
                Event::Entry {
                    agent: other,
                    region: crossing,
                },
                Event::Entry {
                    agent,
                    region: crossing,
                },
                Event::Entry {
                    agent,
                    region: conflict,
                },
                Event::Queue {
                    agent,
                    joined: true,
                },
                Event::ControlTransition {
                    agent,
                    control: ControlTransitionKind::SignalStop,
                    active: true,
                },
            ],
        );
        // Occupants ascend, and regions keep their own id spaces apart.
        assert_eq!(
            safety.occupied_regions(),
            &[
                OccupiedRegion {
                    region: crossing,
                    occupants: vec![0, 1],
                },
                OccupiedRegion {
                    region: conflict,
                    occupants: vec![0],
                },
            ]
        );
        assert_eq!(safety.queued(), &[0]);
        assert_eq!(
            safety.controlling(),
            &[(0, ControlTransitionKind::SignalStop)]
        );
        assert_eq!(safety.events().len(), 5);

        // A despawn clears every state without a closing record.
        safety.observe(
            11,
            &[Event::Despawned {
                agent,
                path: tangle_model::PathId::from_index(0),
                reason: DespawnReason::ExitedPath,
            }],
        );
        assert_eq!(
            safety.occupied_regions(),
            &[OccupiedRegion {
                region: crossing,
                occupants: vec![1],
            }]
        );
        assert!(safety.queued().is_empty());
        assert!(safety.controlling().is_empty());

        // The closing edges empty the rest.
        safety.observe(
            12,
            &[
                Event::Exit {
                    agent: other,
                    region: crossing,
                },
                Event::Queue {
                    agent,
                    joined: false,
                },
            ],
        );
        assert!(safety.occupied_regions().is_empty());
        assert!(safety.queued().is_empty());
        // The despawn and the ordinary records are not safety markers.
        assert_eq!(safety.events().len(), 7);
        assert!(
            safety
                .events()
                .iter()
                .all(|record| is_safety_record(record.kind()))
        );
    }

    #[test]
    fn the_window_drops_records_older_than_the_lifetime() {
        let agent = AgentId::from_index(0);
        let mut safety = SafetyOverlay::new(LIFETIME_TICKS);
        safety.observe(
            10,
            &[Event::Queue {
                agent,
                joined: true,
            }],
        );
        safety.observe(
            13,
            &[Event::Queue {
                agent,
                joined: false,
            }],
        );
        assert_eq!(safety.events().len(), 2);
        // Ticks 13 - 10 = 3 < 4, so both are still inside the window.
        safety.observe(
            14,
            &[Event::Queue {
                agent,
                joined: true,
            }],
        );
        assert_eq!(safety.events().len(), 2);
        assert_eq!(safety.events()[0].tick(), 13);
        // Filtering without observing a newer tick keeps the frame honest: the
        // window at tick 13 holds only the record of that tick.
        assert_eq!(safety.windowed_at(13).events().len(), 1);
        assert_eq!(safety.windowed_at(13).events()[0].tick(), 13);
        assert!(safety.windowed_at(20).events().is_empty());
    }

    #[test]
    fn markers_anchor_on_region_centres_and_live_participants() {
        let crossing = RegionKey::Crossing(CrossingId::from_index(0));
        let events = vec![
            pair(EventKind::Collision),
            Event::Entry {
                agent: AgentId::from_index(0),
                region: crossing,
            },
            Event::Queue {
                agent: AgentId::from_index(1),
                joined: true,
            },
            Event::Violation {
                agent: AgentId::from_index(9),
                kind: ViolationKind::RanRedLight,
            },
        ];
        let bodies = vec![
            body(0, DVec2::new(4.0, 0.0), AgentMode::Vehicle),
            body(1, DVec2::new(10.0, 0.0), AgentMode::Pedestrian),
        ];
        let frame = projected_frame(20, bodies, events);
        let markers = frame.safety_markers();
        // The departed agent #9 draws no marker, so the other three record a
        // position in record order.
        assert_eq!(
            markers
                .iter()
                .map(|marker| marker.kind())
                .collect::<Vec<_>>(),
            vec![EventKind::Collision, EventKind::Entry, EventKind::Queue]
        );
        // The pair marker is the midpoint of its two live bodies, not either
        // body's own position.
        assert_eq!(markers[0].position(), DVec2::new(7.0, 0.0));
        // The crossing ring spans x -2..2 and y -4..4, so its centre is origin
        // rather than the entering body's position at (4, 0).
        assert_eq!(markers[1].position(), DVec2::ZERO);
        assert_ne!(markers[1].position(), DVec2::new(4.0, 0.0));
        assert_eq!(markers[2].position(), DVec2::new(10.0, 0.0));
        assert_eq!(
            markers[0].participants().agents(),
            &[0, 1],
            "a marker links its bodies"
        );
        assert_eq!(markers[1].participants().region(), Some(crossing));

        // Only one participant alive still anchors the marker on the survivor.
        let frame = projected_frame(
            20,
            vec![body(1, DVec2::new(10.0, 0.0), AgentMode::Pedestrian)],
            vec![pair(EventKind::NearMiss)],
        );
        assert_eq!(frame.safety_markers()[0].position(), DVec2::new(10.0, 0.0));
    }

    #[test]
    fn emphasis_reports_one_precedence_ordered_style_per_body() {
        let events = vec![
            Event::NearMiss {
                agent: AgentId::from_index(0),
                other: AgentId::from_index(3),
                clearance_m: 0.5,
                entering: true,
            },
            Event::Queue {
                agent: AgentId::from_index(0),
                joined: true,
            },
            Event::ControlTransition {
                agent: AgentId::from_index(1),
                control: ControlTransitionKind::CrossingWait,
                active: true,
            },
            Event::Violation {
                agent: AgentId::from_index(2),
                kind: ViolationKind::CrossedAgainstSignal,
            },
            Event::Queue {
                agent: AgentId::from_index(4),
                joined: true,
            },
        ];
        let bodies = (0..5)
            .map(|id| body(id, DVec2::ZERO, AgentMode::Vehicle))
            .collect();
        let frame = projected_frame(20, bodies, events);
        // Agent 0 is in the band and queued, so the band wins; both bodies of
        // the pair carry it; the open queue and controller states stand on
        // their own.
        assert_eq!(
            frame.body_emphasis(),
            vec![
                (0, BodyEmphasis::NearMiss),
                (1, BodyEmphasis::ControlTransition),
                (2, BodyEmphasis::Violation),
                (3, BodyEmphasis::NearMiss),
                (4, BodyEmphasis::Queue),
            ]
        );
        assert_eq!(BodyEmphasis::Collision.label(), "collision");
        assert_eq!(BodyEmphasis::NearMiss.label(), "near_miss");
        assert_eq!(BodyEmphasis::Queue.label(), "queue");
        assert_eq!(
            BodyEmphasis::ControlTransition.label(),
            "control_transition"
        );

        // A body that leaves the queue stops being emphasized once no state or
        // window record names it.
        let frame = projected_frame(
            20,
            vec![body(4, DVec2::ZERO, AgentMode::Vehicle)],
            vec![Event::Queue {
                agent: AgentId::from_index(4),
                joined: false,
            }],
        );
        assert!(frame.body_emphasis().is_empty());
    }

    #[test]
    fn event_summaries_name_every_record_for_an_inspector() {
        let agent = AgentId::from_index(0);
        let other = AgentId::from_index(7);
        let crossing = CrossingId::from_index(2);
        let region = RegionKey::Crossing(crossing);
        let cases = [
            (
                Event::Spawned {
                    agent,
                    mode: AgentMode::Vehicle,
                    path: PathId::from_index(1),
                    distance_m: 12.5,
                },
                "spawned  #0  path 1  at 12.50 m",
            ),
            (
                Event::Despawned {
                    agent,
                    path: PathId::from_index(1),
                    reason: DespawnReason::ExitedPath,
                },
                "despawned  #0  path 1",
            ),
            (
                Event::Yielded {
                    agent,
                    crossing,
                    yielding: true,
                },
                "yield began  #0  crossing 2",
            ),
            (
                Event::Yielded {
                    agent,
                    crossing,
                    yielding: false,
                },
                "yield ended  #0  crossing 2",
            ),
            (
                Event::Collision {
                    agent,
                    other,
                    clearance_m: -0.25,
                    contacting: true,
                },
                "contact began  #0 + #7  clearance -0.25 m",
            ),
            (
                Event::NearMiss {
                    agent,
                    other,
                    clearance_m: 0.5,
                    entering: false,
                },
                "near miss ended  #0 + #7  clearance 0.50 m",
            ),
            (
                Event::Violation {
                    agent,
                    kind: ViolationKind::CrossedAgainstSignal,
                },
                "violation  #0  crossed_against_signal",
            ),
            (Event::Entry { agent, region }, "entry  #0  crossing 2"),
            (Event::Exit { agent, region }, "exit  #0  crossing 2"),
            (
                Event::Entry {
                    agent,
                    region: RegionKey::ConflictRegion(ConflictRegionId::from_index(3)),
                },
                "entry  #0  conflict region 3",
            ),
            (
                Event::Queue {
                    agent,
                    joined: true,
                },
                "queue joined  #0",
            ),
            (
                Event::Queue {
                    agent,
                    joined: false,
                },
                "queue left  #0",
            ),
            (
                Event::ControlTransition {
                    agent,
                    control: ControlTransitionKind::SignalStop,
                    active: true,
                },
                "control  #0  signal_stop active",
            ),
            (
                Event::ControlTransition {
                    agent,
                    control: ControlTransitionKind::CrossingWait,
                    active: false,
                },
                "control  #0  crossing_wait ended",
            ),
        ];
        for (event, expected) in cases {
            let summary = event_summary(FrameEvent::new(120, event));
            assert_eq!(summary, format!("t120 {expected}"), "{event:?}");
        }
    }

    #[test]
    fn a_body_with_no_records_has_no_markers_emphasis_or_links() {
        let frame = projected_frame(
            3,
            vec![body(0, DVec2::ZERO, AgentMode::Vehicle)],
            Vec::new(),
        );
        assert!(frame.safety_markers().is_empty());
        assert!(frame.body_emphasis().is_empty());
        assert!(frame.occupied_regions().is_empty());
        assert!(frame.events_involving(0).is_empty());
        assert!(frame.safety.is_empty());
    }

    #[test]
    fn event_links_list_the_records_of_one_body_with_their_participants() {
        let events = vec![
            pair(EventKind::NearMiss),
            Event::Queue {
                agent: AgentId::from_index(0),
                joined: true,
            },
        ];
        let frame = projected_frame(
            20,
            vec![body(0, DVec2::ZERO, AgentMode::Vehicle)],
            events.clone(),
        );
        let links: Vec<EventParticipants> = frame
            .events_involving(0)
            .into_iter()
            .map(|record| EventParticipants::of(record.event()))
            .collect();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].others(0), vec![1]);
        assert_eq!(links[1].others(0), Vec::<usize>::new());
        // The record's other participant links back to the same record, and an
        // uninvolved agent has no link at all.
        assert_eq!(frame.events_involving(1).len(), 1);
        assert_eq!(
            EventParticipants::of(frame.events_involving(1)[0].event()).others(1),
            vec![0]
        );
        assert!(frame.events_involving(9).is_empty());
        // The projection is stable: the same records project the same frame
        // data.
        assert_eq!(frame.safety_markers(), frame.safety_markers());
        assert_eq!(frame.events_involving(0).len(), 2);
    }
}
