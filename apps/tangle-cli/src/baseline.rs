//! Versioned Phase 1 baseline capture.
//!
//! A baseline freezes the observable behavior of the current kernel before any
//! Phase 2 work so a later change can be classified as intentional or
//! unintended instead of absorbed as unexplained golden-file churn. It records
//! the scenario content hash, the model and event versions, and the canonical
//! trace hash and summary counts at each Phase 1 fidelity preset.
//!
//! The [`Baseline`] manifest is deterministic and safe to check in and compare
//! byte for byte. Wall-clock timing is not reproducible, so it lives in a
//! separate [`PerformanceReport`] that is evidence about a machine, not a gate.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tangle_model::{CompiledScenario, MODEL_VERSION};
use tangle_sim::{EVENT_VERSION, InitError, RunConfig, Seconds, Simulation};

use crate::trace::TraceRecorder;

/// Version of the deterministic baseline manifest format.
pub const BASELINE_VERSION: u32 = 1;

/// Version of the wall-clock performance report format.
pub const PERFORMANCE_REPORT_VERSION: u32 = 1;

/// Failure to capture a baseline.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CaptureError {
    /// The requested simulated duration is not finite and positive.
    #[error("baseline duration must be finite and positive, got {duration_s} s")]
    InvalidDuration {
        /// The rejected duration in seconds.
        duration_s: f64,
    },
    /// A preset's step is longer than the whole run.
    #[error("baseline duration {duration_s} s is shorter than one {step_s} s step")]
    DurationShorterThanStep {
        /// The requested duration in seconds.
        duration_s: f64,
        /// The preset step in seconds.
        step_s: f64,
    },
    /// The kernel rejected a preset run.
    #[error(transparent)]
    Init(#[from] InitError),
}

/// One Phase 1 fidelity preset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    /// Short preset name used as a manifest key.
    pub name: &'static str,
    /// Fixed physics step in seconds.
    pub step_s: f64,
}

/// The three Phase 1 fidelity presets from `PHASE_1_PLAN.md`.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "fast",
        step_s: 0.1,
    },
    Preset {
        name: "standard",
        step_s: 0.05,
    },
    Preset {
        name: "fine",
        step_s: 0.02,
    },
];

/// Provenance of the authored scenario bytes a baseline was captured from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioProvenance {
    /// Authored scenario identifier.
    pub id: String,
    /// Source path as handed to the command, normally repository-relative.
    pub source_path: String,
    /// Schema version the document was authored against.
    pub schema_version: u32,
    /// SHA-256 of the raw source bytes, including comments and whitespace.
    pub content_sha256: String,
}

/// The canonical trace and run counts for one fidelity preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetTrace {
    /// Preset name.
    pub preset: String,
    /// Fixed physics step in seconds.
    pub step_s: f64,
    /// Fixed steps advanced, `round(duration_s / step_s)`.
    pub ticks: u64,
    /// SHA-256 of the canonical JSON Lines trace.
    pub trace_sha256: String,
    /// Canonical trace size in bytes.
    pub trace_bytes: usize,
    /// Agents spawned over the run.
    pub spawned: u64,
    /// Agents despawned over the run.
    pub despawned: u64,
    /// Agents still alive at the end.
    pub remaining: usize,
    /// Simulated duration in seconds.
    pub elapsed_s: f64,
}

/// A derived convergence statement over the captured presets.
///
/// Phase 1's walking skeleton is deliberately trivial, so convergence is about
/// which quantities survive a step change, not about statistical agreement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Convergence {
    /// Whether spawn, despawn, and surviving counts are identical across presets.
    pub invariant_counts: bool,
    /// Number of distinct canonical trace hashes across the presets.
    pub distinct_trace_hashes: usize,
    /// Human-readable summary of what is and is not preset-invariant.
    pub note: String,
}

/// Deterministic Phase 1 baseline manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// Manifest format version.
    pub baseline_version: u32,
    /// Plan phase this baseline belongs to.
    pub phase: String,
    /// Authored scenario provenance.
    pub scenario: ScenarioProvenance,
    /// Model crate version.
    pub model_version: String,
    /// Typed event schema version.
    pub event_version: u32,
    /// Root seed the run used.
    pub seed: u64,
    /// Simulated duration each preset covered, in seconds.
    pub duration_s: f64,
    /// Per-preset trace hashes and run counts.
    pub presets: Vec<PresetTrace>,
    /// Derived convergence statement.
    pub convergence: Convergence,
}

impl Baseline {
    /// Pretty JSON with a trailing newline: the checked-in on-disk form.
    pub fn to_pretty_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).expect("baseline manifest serializes");
        json.push('\n');
        json
    }
}

/// Inputs needed to capture a baseline.
#[derive(Debug, Clone, Copy)]
pub struct CaptureRequest<'a> {
    /// Compiled scenario to run. It is cloned once per preset.
    pub scenario: &'a CompiledScenario,
    /// Repository-relative source path to record.
    pub source_path: &'a str,
    /// SHA-256 of the source bytes to record.
    pub content_sha256: &'a str,
    /// Root seed for every preset.
    pub seed: u64,
    /// Simulated duration every preset must cover, in seconds.
    pub duration_s: f64,
}

/// Hardware identity for a performance report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hardware {
    /// Operating system family.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Logical CPUs available to the process.
    pub logical_cpus: usize,
}

/// Wall-clock measurement for one fidelity preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetPerformance {
    /// Preset name.
    pub preset: String,
    /// Fixed physics step in seconds.
    pub step_s: f64,
    /// Fixed steps advanced.
    pub ticks: u64,
    /// Agents alive at the start of a step, summed over the run.
    pub agent_steps: u64,
    /// Wall seconds spent inside the step-and-record loop.
    pub wall_seconds: f64,
    /// `agent_steps / wall_seconds`, or zero when the loop did not run.
    pub agent_steps_per_second: f64,
}

/// Non-normative wall-clock report captured alongside a baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// Report format version.
    pub report_version: u32,
    /// Capture time as Unix seconds.
    pub captured_unix_s: u64,
    /// `debug` or `release`.
    pub build_profile: String,
    /// Machine identity.
    pub hardware: Hardware,
    /// Per-preset measurements.
    pub presets: Vec<PresetPerformance>,
}

/// Run every fidelity preset over the same simulated duration and capture the
/// deterministic baseline and the point-in-time performance report.
///
/// The duration, not the tick count, is held constant: Fast, Standard, and Fine
/// differ in step size, so fixing ticks would compare different simulated
/// horizons. Each preset rounds the duration to a whole number of steps.
///
/// The kernel is stepped through the same [`TraceRecorder`] the golden trace
/// uses, so the recorded hash is the canonical one, not a second implementation.
pub fn capture(request: CaptureRequest<'_>) -> Result<(Baseline, PerformanceReport), CaptureError> {
    let duration_s = request.duration_s;
    if !duration_s.is_finite() || duration_s <= 0.0 {
        return Err(CaptureError::InvalidDuration { duration_s });
    }

    let mut presets = Vec::with_capacity(PRESETS.len());
    let mut measurements = Vec::with_capacity(PRESETS.len());

    for preset in PRESETS {
        let ticks = (duration_s / preset.step_s).round();
        if ticks < 1.0 {
            return Err(CaptureError::DurationShorterThanStep {
                duration_s,
                step_s: preset.step_s,
            });
        }
        let ticks = ticks as u64;
        let config = RunConfig::new(request.seed).with_step(Seconds::from_secs(preset.step_s));
        let mut sim = Simulation::new(request.scenario.clone(), config)?;
        let mut recorder = TraceRecorder::new(&sim, &config, ticks);

        let mut agent_steps: u64 = 0;
        let start = Instant::now();
        for _ in 0..ticks {
            agent_steps += sim.agent_count() as u64;
            let output = sim.step();
            recorder.record(&output);
        }
        let wall_seconds = start.elapsed().as_secs_f64();

        let summary = sim.finish();
        let spawned = summary.spawned();
        let despawned = summary.despawned();
        let remaining = summary.remaining();
        let elapsed_s = summary.elapsed().as_secs();
        let trace = recorder.finish(summary);

        presets.push(PresetTrace {
            preset: preset.name.to_owned(),
            step_s: preset.step_s,
            ticks,
            trace_sha256: trace.hash().to_owned(),
            trace_bytes: trace.bytes().len(),
            spawned,
            despawned,
            remaining,
            elapsed_s,
        });
        measurements.push(PresetPerformance {
            preset: preset.name.to_owned(),
            step_s: preset.step_s,
            ticks,
            agent_steps,
            wall_seconds,
            agent_steps_per_second: if wall_seconds > 0.0 {
                agent_steps as f64 / wall_seconds
            } else {
                0.0
            },
        });
    }

    let baseline = Baseline {
        baseline_version: BASELINE_VERSION,
        phase: "phase1".to_owned(),
        scenario: ScenarioProvenance {
            id: request.scenario.id().to_owned(),
            source_path: request.source_path.to_owned(),
            schema_version: request.scenario.schema_version(),
            content_sha256: request.content_sha256.to_owned(),
        },
        model_version: MODEL_VERSION.to_owned(),
        event_version: EVENT_VERSION,
        seed: request.seed,
        duration_s,
        convergence: convergence(&presets),
        presets,
    };

    let performance = PerformanceReport {
        report_version: PERFORMANCE_REPORT_VERSION,
        captured_unix_s: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0),
        build_profile: if cfg!(debug_assertions) {
            "debug".to_owned()
        } else {
            "release".to_owned()
        },
        hardware: Hardware {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            logical_cpus: std::thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(1),
        },
        presets: measurements,
    };

    Ok((baseline, performance))
}

fn convergence(presets: &[PresetTrace]) -> Convergence {
    let invariant_counts = presets.windows(2).all(|pair| {
        pair[0].spawned == pair[1].spawned
            && pair[0].despawned == pair[1].despawned
            && pair[0].remaining == pair[1].remaining
    });

    let mut hashes: Vec<&str> = presets
        .iter()
        .map(|preset| preset.trace_sha256.as_str())
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    let distinct_trace_hashes = hashes.len();

    let note = if invariant_counts && distinct_trace_hashes == 1 {
        "All presets agree on counts and canonical trace; the behavior is step-invariant."
    } else if invariant_counts {
        "All presets agree on spawn, despawn, and surviving counts; canonical trace hashes differ because event ticks move with the step."
    } else {
        "Presets disagree on run counts; the behavior is not step-convergent."
    };

    Convergence {
        invariant_counts,
        distinct_trace_hashes,
        note: note.to_owned(),
    }
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
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 60.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] } ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      population: {
        vehicle_count: 6,
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

    fn request<'a>(scenario: &'a CompiledScenario) -> CaptureRequest<'a> {
        CaptureRequest {
            scenario,
            source_path: "scenarios/walking/walking_guide_v1.json5",
            content_sha256: "test-content-hash",
            seed: 0,
            duration_s: 12.5,
        }
    }

    #[test]
    fn presets_match_the_phase_1_plan() {
        assert_eq!(PRESETS[0].step_s, 0.1);
        assert_eq!(PRESETS[1].step_s, 0.05);
        assert_eq!(PRESETS[2].step_s, 0.02);
    }

    #[test]
    fn capture_is_deterministic() {
        let scenario = walking();
        let (first, _) = capture(request(&scenario)).expect("captures");
        let (second, _) = capture(request(&scenario)).expect("captures");
        assert_eq!(first, second);
    }

    #[test]
    fn capture_records_provenance_and_versions() {
        let scenario = walking();
        let (baseline, _) = capture(request(&scenario)).expect("captures");
        assert_eq!(baseline.baseline_version, BASELINE_VERSION);
        assert_eq!(baseline.phase, "phase1");
        assert_eq!(baseline.scenario.id, "walking_guide_v1");
        assert_eq!(baseline.scenario.schema_version, 1);
        assert_eq!(baseline.event_version, EVENT_VERSION);
        assert_eq!(baseline.model_version, MODEL_VERSION);
        assert_eq!(baseline.presets.len(), PRESETS.len());
    }

    #[test]
    fn walking_counts_are_preset_invariant() {
        let scenario = walking();
        let (baseline, _) = capture(request(&scenario)).expect("captures");
        assert!(baseline.convergence.invariant_counts);
        for preset in &baseline.presets {
            assert_eq!(preset.spawned, 6);
            assert_eq!(preset.despawned, 6);
            assert_eq!(preset.remaining, 0);
        }
    }

    #[test]
    fn duration_is_held_constant_across_presets() {
        let scenario = walking();
        let (baseline, _) = capture(request(&scenario)).expect("captures");
        let ticks: Vec<u64> = baseline.presets.iter().map(|preset| preset.ticks).collect();
        // 12.5 s at 0.1 / 0.05 / 0.02 s steps.
        assert_eq!(ticks, vec![125, 250, 625]);
        assert!(
            baseline
                .presets
                .iter()
                .all(|preset| (preset.elapsed_s - 12.5).abs() < 0.1)
        );
    }

    #[test]
    fn standard_preset_matches_the_canonical_golden_duration() {
        let scenario = walking();
        let (baseline, _) = capture(request(&scenario)).expect("captures");
        let standard = baseline
            .presets
            .iter()
            .find(|preset| preset.preset == "standard")
            .expect("standard preset exists");
        // The checked-in golden trace runs 250 Standard steps.
        assert_eq!(standard.ticks, 250);
    }

    #[test]
    fn rejects_a_non_positive_duration() {
        let scenario = walking();
        let error = capture(CaptureRequest {
            duration_s: 0.0,
            ..request(&scenario)
        })
        .expect_err("zero duration must be rejected");
        assert_eq!(error, CaptureError::InvalidDuration { duration_s: 0.0 });
    }

    #[test]
    fn performance_report_names_the_machine_and_profiles() {
        let scenario = walking();
        let (_, performance) = capture(request(&scenario)).expect("captures");
        assert_eq!(performance.report_version, PERFORMANCE_REPORT_VERSION);
        assert!(!performance.hardware.os.is_empty());
        assert!(!performance.hardware.arch.is_empty());
        assert!(matches!(
            performance.build_profile.as_str(),
            "debug" | "release"
        ));
        assert_eq!(performance.presets.len(), PRESETS.len());
        for measurement in &performance.presets {
            assert!(measurement.ticks >= 1);
            assert!(measurement.wall_seconds >= 0.0);
            assert!(measurement.agent_steps_per_second >= 0.0);
        }
    }
}
