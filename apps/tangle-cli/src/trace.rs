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

use serde::Serialize;
use sha2::{Digest, Sha256};
use tangle_model::CompiledScenario;
use tangle_sim::{
    DespawnReason, EVENT_VERSION, Event, InitError, RegionKey, RunConfig, RunSummary, Simulation,
    StepOutput,
};

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
            write_line(&mut self.bytes, &EventRecord::new(tick, *event));
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
    let mut sim = Simulation::new(scenario, config)?;
    let mut recorder = TraceRecorder::new(&sim, &config, ticks);

    for _ in 0..ticks {
        recorder.record(&sim.step());
    }

    Ok(recorder.finish(sim.finish()))
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
    use tangle_model::{CompiledScenario, parse_scenario_source};

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
            r#"{"kind":"run","scenario_id":"walking_guide_v1","schema_version":1,"event_version":2,"seed":0,"step_s":0.05,"ticks":250}"#
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
        use tangle_model::{ConflictRegionId, CrossingId, PathId};
        use tangle_sim::{AgentId, AgentMode, ControlTransitionKind, RegionKey, ViolationKind};

        let agent = AgentId::from_index(3);
        let partner = AgentId::from_index(7);
        let cases: [(&str, Event); 10] = [
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
