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

use std::fmt::Write as _;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tangle_model::CompiledScenario;
use tangle_sim::{DespawnReason, Event, InitError, RunConfig, RunSummary, Simulation, StepOutput};

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
                seed: config.seed(),
                step_s: config.step().as_secs(),
                ticks,
            },
        );
        Self { bytes }
    }

    /// Record one completed step's events, attributed to its tick.
    ///
    /// Events are recorded in kernel emission order: by ascending step, and
    /// within a step by ascending agent index. Every event a step produces is
    /// attributed to the tick that step completes.
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
/// Events are recorded in kernel emission order: by ascending step, and within
/// a step by ascending agent index. Every event a step produces is attributed
/// to the tick that step completes.
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

fn sha256_hex(bytes: &[u8]) -> String {
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
    seed: u64,
    step_s: f64,
    ticks: u64,
}

/// One typed event, optionally carrying the fields that variant owns.
#[derive(Serialize)]
struct EventRecord {
    kind: &'static str,
    tick: u64,
    event: &'static str,
    agent: u32,
    path: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
}

impl EventRecord {
    fn new(tick: u64, event: Event) -> Self {
        match event {
            Event::Spawned {
                agent,
                path,
                distance_m,
            } => Self {
                kind: "event",
                tick,
                event: "spawned",
                agent: agent.get(),
                path: path.get(),
                distance_m: Some(distance_m),
                reason: None,
            },
            Event::Despawned {
                agent,
                path,
                reason,
            } => Self {
                kind: "event",
                tick,
                event: "despawned",
                agent: agent.get(),
                path: path.get(),
                distance_m: None,
                reason: Some(match reason {
                    DespawnReason::ExitedPath => "exited_path",
                }),
            },
        }
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
            r#"{"kind":"run","scenario_id":"walking_guide_v1","schema_version":1,"seed":0,"step_s":0.05,"ticks":250}"#
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
            r#"{"kind":"event","tick":1,"event":"spawned","agent":0,"path":0,"distance_m":0.0}"#
        ));
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
}
