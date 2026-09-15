//! Route-relative tactical overlay projection: usable corridor, target offset,
//! predicted gap, maneuver state, and wrong-way rule state.
//!
//! An Increment 2 run carries route-relative tactical state the earlier
//! projections did not: the versioned [`RouteStateSample`] on each steering
//! body's motion sample, and the typed maneuver and opposing-traversal edge
//! records the kernel emits. This module turns both into exactly the primitives
//! a renderer draws, so every backend reads the same data and no backend
//! re-derives a tactic, a reason code, or an interval from simulation
//! internals.
//!
//! Two of the five primitives are sparse and edge-bounded, so they are folded
//! here from the records the host observed, exactly as
//! [`SafetyOverlay`](crate::safety::SafetyOverlay) folds region occupancy: a
//! maneuver interval opens on the maneuver edge that leaves
//! [`ManeuverState::Following`] and closes when an edge returns the agent to it,
//! and a wrong-way interval opens on its `entering: true` record and closes on
//! its `entering: false` record or on the traversing agent's despawn. The other
//! three are read per body from the versioned sample the frame already carries.
//! Every derivation is a pure function of one [`SceneFrame`] — its bodies, its
//! geometry, and its folded intervals — so replaying the same run always draws
//! the same overlays, and a frame with no route state and no interval draws
//! nothing.
//!
//! [`RouteStateSample`]: hekate_sim::RouteStateSample

use glam::DVec2;
use hekate_model::{
    FacilityId, MovementDirection, MovementId, NominalDirection, PermissionEffect, TacticKind,
};
use hekate_sim::{
    Event, ManeuverEdge, ManeuverReasonCode, ManeuverState, PassSide, WrongWayReason,
};

use crate::safety::{FrameEvent, insert_ordered};
use crate::scene::{SceneBody, SceneFacility, SceneFrame};

/// One agent's open maneuver interval.
///
/// It opens on the maneuver edge that leaves [`ManeuverState::Following`] and
/// lasts until an edge returns the agent to `Following` or the agent despawns,
/// which is exactly the interval the live `maneuver_state` sample is non-`Following`
/// over. `latest` is the most recent edge of the interval, so a maneuver that
/// moved `preparing -> committed` reports the committing edge while it lasts.
#[derive(Debug, Clone, PartialEq)]
pub struct ManeuverInterval {
    agent: usize,
    opened: FrameEvent,
    latest: FrameEvent,
}

impl ManeuverInterval {
    /// The maneuvering agent.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// The edge that opened the interval.
    pub const fn opened(&self) -> &FrameEvent {
        &self.opened
    }

    /// The most recent edge of the interval; equal to [`Self::opened`] until
    /// the maneuver moves again.
    pub const fn latest(&self) -> &FrameEvent {
        &self.latest
    }
}

/// One agent's open opposing-traversal interval.
///
/// It opens on the `entering: true` record and lasts until the `entering: false`
/// record or the traversing agent's despawn, so the wrong-way rule state it
/// names is present exactly while the traversal is.
#[derive(Debug, Clone, PartialEq)]
pub struct WrongWayInterval {
    agent: usize,
    opened: FrameEvent,
}

impl WrongWayInterval {
    /// The traversing agent.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// The `entering: true` record that opened the interval.
    pub const fn opened(&self) -> &FrameEvent {
        &self.opened
    }
}

/// The route-relative tactical intervals one projected frame carries.
///
/// The fold is deterministic: it reads only the ordered records it is given,
/// keeps no time or randomness, and orders its intervals ascending by agent id,
/// so two runs of the same stream project byte-identical overlays.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TacticalOverlay {
    maneuvers: Vec<ManeuverInterval>,
    wrong_way: Vec<WrongWayInterval>,
}

impl TacticalOverlay {
    /// Fold one tick's records into the open intervals.
    ///
    /// Call once per completed kernel step with the tick that step produced. The
    /// fold is edge-triggered, so calling it twice with the same tick would
    /// count the same edge twice.
    pub fn observe(&mut self, tick: u64, events: &[Event]) {
        for event in events {
            match *event {
                Event::Maneuver { agent, to, .. } => {
                    let id = agent.get() as usize;
                    if to == ManeuverState::Following {
                        self.maneuvers.retain(|interval| interval.agent != id);
                    } else {
                        let record = FrameEvent::new(tick, event.clone());
                        match self
                            .maneuvers
                            .iter_mut()
                            .find(|interval| interval.agent == id)
                        {
                            Some(interval) => interval.latest = record,
                            None => insert_ordered(
                                &mut self.maneuvers,
                                ManeuverInterval {
                                    agent: id,
                                    opened: record.clone(),
                                    latest: record,
                                },
                                |left, right| left.agent.cmp(&right.agent),
                            ),
                        }
                    }
                }
                Event::OpposingTraversal {
                    agent, entering, ..
                } => {
                    let id = agent.get() as usize;
                    self.wrong_way.retain(|interval| interval.agent != id);
                    if entering {
                        insert_ordered(
                            &mut self.wrong_way,
                            WrongWayInterval {
                                agent: id,
                                opened: FrameEvent::new(tick, event.clone()),
                            },
                            |left, right| left.agent.cmp(&right.agent),
                        );
                    }
                }
                Event::Despawned { agent, .. } => {
                    let id = agent.get() as usize;
                    self.maneuvers.retain(|interval| interval.agent != id);
                    self.wrong_way.retain(|interval| interval.agent != id);
                }
                Event::Spawned { .. }
                | Event::Yielded { .. }
                | Event::Collision { .. }
                | Event::NearMiss { .. }
                | Event::Violation { .. }
                | Event::Entry { .. }
                | Event::Exit { .. }
                | Event::Queue { .. }
                | Event::ControlTransition { .. }
                | Event::FacilityTransition { .. }
                | Event::ClosePass { .. } => {}
            }
        }
    }

    /// Open maneuver intervals, ascending by agent id.
    pub fn maneuvers(&self) -> &[ManeuverInterval] {
        &self.maneuvers
    }

    /// Open opposing-traversal intervals, ascending by agent id.
    pub fn wrong_way(&self) -> &[WrongWayInterval] {
        &self.wrong_way
    }

    /// Whether the frame carries no open interval.
    pub fn is_empty(&self) -> bool {
        self.maneuvers.is_empty() && self.wrong_way.is_empty()
    }
}

/// One body's usable corridor: the lateral interval it may occupy inside the
/// facility band it follows.
///
/// The interval is in the body's own travel frame, the same frame as the
/// sample's `d_m`, and `left` is the unit world direction of positive `d`. A
/// backend draws the corridor segment from `anchor + left * d_min_m` to
/// `anchor + left * d_max_m`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorridorOverlay {
    agent: usize,
    facility: FacilityId,
    anchor: DVec2,
    left: DVec2,
    along_m: f64,
    offset_m: f64,
    d_min_m: f64,
    d_max_m: f64,
    clearance_m: Option<f64>,
    horizon_s: Option<f64>,
}

impl CorridorOverlay {
    /// The body the corridor belongs to.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// The facility band the corridor is drawn inside.
    pub const fn facility(&self) -> FacilityId {
        self.facility
    }

    /// The body's world position in metres, the corridor's anchor.
    pub const fn anchor(&self) -> DVec2 {
        self.anchor
    }

    /// Unit world direction of positive offset (left of the body's heading).
    pub const fn left(&self) -> DVec2 {
        self.left
    }

    /// Arc length along the facility reference in metres.
    pub const fn along_m(&self) -> f64 {
        self.along_m
    }

    /// The body's signed lateral offset in metres, positive to the left.
    pub const fn offset_m(&self) -> f64 {
        self.offset_m
    }

    /// Lowest usable signed offset in metres.
    pub const fn d_min_m(&self) -> f64 {
        self.d_min_m
    }

    /// Highest usable signed offset in metres.
    pub const fn d_max_m(&self) -> f64 {
        self.d_max_m
    }

    /// The usable interval in metres, `(d_min, d_max)`.
    pub const fn interval_m(&self) -> (f64, f64) {
        (self.d_min_m, self.d_max_m)
    }

    /// The mode's resolved target clearance in metres, when its policy
    /// declares one.
    pub const fn clearance_m(&self) -> Option<f64> {
        self.clearance_m
    }

    /// The mode's resolved feasible horizon in seconds, when its policy
    /// declares one.
    pub const fn horizon_s(&self) -> Option<f64> {
        self.horizon_s
    }
}

/// One body's fixed target offset: the lateral offset a maneuver is displacing
/// toward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetOffsetOverlay {
    agent: usize,
    offset_m: f64,
    side: PassSide,
    from_m: f64,
    anchor: DVec2,
    left: DVec2,
    target: DVec2,
}

impl TargetOffsetOverlay {
    /// The maneuvering body.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// The target signed offset in metres, in the body's travel frame.
    pub const fn offset_m(&self) -> f64 {
        self.offset_m
    }

    /// The side of the offset itself: a non-negative offset is on the
    /// positive-`d`, left side.
    pub const fn side(&self) -> PassSide {
        self.side
    }

    /// The body's current offset in metres, the offset the target moves from.
    pub const fn from_m(&self) -> f64 {
        self.from_m
    }

    /// The body's world position in metres, the offset's anchor.
    pub const fn anchor(&self) -> DVec2 {
        self.anchor
    }

    /// Unit world direction of positive offset (left of the body's heading).
    pub const fn left(&self) -> DVec2 {
        self.left
    }

    /// World position of the target offset in metres.
    pub const fn target(&self) -> DVec2 {
        self.target
    }
}

/// One body's predicted minimum clearance over its maneuver horizon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedGapOverlay {
    agent: usize,
    predicted_min_clearance_m: f64,
    target_clearance_m: Option<f64>,
    horizon_s: Option<f64>,
    anchor: DVec2,
}

impl PredictedGapOverlay {
    /// The predicted body.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// Predicted minimum clearance over the horizon, in metres.
    pub const fn predicted_min_clearance_m(&self) -> f64 {
        self.predicted_min_clearance_m
    }

    /// The mode's resolved target clearance in metres, when its policy
    /// declares one.
    pub const fn target_clearance_m(&self) -> Option<f64> {
        self.target_clearance_m
    }

    /// The mode's resolved feasible horizon in seconds, when its policy
    /// declares one.
    pub const fn horizon_s(&self) -> Option<f64> {
        self.horizon_s
    }

    /// Predicted clearance less the target clearance, present when the sample
    /// resolves a target; a negative margin is a predicted shortfall.
    pub fn margin_m(&self) -> Option<f64> {
        self.target_clearance_m
            .map(|target| self.predicted_min_clearance_m - target)
    }

    /// The body's world position in metres, the gap's anchor.
    pub const fn anchor(&self) -> DVec2 {
        self.anchor
    }
}

/// One body's maneuver state, with the edge that put it there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManeuverOverlay {
    agent: usize,
    state: ManeuverState,
    kind: TacticKind,
    edge: ManeuverEdge,
    partner: Option<usize>,
    source_facility: FacilityId,
    target_facility: Option<FacilityId>,
    target_offset_m: f64,
    side: PassSide,
    reason: ManeuverReasonCode,
    opened_tick: u64,
}

impl ManeuverOverlay {
    /// The maneuvering body.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// The body's maneuver lifecycle state.
    pub const fn state(&self) -> ManeuverState {
        self.state
    }

    /// The tactic the maneuver belongs to.
    pub const fn kind(&self) -> TacticKind {
        self.kind
    }

    /// The documented edge the interval's most recent transition took.
    pub const fn edge(&self) -> ManeuverEdge {
        self.edge
    }

    /// The body the maneuver displaces around, absent for a maneuver that
    /// targets an offset rather than a body.
    pub const fn partner(&self) -> Option<usize> {
        self.partner
    }

    /// The facility traversal the maneuver started on.
    pub const fn source_facility(&self) -> FacilityId {
        self.source_facility
    }

    /// The facility the maneuver targets, absent for a same-facility maneuver.
    pub const fn target_facility(&self) -> Option<FacilityId> {
        self.target_facility
    }

    /// Target signed offset in metres, in the body's own travel frame.
    pub const fn target_offset_m(&self) -> f64 {
        self.target_offset_m
    }

    /// The side the displacement claims, in the body's own travel frame.
    pub const fn side(&self) -> PassSide {
        self.side
    }

    /// Why the most recent edge happened.
    pub const fn reason(&self) -> ManeuverReasonCode {
        self.reason
    }

    /// Tick that opened the maneuver interval.
    pub const fn opened_tick(&self) -> u64 {
        self.opened_tick
    }
}

/// One body's wrong-way rule state while its opposing traversal is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrongWayOverlay {
    agent: usize,
    facility: FacilityId,
    movement: Option<MovementId>,
    direction: MovementDirection,
    nominal_direction: NominalDirection,
    perceived_rule: Option<PermissionEffect>,
    reason: WrongWayReason,
    violating: bool,
    opened_tick: u64,
}

impl WrongWayOverlay {
    /// The traversing body.
    pub const fn agent(&self) -> usize {
        self.agent
    }

    /// The facility traversed against its rule direction.
    pub const fn facility(&self) -> FacilityId {
        self.facility
    }

    /// The movement connector it entered on, absent for a facility traversal.
    pub const fn movement(&self) -> Option<MovementId> {
        self.movement
    }

    /// The direction the body actually travels.
    pub const fn direction(&self) -> MovementDirection {
        self.direction
    }

    /// The facility's authored or compiled nominal direction.
    pub const fn nominal_direction(&self) -> NominalDirection {
        self.nominal_direction
    }

    /// The permission statement the body acted under, absent when none
    /// applies.
    pub const fn perceived_rule(&self) -> Option<PermissionEffect> {
        self.perceived_rule
    }

    /// Why the decision selected the opposing option.
    pub const fn reason(&self) -> WrongWayReason {
        self.reason
    }

    /// Whether the applicable rule does not permit the traversal.
    pub const fn violating(&self) -> bool {
        self.violating
    }

    /// Tick that opened the opposing-traversal interval.
    pub const fn opened_tick(&self) -> u64 {
        self.opened_tick
    }
}

impl SceneFrame {
    /// The usable corridor of every body that carries route state on a
    /// projected facility band, ascending by agent id.
    ///
    /// The corridor is the band inset by the body's half width and its resolved
    /// target clearance: the offsets the body plus its clearance fit inside.
    /// The compiled interval the kernel steers within is not projected onto a
    /// snapshot, so the frame derives its own from the band geometry it carries;
    /// a body with no route state, no guide path, no matching band, or no room
    /// inside it draws no corridor.
    pub fn corridors(&self) -> Vec<CorridorOverlay> {
        let mut corridors: Vec<CorridorOverlay> = self
            .bodies
            .iter()
            .filter_map(|body| self.corridor_of(body))
            .collect();
        corridors.sort_by_key(|corridor| corridor.agent);
        corridors
    }

    /// The fixed target offset of every body that has one, ascending by agent
    /// id.
    pub fn target_offsets(&self) -> Vec<TargetOffsetOverlay> {
        let mut targets: Vec<TargetOffsetOverlay> = self
            .bodies
            .iter()
            .filter_map(|body| {
                let state = body.route_state?;
                let offset_m = state.target_offset_m?;
                let left = left_normal(body.heading_rad);
                Some(TargetOffsetOverlay {
                    agent: body.id,
                    offset_m,
                    side: if offset_m >= 0.0 {
                        PassSide::Left
                    } else {
                        PassSide::Right
                    },
                    from_m: state.d_m,
                    anchor: body.position,
                    left,
                    target: body.position + left * (offset_m - state.d_m),
                })
            })
            .collect();
        targets.sort_by_key(|target| target.agent);
        targets
    }

    /// The predicted minimum clearance of every body that has one, ascending by
    /// agent id.
    pub fn predicted_gaps(&self) -> Vec<PredictedGapOverlay> {
        let mut gaps: Vec<PredictedGapOverlay> = self
            .bodies
            .iter()
            .filter_map(|body| {
                let state = body.route_state?;
                Some(PredictedGapOverlay {
                    agent: body.id,
                    predicted_min_clearance_m: state.predicted_min_clearance_m?,
                    target_clearance_m: state.target_clearance_m,
                    horizon_s: state.horizon_s,
                    anchor: body.position,
                })
            })
            .collect();
        gaps.sort_by_key(|gap| gap.agent);
        gaps
    }

    /// The maneuver state of every body with an open maneuver interval,
    /// ascending by agent id.
    ///
    /// The state is the live versioned sample the frame projects; the interval's
    /// own edge names the tactic, the edge, the displaced partner, the
    /// facilities, the target offset, and the reason.
    pub fn maneuver_overlays(&self) -> Vec<ManeuverOverlay> {
        self.tactical
            .maneuvers()
            .iter()
            .filter_map(|interval| {
                let Event::Maneuver {
                    kind,
                    to,
                    edge,
                    partner,
                    source_facility,
                    target_facility,
                    target_offset_m,
                    side,
                    reason,
                    ..
                } = interval.latest().event()
                else {
                    return None;
                };
                let live = self
                    .body(interval.agent())
                    .and_then(|body| body.route_state)
                    .map(|state| state.maneuver_state);
                Some(ManeuverOverlay {
                    agent: interval.agent(),
                    state: live.unwrap_or(*to),
                    kind: *kind,
                    edge: *edge,
                    partner: partner.map(|partner| partner.get() as usize),
                    source_facility: *source_facility,
                    target_facility: *target_facility,
                    target_offset_m: *target_offset_m,
                    side: *side,
                    reason: *reason,
                    opened_tick: interval.opened().tick(),
                })
            })
            .collect()
    }

    /// The wrong-way rule state of every body with an open opposing-traversal
    /// interval, ascending by agent id.
    ///
    /// The interval's opening record names the facility, the movement, the
    /// nominal direction, the reason, and whether the traversal violates the
    /// rule; the live versioned sample supplies the direction and the perceived
    /// rule when the body carries one, because the sample is the state the run
    /// actually perceived.
    pub fn wrong_way_overlays(&self) -> Vec<WrongWayOverlay> {
        self.tactical
            .wrong_way()
            .iter()
            .filter_map(|interval| {
                let Event::OpposingTraversal {
                    facility,
                    movement,
                    direction,
                    nominal_direction,
                    perceived_rule,
                    reason,
                    violating,
                    ..
                } = interval.opened().event()
                else {
                    return None;
                };
                let live = self
                    .body(interval.agent())
                    .and_then(|body| body.route_state);
                Some(WrongWayOverlay {
                    agent: interval.agent(),
                    facility: *facility,
                    movement: *movement,
                    direction: live
                        .and_then(|state| state.opposing_direction)
                        .unwrap_or(*direction),
                    nominal_direction: *nominal_direction,
                    perceived_rule: live
                        .and_then(|state| state.perceived_rule)
                        .or(*perceived_rule),
                    reason: *reason,
                    violating: *violating,
                    opened_tick: interval.opened().tick(),
                })
            })
            .collect()
    }

    /// The usable corridor of `body`, or `None` when the frame cannot derive
    /// one.
    fn corridor_of(&self, body: &SceneBody) -> Option<CorridorOverlay> {
        let state = body.route_state?;
        let facility = self.facility_of(body)?;
        let ring = facility.points();
        if ring.len() < 3 {
            return None;
        }
        let left = left_normal(body.heading_rad);
        // The ring is measured from the body, so a band edge `left_extent_m`
        // away in the body's own frame sits at absolute offset
        // `left_extent_m + d_m`; the usable interval is that edge inset by the
        // body's half width and its resolved clearance.
        let left_extent_m = ring_extent_m(ring, body.position, left)?;
        let right_extent_m = ring_extent_m(ring, body.position, -left)?;
        let inset_m = body.width_m * 0.5 + state.target_clearance_m.unwrap_or(0.0);
        let d_max_m = left_extent_m + state.d_m - inset_m;
        let d_min_m = inset_m + state.d_m - right_extent_m;
        if d_min_m > d_max_m {
            return None;
        }
        Some(CorridorOverlay {
            agent: body.id,
            facility: facility.id(),
            anchor: body.position,
            left,
            along_m: state.s_m,
            offset_m: state.d_m,
            d_min_m,
            d_max_m,
            clearance_m: state.target_clearance_m,
            horizon_s: state.horizon_s,
        })
    }

    /// The projected facility band `body` follows: the band whose reference
    /// path is the body's guide path, preferring one whose ring contains the
    /// body.
    fn facility_of(&self, body: &SceneBody) -> Option<&SceneFacility> {
        let path = body.path?;
        let mut fallback = None;
        for facility in self.geometry.facilities() {
            if facility.reference().map(|reference| reference.path()) != Some(path) {
                continue;
            }
            if ring_contains(facility.points(), body.position) {
                return Some(facility);
            }
            fallback.get_or_insert(facility);
        }
        fallback
    }
}

/// One-line inspector text for a body's usable corridor; `None` means the body
/// carries no route state on a projected facility band.
///
/// The presentation layer owns this wording so every backend describes the same
/// corridor the same way.
pub fn corridor_summary(corridor: Option<&CorridorOverlay>) -> String {
    let Some(corridor) = corridor else {
        return "none (no route state on a projected facility band)".to_owned();
    };
    format!(
        "#{agent}  facility {facility}  usable corridor {d_min:.2}..{d_max:.2} m  \
         offset {offset:.2} m  clearance {clearance}  horizon {horizon}",
        agent = corridor.agent,
        facility = corridor.facility.get(),
        d_min = corridor.d_min_m,
        d_max = corridor.d_max_m,
        offset = corridor.offset_m,
        clearance = metres(corridor.clearance_m),
        horizon = seconds(corridor.horizon_s),
    )
}

/// One-line inspector text for a body's fixed target offset; `None` means the
/// body has no fixed target.
pub fn target_offset_summary(target: Option<&TargetOffsetOverlay>) -> String {
    let Some(target) = target else {
        return "none (no fixed target offset)".to_owned();
    };
    format!(
        "#{agent}  target offset {offset:.2} m ({side})  from {from:.2} m",
        agent = target.agent,
        offset = target.offset_m,
        side = target.side.label(),
        from = target.from_m,
    )
}

/// One-line inspector text for a body's predicted minimum clearance; `None`
/// means the run predicted no clearance for it.
pub fn predicted_gap_summary(gap: Option<&PredictedGapOverlay>) -> String {
    let Some(gap) = gap else {
        return "none (no predicted clearance)".to_owned();
    };
    format!(
        "#{agent}  predicted min clearance {predicted:.2} m  target {target}  margin {margin}  \
         horizon {horizon}",
        agent = gap.agent,
        predicted = gap.predicted_min_clearance_m,
        target = metres(gap.target_clearance_m),
        margin = metres(gap.margin_m()),
        horizon = seconds(gap.horizon_s),
    )
}

/// One-line inspector text for a body's maneuver state; `None` means the frame
/// carries no open maneuver interval for it.
pub fn maneuver_summary(maneuver: Option<&ManeuverOverlay>) -> String {
    let Some(maneuver) = maneuver else {
        return "none (no open maneuver)".to_owned();
    };
    format!(
        "#{agent}  maneuver {state}  tactic {kind}  edge {edge}  reason {reason}  \
         partner {partner}  target {offset:.2} m ({side})  facility {source} -> {target_facility}",
        agent = maneuver.agent,
        state = maneuver.state.label(),
        kind = maneuver.kind.label(),
        edge = maneuver.edge.label(),
        reason = maneuver.reason.label(),
        partner = body_label(maneuver.partner),
        offset = maneuver.target_offset_m,
        side = maneuver.side.label(),
        source = maneuver.source_facility.get(),
        target_facility = maneuver
            .target_facility
            .map_or_else(|| "none".to_owned(), |facility| facility.get().to_string()),
    )
}

/// One-line inspector text for a body's wrong-way rule state; `None` means the
/// frame carries no open opposing traversal for it.
pub fn wrong_way_summary(wrong_way: Option<&WrongWayOverlay>) -> String {
    let Some(wrong_way) = wrong_way else {
        return "none (no open opposing traversal)".to_owned();
    };
    format!(
        "#{agent}  opposing facility {facility}  movement {movement}  direction {direction}  \
         nominal {nominal}  rule {rule}  reason {reason}  violating {violating}",
        agent = wrong_way.agent,
        facility = wrong_way.facility.get(),
        movement = wrong_way
            .movement
            .map_or_else(|| "none".to_owned(), |movement| movement.get().to_string()),
        direction = wrong_way.direction.label(),
        nominal = wrong_way.nominal_direction.label(),
        rule = wrong_way.perceived_rule.map_or("none", |rule| rule.label()),
        reason = wrong_way.reason.label(),
        violating = wrong_way.violating,
    )
}

/// Inspector text for an optional scalar in metres.
fn metres(value: Option<f64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| format!("{value:.2} m"))
}

/// Inspector text for an optional scalar in seconds.
fn seconds(value: Option<f64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| format!("{value:.2} s"))
}

/// Inspector name of an optional body: `#id`, or `none`.
fn body_label(agent: Option<usize>) -> String {
    agent.map_or_else(|| "none".to_owned(), |agent| format!("#{agent}"))
}

/// Unit world direction of positive lateral offset for `heading_rad`: the
/// heading rotated a quarter turn to the left.
fn left_normal(heading_rad: f64) -> DVec2 {
    let (sin, cos) = heading_rad.sin_cos();
    DVec2::new(-sin, cos)
}

/// Distance from `origin` to the nearest crossing of the closed ring along the
/// unit `axis`, or `None` when the ray never leaves the ring.
///
/// This is the band half extent the corridor is inset from: a body inside its
/// band crosses the ring once in each perpendicular direction.
fn ring_extent_m(ring: &[DVec2], origin: DVec2, axis: DVec2) -> Option<f64> {
    let mut nearest: Option<f64> = None;
    for (index, start) in ring.iter().enumerate() {
        let end = ring[(index + 1) % ring.len()];
        let edge = end - *start;
        let denominator = axis.x * edge.y - axis.y * edge.x;
        if denominator.abs() < 1e-12 {
            // A ring edge parallel to the ray never bounds it.
            continue;
        }
        let delta = *start - origin;
        let along_ray = (delta.x * edge.y - delta.y * edge.x) / denominator;
        let along_edge = (delta.x * axis.y - delta.y * axis.x) / denominator;
        if along_ray > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&along_edge) {
            nearest = Some(nearest.map_or(along_ray, |held: f64| held.min(along_ray)));
        }
    }
    nearest.filter(|extent| extent.is_finite())
}

/// Whether the closed ring contains `point`, by the even-odd crossing rule.
fn ring_contains(ring: &[DVec2], point: DVec2) -> bool {
    let mut inside = false;
    for (index, start) in ring.iter().enumerate() {
        let end = ring[(index + 1) % ring.len()];
        if (start.y > point.y) != (end.y > point.y) {
            let crossing = (end.x - start.x) * (point.y - start.y) / (end.y - start.y) + start.x;
            if point.x < crossing {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use hekate_model::{BodyKind, CompiledScenario, PathId, parse_scenario_source_v2};
    use hekate_sim::{AgentId, AgentMode, DespawnReason, RouteStateSample};

    use crate::clock::Speed;
    use crate::safety::SafetyOverlay;
    use crate::scene::{FrameStatus, Overlay, Overlays, SceneGeometry, Viewport};

    /// A version-2 band: one 3 m facility whose reference path is the band's
    /// centreline, so a body's signed offset is its world `y`.
    fn bands() -> CompiledScenario {
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
    fn sample(d_m: f64) -> RouteStateSample {
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
    fn body(id: usize, position: DVec2, state: Option<RouteStateSample>) -> SceneBody {
        SceneBody {
            id,
            position,
            heading_rad: 0.0,
            length_m: 4.5,
            width_m: 1.8,
            mode: AgentMode::Vehicle,
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

    /// A frame over the band scenario carrying `bodies` and `tactical`.
    fn projected(bodies: Vec<SceneBody>, tactical: TacticalOverlay) -> SceneFrame {
        let scenario = bands();
        SceneFrame {
            scenario_id: scenario.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: bodies.len(),
                speed: Speed::Real,
                paused: false,
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

    fn maneuver(agent: usize, to: ManeuverState, edge: ManeuverEdge) -> Event {
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

    fn opposing(agent: usize, entering: bool) -> Event {
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

    #[test]
    fn the_fold_opens_and_closes_maneuver_and_wrong_way_intervals() {
        let mut tactical = TacticalOverlay::default();
        assert!(tactical.is_empty());
        tactical.observe(
            10,
            &[
                maneuver(2, ManeuverState::Preparing, ManeuverEdge::Attempted),
                maneuver(0, ManeuverState::Preparing, ManeuverEdge::Attempted),
                opposing(1, true),
            ],
        );
        // Intervals ascend by agent regardless of record order.
        assert_eq!(
            tactical
                .maneuvers()
                .iter()
                .map(|interval| interval.agent())
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(tactical.maneuvers()[0].opened().tick(), 10);
        assert_eq!(
            tactical
                .wrong_way()
                .iter()
                .map(|interval| interval.agent())
                .collect::<Vec<_>>(),
            vec![1]
        );

        // A further edge of an open interval updates it without reopening it.
        tactical.observe(
            11,
            &[maneuver(
                0,
                ManeuverState::Committed,
                ManeuverEdge::Committed,
            )],
        );
        assert_eq!(tactical.maneuvers()[0].opened().tick(), 10);
        assert_eq!(tactical.maneuvers()[0].latest().tick(), 11);

        // A despawn clears every interval the agent held, without a closing
        // record.
        tactical.observe(
            12,
            &[Event::Despawned {
                agent: AgentId::from_index(0),
                path: PathId::from_index(0),
                reason: DespawnReason::ExitedPath,
            }],
        );
        assert_eq!(
            tactical
                .maneuvers()
                .iter()
                .map(|interval| interval.agent())
                .collect::<Vec<_>>(),
            vec![2]
        );

        // The closing edges empty the rest: a maneuver that returns to
        // following, and an opposing traversal that leaves.
        tactical.observe(
            13,
            &[
                maneuver(2, ManeuverState::Following, ManeuverEdge::Completed),
                opposing(1, false),
            ],
        );
        assert!(tactical.is_empty(), "{tactical:?}");
    }

    #[test]
    fn a_maneuver_overlay_names_its_state_tactic_edge_and_reason() {
        let mut tactical = TacticalOverlay::default();
        tactical.observe(
            7,
            &[maneuver(
                0,
                ManeuverState::Committed,
                ManeuverEdge::Committed,
            )],
        );
        let mut state = sample(0.0);
        state.maneuver_state = ManeuverState::Committed;
        let frame = projected(vec![body(0, DVec2::new(50.0, 0.0), Some(state))], tactical);
        let overlay = frame.maneuver_overlays()[0];
        assert_eq!(overlay.agent(), 0);
        assert_eq!(overlay.state(), ManeuverState::Committed);
        assert_eq!(overlay.kind(), TacticKind::Overtake);
        assert_eq!(overlay.edge(), ManeuverEdge::Committed);
        assert_eq!(overlay.partner(), Some(1));
        assert_eq!(overlay.source_facility(), FacilityId::from_index(0));
        assert_eq!(overlay.target_facility(), None);
        assert_eq!(overlay.target_offset_m(), 0.8);
        assert_eq!(overlay.side(), PassSide::Left);
        assert_eq!(overlay.reason(), ManeuverReasonCode::SlowerLeader);
        assert_eq!(overlay.opened_tick(), 7);

        // The live sample is the state the frame projects; a body that carries
        // none falls back to the interval's own edge.
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.0), None)],
            TacticalOverlay::default(),
        );
        assert!(frame.maneuver_overlays().is_empty());
    }

    #[test]
    fn a_corridor_is_the_band_inset_by_the_body_and_its_clearance() {
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.0), Some(sample(0.0)))],
            TacticalOverlay::default(),
        );
        let corridor = frame.corridors()[0];
        assert_eq!(corridor.agent(), 0);
        assert_eq!(corridor.facility(), FacilityId::from_index(0));
        assert_eq!(corridor.anchor(), DVec2::new(50.0, 0.0));
        assert_eq!(corridor.left(), DVec2::new(0.0, 1.0));
        assert_eq!(corridor.along_m(), 50.0);
        assert_eq!(corridor.offset_m(), 0.0);
        // A 3 m band less the 1.8 m body and its 0.3 m clearance leaves the
        // kernel's own +/-0.3 m usable interval.
        assert!((corridor.d_max_m() - 0.3).abs() < 1e-9, "{corridor:?}");
        assert!((corridor.d_min_m() + 0.3).abs() < 1e-9, "{corridor:?}");
        assert_eq!(
            corridor.interval_m(),
            (corridor.d_min_m(), corridor.d_max_m())
        );
        assert_eq!(corridor.clearance_m(), Some(0.3));
        assert_eq!(corridor.horizon_s(), Some(2.0));

        // The interval is absolute, so a body offset inside the band reports
        // the same envelope it can occupy, not one measured from itself.
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.5), Some(sample(0.5)))],
            TacticalOverlay::default(),
        );
        let corridor = frame.corridors()[0];
        assert!((corridor.d_max_m() - 0.3).abs() < 1e-9, "{corridor:?}");
        assert!((corridor.d_min_m() + 0.3).abs() < 1e-9, "{corridor:?}");
    }

    #[test]
    fn a_body_with_no_route_state_carries_no_tactical_overlay() {
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.0), None)],
            TacticalOverlay::default(),
        );
        assert!(frame.corridors().is_empty());
        assert!(frame.target_offsets().is_empty());
        assert!(frame.predicted_gaps().is_empty());
        assert!(frame.maneuver_overlays().is_empty());
        assert!(frame.wrong_way_overlays().is_empty());
        assert!(corridor_summary(None).starts_with("none"));
        assert!(target_offset_summary(None).starts_with("none"));
        assert!(predicted_gap_summary(None).starts_with("none"));
        assert!(maneuver_summary(None).starts_with("none"));
        assert!(wrong_way_summary(None).starts_with("none"));
    }

    #[test]
    fn a_target_offset_and_predicted_gap_project_from_the_route_sample() {
        let mut state = sample(0.0);
        state.target_offset_m = Some(0.8);
        state.predicted_min_clearance_m = Some(0.9);
        state.target_clearance_m = Some(0.5);
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.2), Some(state))],
            TacticalOverlay::default(),
        );
        let target = frame.target_offsets()[0];
        assert_eq!(target.agent(), 0);
        assert_eq!(target.offset_m(), 0.8);
        assert_eq!(target.side(), PassSide::Left);
        assert_eq!(target.from_m(), 0.0);
        assert_eq!(target.anchor(), DVec2::new(50.0, 0.2));
        // The target sits 0.8 m to the left of the body's own offset.
        assert_eq!(target.target(), DVec2::new(50.0, 1.0));

        let gap = frame.predicted_gaps()[0];
        assert_eq!(gap.agent(), 0);
        assert_eq!(gap.predicted_min_clearance_m(), 0.9);
        assert_eq!(gap.target_clearance_m(), Some(0.5));
        assert_eq!(gap.horizon_s(), Some(2.0));
        assert_eq!(gap.margin_m(), Some(0.4));
        assert_eq!(gap.anchor(), DVec2::new(50.0, 0.2));

        // A right-side target reports its own sign.
        let mut state = sample(0.0);
        state.target_offset_m = Some(-0.4);
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.0), Some(state))],
            TacticalOverlay::default(),
        );
        assert_eq!(frame.target_offsets()[0].side(), PassSide::Right);

        // A sample with neither value projects neither primitive.
        let frame = projected(
            vec![body(0, DVec2::new(50.0, 0.0), Some(sample(0.0)))],
            TacticalOverlay::default(),
        );
        assert!(frame.target_offsets().is_empty());
        assert!(frame.predicted_gaps().is_empty());
    }

    #[test]
    fn a_wrong_way_overlay_names_its_open_interval_and_live_rule() {
        let mut tactical = TacticalOverlay::default();
        tactical.observe(4, &[opposing(0, true)]);
        let mut state = sample(-0.2);
        state.perceived_rule = Some(PermissionEffect::Permit);
        state.opposing_direction = Some(MovementDirection::Forward);
        let frame = projected(vec![body(0, DVec2::new(50.0, -0.2), Some(state))], tactical);
        let overlay = frame.wrong_way_overlays()[0];
        assert_eq!(overlay.agent(), 0);
        assert_eq!(overlay.facility(), FacilityId::from_index(0));
        assert_eq!(overlay.movement(), Some(MovementId::from_index(0)));
        assert_eq!(overlay.nominal_direction(), NominalDirection::Forward);
        assert_eq!(overlay.reason(), WrongWayReason::NoncompliantChoice);
        assert!(overlay.violating());
        assert_eq!(overlay.opened_tick(), 4);
        // The live sample is the state the run perceived; the interval's own
        // record is the fallback when the body carries none.
        assert_eq!(overlay.direction(), MovementDirection::Forward);
        assert_eq!(overlay.perceived_rule(), Some(PermissionEffect::Permit));

        let frame = projected(vec![body(0, DVec2::new(50.0, -0.2), None)], {
            let mut tactical = TacticalOverlay::default();
            tactical.observe(4, &[opposing(0, true)]);
            tactical
        });
        let overlay = frame.wrong_way_overlays()[0];
        assert_eq!(overlay.direction(), MovementDirection::Reverse);
        assert_eq!(overlay.perceived_rule(), Some(PermissionEffect::Prohibit));

        // The interval closing removes the overlay even though the body lives.
        let mut tactical = TacticalOverlay::default();
        tactical.observe(4, &[opposing(0, true)]);
        tactical.observe(5, &[opposing(0, false)]);
        let frame = projected(
            vec![body(0, DVec2::new(50.0, -0.2), Some(sample(-0.2)))],
            tactical,
        );
        assert!(frame.wrong_way_overlays().is_empty());
    }

    #[test]
    fn the_inspector_summaries_name_every_identifier() {
        let mut tactical = TacticalOverlay::default();
        tactical.observe(
            7,
            &[
                maneuver(0, ManeuverState::Committed, ManeuverEdge::Committed),
                opposing(0, true),
            ],
        );
        let mut state = sample(0.0);
        state.maneuver_state = ManeuverState::Committed;
        state.target_offset_m = Some(0.8);
        state.predicted_min_clearance_m = Some(0.9);
        state.target_clearance_m = Some(0.5);
        state.opposing_direction = Some(MovementDirection::Reverse);
        state.perceived_rule = Some(PermissionEffect::Prohibit);
        let frame = projected(vec![body(0, DVec2::new(50.0, 0.0), Some(state))], tactical);

        let corridor = corridor_summary(frame.corridors().first());
        assert_eq!(
            corridor,
            "#0  facility 0  usable corridor -0.10..0.10 m  offset 0.00 m  \
             clearance 0.50 m  horizon 2.00 s"
        );
        let target = target_offset_summary(frame.target_offsets().first());
        assert_eq!(target, "#0  target offset 0.80 m (left)  from 0.00 m");
        let gap = predicted_gap_summary(frame.predicted_gaps().first());
        assert_eq!(
            gap,
            "#0  predicted min clearance 0.90 m  target 0.50 m  margin 0.40 m  horizon 2.00 s"
        );
        let maneuver = maneuver_summary(frame.maneuver_overlays().first());
        assert_eq!(
            maneuver,
            "#0  maneuver committed  tactic overtake  edge committed  reason slower_leader  \
             partner #1  target 0.80 m (left)  facility 0 -> none"
        );
        let wrong_way = wrong_way_summary(frame.wrong_way_overlays().first());
        assert_eq!(
            wrong_way,
            "#0  opposing facility 0  movement 0  direction reverse  nominal forward  \
             rule prohibit  reason noncompliant_choice  violating true"
        );
    }

    #[test]
    fn every_tactical_overlay_flag_toggles_independently() {
        let mut overlays = Overlays::default();
        let flags = |overlays: &Overlays| {
            [
                overlays.geometry,
                overlays.vectors,
                overlays.safety,
                overlays.corridor,
                overlays.target_offset,
                overlays.predicted_gap,
                overlays.maneuver,
                overlays.wrong_way,
            ]
        };
        for (index, overlay) in [
            Overlay::Geometry,
            Overlay::Vectors,
            Overlay::Safety,
            Overlay::Corridor,
            Overlay::TargetOffset,
            Overlay::PredictedGap,
            Overlay::Maneuver,
            Overlay::WrongWay,
        ]
        .into_iter()
        .enumerate()
        {
            let before = flags(&overlays);
            overlays.toggle(overlay);
            let after = flags(&overlays);
            assert_ne!(before[index], after[index], "{overlay:?} did not flip");
            assert_eq!(
                after
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, value)| *value)
                    .collect::<Vec<_>>(),
                before
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, value)| *value)
                    .collect::<Vec<_>>(),
                "{overlay:?} flipped another overlay"
            );
        }
    }
}
