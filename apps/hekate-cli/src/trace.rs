//! Canonical, hashable serialization of a headless run.
//!
//! A canonical trace is the durable behavioral record of one run: the same
//! scenario, seed, and configuration must always produce the same bytes, and
//! therefore the same SHA-256 hash. It is JSON Lines with one compact record
//! per line in a fixed field order, so a checked-in golden file can be
//! compared byte for byte instead of through a fuzzy diff.
//!
//! Record shapes are deliberately closed structs with declaration-order fields.
//! Never serialize a map here: field order is part of the hash contract.
//!
//! The run header names both the scenario schema version it was authored
//! against and the [`EVENT_VERSION`] of the records that follow, so a consumer
//! reading only the artifact can tell which event union to expect.

use std::fmt::Write as _;

use hekate_model::CompiledScenario;
use hekate_sim::{
    ClosePassBand, DespawnReason, EVENT_VERSION, Event, InitError, RegionKey, RunConfig,
    RunSummary, Simulation, StepOutput,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::run_dir::TrajectorySampling;
use crate::run_metrics::{RunMetrics, RunMetricsRecorder};
use crate::trajectories::{TrajectoryRecorder, TrajectorySample};

/// A canonical trace plus the hash of its exact bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    bytes: Vec<u8>,
    hash: String,
}

impl Trace {
    /// The canonical trace bytes: UTF-8 JSON Lines with `\n` terminators.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Lowercase hexadecimal SHA-256 of [`Self::bytes`].
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

/// Build a canonical trace incrementally from a simulation the caller drives.
///
/// [`canonical_trace`] is the common entry point, but a golden-parity test may
/// advance the same kernel through a presentation clock instead of a plain
/// step loop. Recording through this type gives that driven run the same bytes
/// and hash as the direct run, so clock pacing cannot change a trace.
///
/// The header is written from `sim` and `config` at construction, and `ticks`
/// must be the total number of steps the caller will record.
pub struct TraceRecorder {
    bytes: Vec<u8>,
}

impl TraceRecorder {
    /// Start a trace for `sim`, which the caller will step `ticks` times.
    pub fn new(sim: &Simulation, config: &RunConfig, ticks: u64) -> Self {
        let mut bytes = Vec::new();
        write_line(
            &mut bytes,
            &RunHeader {
                kind: "run",
                scenario_id: sim.scenario().id(),
                schema_version: sim.scenario().schema_version(),
                event_version: EVENT_VERSION,
                seed: config.seed(),
                step_s: config.step().as_secs(),
                ticks,
            },
        );
        Self { bytes }
    }

    /// Record one completed step's events, attributed to its tick.
    ///
    /// Events are recorded in kernel emission order, which is the documented
    /// within-tick order: ascending agent, then event kind, then the variant's
    /// stable key. Every event a step produces is attributed to the tick that
    /// step completes.
    pub fn record(&mut self, output: &StepOutput<'_>) {
        let tick = output.time().tick();
        for event in output.events() {
            write_line(&mut self.bytes, &EventRecord::new(tick, event.clone()));
        }
    }

    /// Finish the trace with the run summary and hash its exact bytes.
    pub fn finish(mut self, summary: RunSummary) -> Trace {
        write_line(
            &mut self.bytes,
            &RunFooter {
                kind: "summary",
                ticks: summary.ticks(),
                spawned: summary.spawned(),
                despawned: summary.despawned(),
                remaining: summary.remaining(),
            },
        );
        let hash = sha256_hex(&self.bytes);
        Trace {
            bytes: self.bytes,
            hash,
        }
    }
}

/// Run a compiled scenario for exactly `ticks` fixed steps and serialize it.
///
/// Events are recorded in kernel emission order, which is the documented
/// within-tick order: ascending agent, then event kind, then the variant's
/// stable key. Every event a step produces is attributed to the tick that step
/// completes.
pub fn canonical_trace(
    scenario: CompiledScenario,
    config: RunConfig,
    ticks: u64,
) -> Result<Trace, InitError> {
    canonical_run(scenario, config, ticks).map(|(trace, _)| trace)
}

/// Run a compiled scenario for exactly `ticks` fixed steps and return both the
/// canonical trace and the kernel's summary of the run.
///
/// A run directory records the summary alongside the trace, and [`canonical_trace`]
/// delegates here, so both consumers share one run loop: the recorded bytes
/// cannot drift from the golden contract just because a caller also wants the
/// run's totals.
pub fn canonical_run(
    scenario: CompiledScenario,
    config: RunConfig,
    ticks: u64,
) -> Result<(Trace, RunSummary), InitError> {
    canonical_run_sampled(scenario, config, ticks, &TrajectorySampling::off())
        .map(|(trace, summary, _)| (trace, summary))
}

/// Run a compiled scenario for exactly `ticks` fixed steps, also capturing the
/// trajectory rows `sampling` retains.
///
/// The trace, the summary, and the recorded bytes are the ones [`canonical_run`]
/// produces: both entry points drive the same loop, so asking a run for sampled
/// trajectories cannot change the canonical trace or its hash. Sampling only
/// decides which rows the caller receives, and the declared policy bounds them.
pub fn canonical_run_sampled(
    scenario: CompiledScenario,
    config: RunConfig,
    ticks: u64,
    sampling: &TrajectorySampling,
) -> Result<(Trace, RunSummary, Vec<TrajectorySample>), InitError> {
    canonical_run_captured(scenario, config, ticks, sampling)
        .map(|(trace, summary, trajectories, _)| (trace, summary, trajectories))
}

/// Run a compiled scenario for exactly `ticks` fixed steps, also capturing the
/// versioned metric values the run reports.
///
/// The trace, summary, and trajectory bytes are the ones [`canonical_run_sampled`]
/// produces: both entry points drive the same loop, so capturing metrics cannot
/// change the canonical trace or its hash. The returned [`RunMetrics`] is read
/// from the live simulation before it is consumed, so it observes exactly the
/// run the trace records.
///
/// The run ends at a close boundary: before the metric capture, the run's
/// still-open close-pass intervals are closed, so a pass in progress at the
/// final tick is reported with the evidence a completed one carries rather than
/// dropped. The closure emits no event — no tick remains to carry one — so the
/// recorded trace bytes stay the canonical ones.
pub fn canonical_run_captured(
    scenario: CompiledScenario,
    config: RunConfig,
    ticks: u64,
    sampling: &TrajectorySampling,
) -> Result<(Trace, RunSummary, Vec<TrajectorySample>, RunMetrics), InitError> {
    let mut sim = Simulation::new(scenario, config)?;
    let mut recorder = TraceRecorder::new(&sim, &config, ticks);
    let mut trajectories = TrajectoryRecorder::new(*sampling);
    let mut metrics = RunMetricsRecorder::new();

    for _ in 0..ticks {
        let output = sim.step();
        recorder.record(&output);
        metrics.record(&output);
        // The step's borrow ends with its last use, so the trajectory recorder
        // observes the same completed tick through an immutable borrow.
        trajectories.observe(&sim);
    }

    // The run's last tick has been observed: an interval still alongside now
    // closes as a termination, before the capture below reads the tracker. The
    // closure emits no event — no tick remains to carry one — so the recorded
    // trace bytes are unchanged.
    sim.close_open_close_passes();

    // The metric capture reads the live simulation, so it closes before
    // `finish` consumes the simulation.
    let metrics = metrics.finish(&sim);
    let summary = sim.finish();
    let trace = recorder.finish(summary.clone());
    Ok((trace, summary, trajectories.finish(), metrics))
}

fn write_line<T: Serialize>(bytes: &mut Vec<u8>, record: &T) {
    serde_json::to_writer(&mut *bytes, record).expect("canonical trace records serialize");
    bytes.push(b'\n');
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hash = String::with_capacity(64);
    for byte in digest {
        write!(hash, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hash
}

/// The self-describing run header, always the first line of a trace.
#[derive(Serialize)]
struct RunHeader<'a> {
    kind: &'static str,
    scenario_id: &'a str,
    schema_version: u32,
    event_version: u32,
    seed: u64,
    step_s: f64,
    ticks: u64,
}

/// One typed event, optionally carrying the fields that variant owns.
///
/// Field order is the serialization order, so variant-specific fields are
/// appended after the fields every event carries; a `None` field is omitted and
/// leaves the bytes of the other variants untouched.
#[derive(Serialize)]
struct EventRecord {
    kind: &'static str,
    tick: u64,
    event: &'static str,
    agent: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crossing: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    yielding: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clearance_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contacting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entering: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    joined: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    violation: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    control: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<u32>,
    // The increment-2 records append their own fields here, in the contract's
    // payload order and sorted so every variant's own fields stay in that
    // order, so no version-2 line changes byte.
    #[serde(skip_serializing_if = "Option::is_none")]
    maneuver_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edge: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    partner: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_facility: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_facility: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_offset_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_facility: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_facility: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_direction: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_direction: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    via: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    s_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    d_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permitted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facility: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    movement: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    direction: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nominal_direction: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    perceived_rule: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    violating: Option<bool>,
    // The close-pass record appends its own fields last, in the contract's
    // payload order, so no earlier variant's line changes byte.
    #[serde(skip_serializing_if = "Option::is_none")]
    min_clearance_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_clearance_time_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relative_speed_mps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bands: Option<Vec<ClosePassBandRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    violating_bands: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crossed_boundary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entered_opposing: Option<bool>,
}

/// One clearance band of a `ClosePass` record: its stable id and the seconds
/// the pair's exact clearance sat inside it.
#[derive(Serialize)]
struct ClosePassBandRecord {
    band: u32,
    duration_s: f64,
}

impl From<&ClosePassBand> for ClosePassBandRecord {
    fn from(band: &ClosePassBand) -> Self {
        Self {
            band: band.band.get(),
            duration_s: band.duration_s,
        }
    }
}

/// A record with every field empty but the ones this variant owns.
impl EventRecord {
    /// The empty record for `tick`, before the variant's own fields are set.
    const fn empty(tick: u64, event: &'static str, agent: u32) -> Self {
        Self {
            kind: "event",
            tick,
            event,
            agent,
            path: None,
            distance_m: None,
            reason: None,
            crossing: None,
            yielding: None,
            mode: None,
            other: None,
            clearance_m: None,
            contacting: None,
            entering: None,
            joined: None,
            active: None,
            violation: None,
            control: None,
            region_kind: None,
            region: None,
            maneuver_kind: None,
            from: None,
            to: None,
            edge: None,
            partner: None,
            source_facility: None,
            target_facility: None,
            target_offset_m: None,
            from_facility: None,
            to_facility: None,
            from_direction: None,
            to_direction: None,
            via: None,
            side: None,
            s_m: None,
            d_m: None,
            permitted: None,
            facility: None,
            movement: None,
            direction: None,
            nominal_direction: None,
            perceived_rule: None,
            violating: None,
            min_clearance_m: None,
            min_clearance_time_s: None,
            relative_speed_mps: None,
            bands: None,
            violating_bands: None,
            crossed_boundary: None,
            entered_opposing: None,
        }
    }

    fn new(tick: u64, event: Event) -> Self {
        match event {
            Event::Spawned {
                agent,
                mode,
                path,
                distance_m,
            } => Self {
                path: Some(path.get()),
                distance_m: Some(distance_m),
                mode: Some(mode.label()),
                ..Self::empty(tick, "spawned", agent.get())
            },
            Event::Despawned {
                agent,
                path,
                reason,
            } => Self {
                path: Some(path.get()),
                reason: Some(match reason {
                    DespawnReason::ExitedPath => "exited_path",
                }),
                ..Self::empty(tick, "despawned", agent.get())
            },
            Event::Yielded {
                agent,
                crossing,
                yielding,
            } => Self {
                crossing: Some(crossing.get()),
                yielding: Some(yielding),
                ..Self::empty(tick, "yielded", agent.get())
            },
            Event::Collision {
                agent,
                other,
                clearance_m,
                contacting,
            } => Self {
                other: Some(other.get()),
                clearance_m: Some(clearance_m),
                contacting: Some(contacting),
                ..Self::empty(tick, "collision", agent.get())
            },
            Event::NearMiss {
                agent,
                other,
                clearance_m,
                entering,
            } => Self {
                other: Some(other.get()),
                clearance_m: Some(clearance_m),
                entering: Some(entering),
                ..Self::empty(tick, "near_miss", agent.get())
            },
            Event::Violation { agent, kind } => Self {
                violation: Some(kind.label()),
                ..Self::empty(tick, "violation", agent.get())
            },
            Event::Entry { agent, region } => Self {
                region_kind: Some(region_kind_label(region)),
                region: Some(region.get()),
                ..Self::empty(tick, "entry", agent.get())
            },
            Event::Exit { agent, region } => Self {
                region_kind: Some(region_kind_label(region)),
                region: Some(region.get()),
                ..Self::empty(tick, "exit", agent.get())
            },
            Event::Queue { agent, joined } => Self {
                joined: Some(joined),
                ..Self::empty(tick, "queue", agent.get())
            },
            Event::ControlTransition {
                agent,
                control,
                active,
            } => Self {
                control: Some(control.label()),
                active: Some(active),
                ..Self::empty(tick, "control_transition", agent.get())
            },
            Event::Maneuver {
                agent,
                kind,
                from,
                to,
                edge,
                partner,
                source_facility,
                target_facility,
                target_offset_m,
                side,
                reason,
            } => Self {
                maneuver_kind: Some(kind.label()),
                from: Some(from.label()),
                to: Some(to.label()),
                edge: Some(edge.label()),
                partner: partner.map(|partner| partner.get()),
                source_facility: Some(source_facility.get()),
                target_facility: target_facility.map(|facility| facility.get()),
                target_offset_m: Some(target_offset_m),
                side: Some(side.label()),
                reason: Some(reason.label()),
                ..Self::empty(tick, "maneuver", agent.get())
            },
            Event::FacilityTransition {
                agent,
                from_facility,
                to_facility,
                from_direction,
                to_direction,
                via,
                side,
                s_m,
                d_m,
                permitted,
            } => Self {
                from_facility: Some(from_facility.get()),
                to_facility: Some(to_facility.get()),
                from_direction: Some(from_direction.label()),
                to_direction: Some(to_direction.label()),
                via: Some(via.label()),
                side: Some(side.label()),
                s_m: Some(s_m),
                d_m: Some(d_m),
                permitted: Some(permitted),
                ..Self::empty(tick, "facility_transition", agent.get())
            },
            Event::OpposingTraversal {
                agent,
                facility,
                movement,
                direction,
                nominal_direction,
                perceived_rule,
                reason,
                violating,
                entering,
            } => Self {
                facility: Some(facility.get()),
                movement: movement.map(|movement| movement.get()),
                direction: Some(direction.label()),
                nominal_direction: Some(nominal_direction.label()),
                perceived_rule: perceived_rule.map(|rule| rule.label()),
                reason: Some(reason.label()),
                violating: Some(violating),
                entering: Some(entering),
                ..Self::empty(tick, "opposing_traversal", agent.get())
            },
            Event::ClosePass {
                agent,
                partner,
                facility,
                side,
                min_clearance_m,
                min_clearance_time_s,
                relative_speed_mps,
                bands,
                violating_bands,
                crossed_boundary,
                entered_opposing,
            } => Self {
                partner: Some(partner.get()),
                facility: Some(facility.get()),
                side: Some(side.label()),
                min_clearance_m: Some(min_clearance_m),
                min_clearance_time_s: Some(min_clearance_time_s),
                relative_speed_mps: Some(relative_speed_mps),
                bands: Some(bands.iter().map(ClosePassBandRecord::from).collect()),
                violating_bands: Some(violating_bands.iter().map(|band| band.get()).collect()),
                crossed_boundary: Some(crossed_boundary),
                entered_opposing: Some(entered_opposing),
                ..Self::empty(tick, "close_pass", agent.get())
            },
        }
    }
}

/// Stable label of a region's kind, the tag its key spaces apart.
const fn region_kind_label(region: RegionKey) -> &'static str {
    match region {
        RegionKey::Crossing(_) => "crossing",
        RegionKey::ConflictRegion(_) => "conflict_region",
    }
}

/// The run totals, always the last line of a trace.
#[derive(Serialize)]
struct RunFooter {
    kind: &'static str,
    ticks: u64,
    spawned: u64,
    despawned: u64,
    remaining: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hekate_model::{CompiledScenario, parse_scenario_source};

    const WALKING: &str = r#"
    {
      schema_version: 1,
      id: 'walking_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] } ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      population: {
        vehicle_count: 2,
        vehicle_speed_mps: 12.0,
        vehicle_spacing_m: 20.0,
        vehicle_length_m: 4.5,
        vehicle_width_m: 1.8,
      },
    }
    "#;

    fn walking() -> CompiledScenario {
        let source = parse_scenario_source(WALKING).expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    fn lines(trace: &Trace) -> Vec<&str> {
        std::str::from_utf8(trace.bytes())
            .expect("trace is UTF-8")
            .lines()
            .collect()
    }

    #[test]
    fn same_input_produces_identical_bytes_and_hash() {
        let first = canonical_trace(walking(), RunConfig::new(0), 250).expect("runs");
        let second = canonical_trace(walking(), RunConfig::new(0), 250).expect("runs");
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(first.hash(), second.hash());
        assert_eq!(first.hash().len(), 64);
    }

    #[test]
    fn trace_is_json_lines_with_header_and_footer() {
        let trace = canonical_trace(walking(), RunConfig::new(0), 250).expect("runs");
        let lines = lines(&trace);
        assert!(trace.bytes().ends_with(b"\n"));
        assert_eq!(
            lines[0],
            r#"{"kind":"run","scenario_id":"walking_guide_v1","schema_version":1,"event_version":3,"seed":0,"step_s":0.05,"ticks":250}"#
        );
        assert_eq!(
            *lines.last().expect("footer"),
            r#"{"kind":"summary","ticks":250,"spawned":2,"despawned":2,"remaining":0}"#
        );
        for line in &lines {
            serde_json::from_str::<serde_json::Value>(line).expect("each record is JSON");
        }
    }

    #[test]
    fn event_fields_are_ordered_and_variant_specific() {
        let trace = canonical_trace(walking(), RunConfig::new(0), 250).expect("runs");
        let lines = lines(&trace);
        assert!(lines[1].starts_with(
            r#"{"kind":"event","tick":1,"event":"spawned","agent":0,"path":0,"distance_m":0.0"#
        ));
        // The spawned record carries the agent mode (F5), so a consumer can
        // interpret the body without a second lookup.
        assert!(lines[1].ends_with(r#","mode":"vehicle"}"#));
        assert!(
            lines
                .iter()
                .any(|line| line.contains(r#""event":"despawned""#)
                    && line.contains(r#""reason":"exited_path""#))
        );
        assert!(!lines.iter().any(|line| line.contains("null")));
    }

    #[test]
    fn events_are_grouped_by_ascending_tick() {
        let trace = canonical_trace(walking(), RunConfig::new(0), 250).expect("runs");
        let ticks: Vec<u64> = lines(&trace)
            .iter()
            .filter(|line| line.contains(r#""kind":"event""#))
            .map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).expect("event JSON");
                value["tick"].as_u64().expect("tick is a number")
            })
            .collect();
        assert!(ticks.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    /// The header names the event union, so a consumer reading only the
    /// artifact knows which record set follows.
    #[test]
    fn the_run_header_names_the_event_version() {
        let trace = canonical_trace(walking(), RunConfig::new(0), 1).expect("runs");
        let header: serde_json::Value =
            serde_json::from_str(lines(&trace)[0]).expect("header JSON");
        assert_eq!(
            header["event_version"].as_u64(),
            Some(u64::from(EVENT_VERSION))
        );
    }

    /// Every variant of the record union serializes through one shape: the
    /// shared fields first, then the fields that variant owns, and nothing
    /// serializes as `null`.
    #[test]
    fn each_record_shape_serializes_its_own_fields_in_order() {
        use hekate_model::{
            ClearanceBandId, ConflictRegionId, CrossingId, FacilityId, MovementDirection,
            MovementId, NominalDirection, PathId, PermissionEffect, TacticKind,
        };
        use hekate_sim::{
            AgentId, AgentMode, ClosePassBand, ControlTransitionKind, ManeuverEdge,
            ManeuverReasonCode, ManeuverState, PassSide, RegionKey, TransitionKind, ViolationKind,
            WrongWayReason,
        };

        let agent = AgentId::from_index(3);
        let partner = AgentId::from_index(7);
        let cases: [(&str, Event); 14] = [
            (
                "spawned",
                Event::Spawned {
                    agent,
                    mode: AgentMode::Pedestrian,
                    path: PathId::from_index(1),
                    distance_m: 2.5,
                },
            ),
            (
                "despawned",
                Event::Despawned {
                    agent,
                    path: PathId::from_index(1),
                    reason: DespawnReason::ExitedPath,
                },
            ),
            (
                "yielded",
                Event::Yielded {
                    agent,
                    crossing: CrossingId::from_index(2),
                    yielding: true,
                },
            ),
            (
                "collision",
                Event::Collision {
                    agent,
                    other: partner,
                    clearance_m: -0.25,
                    contacting: true,
                },
            ),
            (
                "near_miss",
                Event::NearMiss {
                    agent,
                    other: partner,
                    clearance_m: 0.75,
                    entering: false,
                },
            ),
            (
                "violation",
                Event::Violation {
                    agent,
                    kind: ViolationKind::RanRedLight,
                },
            ),
            (
                "entry",
                Event::Entry {
                    agent,
                    region: RegionKey::Crossing(CrossingId::from_index(0)),
                },
            ),
            (
                "exit",
                Event::Exit {
                    agent,
                    region: RegionKey::ConflictRegion(ConflictRegionId::from_index(4)),
                },
            ),
            (
                "queue",
                Event::Queue {
                    agent,
                    joined: true,
                },
            ),
            (
                "control_transition",
                Event::ControlTransition {
                    agent,
                    control: ControlTransitionKind::CrossingWait,
                    active: true,
                },
            ),
            (
                "maneuver",
                Event::Maneuver {
                    agent,
                    kind: TacticKind::Overtake,
                    from: ManeuverState::Following,
                    to: ManeuverState::Preparing,
                    edge: ManeuverEdge::Attempted,
                    partner: Some(partner),
                    source_facility: FacilityId::from_index(2),
                    target_facility: Some(FacilityId::from_index(3)),
                    target_offset_m: 2.6,
                    side: PassSide::Left,
                    reason: ManeuverReasonCode::SlowerLeader,
                },
            ),
            (
                "facility_transition",
                Event::FacilityTransition {
                    agent,
                    from_facility: FacilityId::from_index(1),
                    to_facility: FacilityId::from_index(2),
                    from_direction: MovementDirection::Forward,
                    to_direction: MovementDirection::Reverse,
                    via: TransitionKind::Lateral,
                    side: PassSide::Right,
                    s_m: 30.5,
                    d_m: -1.25,
                    permitted: false,
                },
            ),
            (
                "opposing_traversal",
                Event::OpposingTraversal {
                    agent,
                    facility: FacilityId::from_index(3),
                    movement: Some(MovementId::from_index(4)),
                    direction: MovementDirection::Reverse,
                    nominal_direction: NominalDirection::Forward,
                    perceived_rule: Some(PermissionEffect::Prohibit),
                    reason: WrongWayReason::NoncompliantChoice,
                    violating: true,
                    entering: false,
                },
            ),
            (
                "close_pass",
                Event::ClosePass {
                    agent,
                    partner,
                    facility: FacilityId::from_index(5),
                    side: PassSide::Left,
                    min_clearance_m: 0.3,
                    min_clearance_time_s: 1.5,
                    relative_speed_mps: 1.25,
                    bands: vec![ClosePassBand {
                        band: ClearanceBandId::from_index(0),
                        duration_s: 0.4,
                    }],
                    violating_bands: vec![ClearanceBandId::from_index(0)],
                    crossed_boundary: false,
                    entered_opposing: true,
                },
            ),
        ];

        for (name, event) in cases {
            let mut bytes = Vec::new();
            write_line(&mut bytes, &EventRecord::new(9, event));
            let line = std::str::from_utf8(&bytes).expect("UTF-8");
            let value: serde_json::Value = serde_json::from_str(line).expect("record JSON");
            assert_eq!(value["event"], name, "line {line}");
            assert_eq!(value["tick"], 9);
            assert_eq!(value["agent"], 3);
            // The shared prefix, then the fields this variant owns; nothing else.
            assert!(line.starts_with(&format!(
                "{{\"kind\":\"event\",\"tick\":9,\"event\":\"{name}\",\"agent\":3"
            )));
            assert!(!line.contains("null"), "line {line} serialized a null");
            let expected = match name {
                "spawned" => Some(r#""mode":"pedestrian""#),
                "despawned" => Some(r#""reason":"exited_path""#),
                "yielded" => Some(r#""crossing":2,"yielding":true"#),
                "collision" => Some(r#""other":7,"clearance_m":-0.25,"contacting":true"#),
                "near_miss" => Some(r#""other":7,"clearance_m":0.75,"entering":false"#),
                "violation" => Some(r#""violation":"ran_red_light""#),
                "entry" => Some(r#""region_kind":"crossing","region":0"#),
                "exit" => Some(r#""region_kind":"conflict_region","region":4"#),
                "queue" => Some(r#""joined":true"#),
                "control_transition" => Some(r#""active":true,"control":"crossing_wait""#),
                "maneuver" => Some(
                    r#""reason":"slower_leader","maneuver_kind":"overtake","from":"following","to":"preparing","edge":"attempted","partner":7,"source_facility":2,"target_facility":3,"target_offset_m":2.6,"side":"left""#,
                ),
                "facility_transition" => Some(
                    r#""from_facility":1,"to_facility":2,"from_direction":"forward","to_direction":"reverse","via":"lateral","side":"right","s_m":30.5,"d_m":-1.25,"permitted":false"#,
                ),
                "opposing_traversal" => Some(
                    r#""reason":"noncompliant_choice","entering":false,"facility":3,"movement":4,"direction":"reverse","nominal_direction":"forward","perceived_rule":"prohibit","violating":true"#,
                ),
                "close_pass" => Some(
                    r#""partner":7,"side":"left","facility":5,"min_clearance_m":0.3,"min_clearance_time_s":1.5,"relative_speed_mps":1.25,"bands":[{"band":0,"duration_s":0.4}],"violating_bands":[0],"crossed_boundary":false,"entered_opposing":true"#,
                ),
                _ => None,
            };
            let expected = expected.expect("every variant has a case");
            assert!(
                line.contains(expected),
                "{name} must serialize {expected}; got {line}"
            );
        }
    }
}
