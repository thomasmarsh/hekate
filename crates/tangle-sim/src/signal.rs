//! Fixed-time signal phase state machine.
//!
//! A compiled [`CompiledSignal`] carries authored phases in cycle order, each
//! with a duration and one display color per signal head. This module advances
//! those phases over simulation time so control can ask what color a movement's
//! head shows.
//!
//! The machine is deliberately time-driven rather than event-driven: a phase
//! boundary is derived from the simulation clock, so pause, frame rate, and
//! host speed cannot shift the cycle. A phase covers the half-open interval
//! `[start_s, start_s + duration_s)`, so a color changes exactly when the
//! simulation time reaches the next phase's `start_s`.
//!
//! Only the phase state machine lives here. Deciding whether a vehicle obeys a
//! red or yellow head is the contextual compliance decision in
//! [`crate::compliance`]; this module only reports the authored color.

use tangle_model::{CompiledScenario, RuleKind, SignalColor, SignalId};

/// Runtime phase state for one compiled signal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SignalRuntime {
    cycle_s: f64,
    elapsed_s: f64,
}

impl SignalRuntime {
    /// A signal starting in its first phase at time zero.
    pub(crate) fn new(cycle_s: f64) -> Self {
        Self {
            cycle_s,
            elapsed_s: 0.0,
        }
    }

    /// Advance the phase clock by one step, wrapping at the cycle length.
    ///
    /// A non-positive cycle length cannot advance, which leaves the signal in
    /// its first phase; validation rejects such a signal before a run.
    pub(crate) fn advance(&mut self, dt: f64) {
        if self.cycle_s > 0.0 {
            self.elapsed_s = (self.elapsed_s + dt) % self.cycle_s;
        }
    }

    /// Time elapsed within the current cycle, in seconds.
    pub(crate) fn elapsed_s(self) -> f64 {
        self.elapsed_s
    }
}

/// Index of the phase active at `elapsed_s` within a signal's cycle.
///
/// Phases are contiguous and ordered by `start_s`. The active phase is the last
/// one whose `start_s` is at or before `elapsed_s`, which makes a boundary tick
/// belong to the later phase and is the deterministic tie-breaker.
pub(crate) fn phase_index_at(
    signal: &tangle_model::CompiledSignal,
    elapsed_s: f64,
) -> Option<usize> {
    let offset = if signal.cycle_s() > 0.0 {
        elapsed_s.rem_euclid(signal.cycle_s())
    } else {
        0.0
    };
    signal
        .phases()
        .iter()
        .enumerate()
        .rev()
        .find(|(_, phase)| phase.start_s() <= offset)
        .map(|(index, _)| index)
}

/// Color shown by one head of `signal` at the given cycle time.
pub(crate) fn head_color(
    signal: &tangle_model::CompiledSignal,
    head_index: usize,
    elapsed_s: f64,
) -> Option<SignalColor> {
    let phase = phase_index_at(signal, elapsed_s)?;
    signal.phases().get(phase)?.color(head_index)
}

/// Map each movement to the signal head that controls it.
///
/// A movement is signal-controlled exactly when a `signal` rule names it and
/// that signal carries a head for it. The first matching rule and head in
/// authored order wins, so the mapping is deterministic. Movements with no
/// signal rule map to `None` and are never held by a signal.
pub(crate) fn movement_signal_map(scenario: &CompiledScenario) -> Vec<Option<(SignalId, usize)>> {
    let mut map = vec![None; scenario.movements().len()];
    for rule in scenario.rules() {
        if rule.kind() != RuleKind::Signal {
            continue;
        }
        let Some(signal_id) = rule.signal() else {
            continue;
        };
        let Some(signal) = scenario.signal(signal_id) else {
            continue;
        };
        let Some(head_index) = signal
            .heads()
            .iter()
            .position(|head| head.movement() == rule.movement())
        else {
            continue;
        };
        let movement = rule.movement().index();
        if movement < map.len() && map[movement].is_none() {
            map[movement] = Some((signal_id, head_index));
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use tangle_model::{CompiledScenario, parse_scenario_source};

    // EW green 10 s, EW yellow 2 s, NS green 10 s, NS yellow 2 s. The cycle is
    // 24 s and the EW head is index 0.
    const SIGNALIZED: &str = r#"
    {
      schema_version: 1,
      id: 'signal_cycle',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'ew', points: [ { x: -40, y: 0 }, { x: 40, y: 0 } ] },
        { id: 'ns', points: [ { x: 0, y: -40 }, { x: 0, y: 40 } ] },
      ],
      portals: [
        { id: 'west', path: 'ew', end: 'start', width_m: 7.0 },
        { id: 'east', path: 'ew', end: 'end', width_m: 7.0 },
        { id: 'south', path: 'ns', end: 'start', width_m: 7.0 },
        { id: 'north', path: 'ns', end: 'end', width_m: 7.0 },
      ],
      movements: [
        { id: 'ew_through', from: 'west', to: 'east', path: 'ew', priority: 0, stop_line_m: 34.0 },
        { id: 'ns_through', from: 'south', to: 'north', path: 'ns', priority: 0, stop_line_m: 34.0 },
      ],
      rules: [
        { id: 'r_ew', movement: 'ew_through', kind: 'signal', signal: 'main' },
        { id: 'r_ns', movement: 'ns_through', kind: 'signal', signal: 'main' },
      ],
      signals: [ { id: 'main',
        heads: [ { id: 'ew', movement: 'ew_through' }, { id: 'ns', movement: 'ns_through' } ],
        phases: [
          { duration_s: 10.0, states: [ { head: 'ew', color: 'green' }, { head: 'ns', color: 'red' } ] },
          { duration_s: 2.0, states: [ { head: 'ew', color: 'yellow' }, { head: 'ns', color: 'red' } ] },
          { duration_s: 10.0, states: [ { head: 'ew', color: 'red' }, { head: 'ns', color: 'green' } ] },
          { duration_s: 2.0, states: [ { head: 'ew', color: 'red' }, { head: 'ns', color: 'yellow' } ] },
        ] } ],
      demand: [
        { id: 'inflow', portal: 'west', rate_vph: 600.0,
          routes: [ { movement: 'ew_through', weight: 1.0 } ] },
      ],
    }
    "#;

    fn scenario() -> CompiledScenario {
        let source = parse_scenario_source(SIGNALIZED).expect("parses");
        CompiledScenario::compile(source).expect("compiles")
    }

    #[test]
    fn phase_boundaries_are_start_inclusive_and_end_exclusive() {
        let scenario = scenario();
        let signal = scenario.signals().first().expect("signal compiles");
        assert_eq!(phase_index_at(signal, 0.0), Some(0));
        assert_eq!(phase_index_at(signal, 9.999), Some(0));
        // Exactly 10 s is the first moment of the yellow phase.
        assert_eq!(phase_index_at(signal, 10.0), Some(1));
        assert_eq!(phase_index_at(signal, 11.999), Some(1));
        assert_eq!(phase_index_at(signal, 12.0), Some(2));
        assert_eq!(phase_index_at(signal, 24.0), Some(0), "wraps at the cycle");
        assert_eq!(phase_index_at(signal, 25.0), Some(0));
    }

    #[test]
    fn head_color_follows_the_authored_phase() {
        let scenario = scenario();
        let signal = scenario.signals().first().expect("signal compiles");
        assert_eq!(head_color(signal, 0, 0.0), Some(SignalColor::Green));
        assert_eq!(head_color(signal, 1, 0.0), Some(SignalColor::Red));
        assert_eq!(head_color(signal, 0, 10.0), Some(SignalColor::Yellow));
        assert_eq!(head_color(signal, 0, 12.0), Some(SignalColor::Red));
        assert_eq!(head_color(signal, 1, 12.0), Some(SignalColor::Green));
        // 34 s wraps to 10 s into the cycle, the first yellow second.
        assert_eq!(head_color(signal, 0, 34.0), Some(SignalColor::Yellow));
    }

    #[test]
    fn the_runtime_advances_and_wraps() {
        let scenario = scenario();
        let signal = scenario.signals().first().expect("signal compiles");
        let mut runtime = SignalRuntime::new(signal.cycle_s());
        assert_eq!(
            head_color(signal, 0, runtime.elapsed_s()),
            Some(SignalColor::Green)
        );
        for _ in 0..200 {
            runtime.advance(0.1);
        }
        // 20 s elapsed: EW is red, NS is green.
        assert!((runtime.elapsed_s() - 20.0).abs() < 1e-9);
        assert_eq!(
            head_color(signal, 0, runtime.elapsed_s()),
            Some(SignalColor::Red)
        );
        // A full cycle later the state has wrapped to the start.
        for _ in 0..40 {
            runtime.advance(0.1);
        }
        assert!(runtime.elapsed_s() < 1e-9);
        assert_eq!(
            head_color(signal, 0, runtime.elapsed_s()),
            Some(SignalColor::Green)
        );
    }
}
