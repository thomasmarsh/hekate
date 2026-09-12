//! Presentation-time clock and playback controls.
//!
//! The kernel owns the authoritative clock: it advances exactly one configured
//! step per [`tangle_sim::Simulation::step`] call and never reads wall-clock
//! time. The presentation layer only decides *how many whole steps* to take
//! from the real time that elapsed between frames. Pause, single-step, speed,
//! and frame-rate controls therefore choose which ticks are observed; they
//! cannot change what those ticks contain or the canonical trace those ticks
//! produce.
//!
//! This module is deliberately free of Bevy and terminal types so the playback
//! contract can be tested headlessly, without a window or a GPU.

/// Playback speed selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Speed {
    /// Real time: one simulated second per wall-clock second.
    #[default]
    Real,
    /// Four simulated seconds per wall-clock second.
    Fast,
    /// Drain as many whole steps as the per-frame cap allows.
    Maximum,
}

impl Speed {
    /// Simulation nanoseconds per wall-clock nanosecond, or `None` for
    /// unbounded. Integer factors keep `1x` and `4x` exact under a nanosecond
    /// accumulator.
    pub const fn factor(self) -> Option<u64> {
        match self {
            Self::Real => Some(1),
            Self::Fast => Some(4),
            Self::Maximum => None,
        }
    }

    /// Short label for the status strip.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Real => "1x",
            Self::Fast => "4x",
            Self::Maximum => "max",
        }
    }
}

/// Upper bound on whole steps taken in a single frame.
///
/// A long stall on a slow host must not make the renderer try to catch up with
/// an unbounded backlog, so the clock caps a frame and discards the remainder.
/// The cap only limits presentation smoothness; it never changes the kernel.
pub const MAX_TICKS_PER_FRAME: u64 = 240;

/// Accumulates real presentation time and yields whole simulation steps.
///
/// Time is tracked in integer nanoseconds so that `1x` and `4x` playback do not
/// drift when a frame delta is an inexact binary fraction of a second. That
/// keeps tick counts independent of how wall time is split into frames.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationClock {
    step_nanos: u64,
    speed: Speed,
    paused: bool,
    accumulated_nanos: u64,
    queued_ticks: u64,
    total_ticks: u64,
}

/// Nanoseconds in one second, for converting presentation time to integers.
const NANOS_PER_SEC: f64 = 1_000_000_000.0;

impl PresentationClock {
    /// Create a clock for a kernel whose fixed step is `step_secs` seconds.
    ///
    /// # Panics
    ///
    /// Panics if `step_secs` is not finite and positive, or rounds to zero
    /// nanoseconds. The kernel already rejects such a step, so reaching here
    /// with one is a programming error.
    pub fn new(step_secs: f64) -> Self {
        assert!(
            step_secs.is_finite() && step_secs > 0.0,
            "presentation step must be finite and positive, got {step_secs}"
        );
        let step_nanos = seconds_to_nanos(step_secs);
        assert!(
            step_nanos > 0,
            "presentation step rounds to zero nanoseconds"
        );
        Self {
            step_nanos,
            speed: Speed::default(),
            paused: false,
            accumulated_nanos: 0,
            queued_ticks: 0,
            total_ticks: 0,
        }
    }

    /// Current playback speed.
    pub const fn speed(&self) -> Speed {
        self.speed
    }

    /// Select the playback speed.
    pub fn set_speed(&mut self, speed: Speed) {
        self.speed = speed;
    }

    /// Whether the clock currently yields no time-based steps.
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set the pause state. Unpausing keeps the accumulated remainder.
    pub fn set_paused(&mut self, paused: bool) {
        if paused && !self.paused {
            // Pausing discards sub-step time so resuming cannot take a
            // surprise step from time accumulated before the pause.
            self.accumulated_nanos = 0;
        }
        self.paused = paused;
    }

    /// Toggle pause.
    pub fn toggle_paused(&mut self) {
        self.set_paused(!self.paused);
    }

    /// Queue exactly one step, taken on the next [`Self::advance`].
    ///
    /// Single-step is meaningful while paused, but is also honoured while
    /// playing so a user can nudge the run without changing speed.
    pub fn request_single_tick(&mut self) {
        self.queued_ticks += 1;
    }

    /// Fractional progress toward the next step, in `[0, 1]`.
    ///
    /// The renderer uses this to interpolate between the previous and current
    /// snapshots. It is presentation-only and never feeds back into the kernel.
    pub fn alpha(&self) -> f64 {
        (self.accumulated_nanos as f64 / self.step_nanos as f64).clamp(0.0, 1.0)
    }

    /// Whole steps taken since construction.
    pub const fn total_ticks(&self) -> u64 {
        self.total_ticks
    }

    /// Consume `frame_secs` of real presentation time and return the number of
    /// whole simulation steps the caller must take now.
    ///
    /// Non-finite or negative frame durations are treated as zero.
    pub fn advance(&mut self, frame_secs: f64) -> u64 {
        let queued = std::mem::take(&mut self.queued_ticks);

        let ticks = if self.paused {
            self.accumulated_nanos = 0;
            queued
        } else {
            let timed = match self.speed.factor() {
                Some(factor) => {
                    let frame_nanos = seconds_to_nanos(frame_secs);
                    self.accumulated_nanos = self
                        .accumulated_nanos
                        .saturating_add(frame_nanos.saturating_mul(factor));
                    let due = self.accumulated_nanos / self.step_nanos;
                    if due > MAX_TICKS_PER_FRAME {
                        self.accumulated_nanos = 0;
                        MAX_TICKS_PER_FRAME
                    } else {
                        self.accumulated_nanos -= due * self.step_nanos;
                        due
                    }
                }
                // Unbounded speed still advances in whole steps, capped per
                // frame so a stalled host cannot spin on an unbounded backlog.
                None => {
                    self.accumulated_nanos = 0;
                    MAX_TICKS_PER_FRAME
                }
            };
            timed + queued
        };

        self.total_ticks += ticks;
        ticks
    }
}

/// Convert a presentation duration to whole nanoseconds, clamping non-finite
/// and non-positive inputs to zero.
fn seconds_to_nanos(secs: f64) -> u64 {
    if !secs.is_finite() || secs <= 0.0 {
        return 0;
    }
    (secs * NANOS_PER_SEC).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tangle_model::CompiledScenario;
    use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

    const STEP: f64 = 0.05;

    fn scenario() -> CompiledScenario {
        let source = tangle_model::parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 60, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 3.5 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 3.5 } ], \
             population: { vehicle_count: 6, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    fn snapshots(seed: u64, ticks: u64) -> Vec<String> {
        let mut sim = Simulation::new(scenario(), RunConfig::new(seed)).expect("builds");
        let mut frames = Vec::with_capacity(ticks as usize);
        frames.push(format!("{:?}", sim.snapshot(SnapshotDetail::Full)));
        for _ in 0..ticks {
            sim.step();
            frames.push(format!("{:?}", sim.snapshot(SnapshotDetail::Full)));
        }
        frames
    }

    /// Drive `sim` with `clock` over `frames`, asserting that every frame
    /// boundary lands on the matching state in `direct` (indexed by completed
    /// tick), and return the tick reached at the end.
    fn drive(
        sim: &mut Simulation,
        clock: &mut PresentationClock,
        frames: &[f64],
        direct: &[String],
    ) -> u64 {
        let mut tick = sim.time().tick();
        assert_eq!(
            format!("{:?}", sim.snapshot(SnapshotDetail::Full)),
            direct[tick as usize]
        );
        for &frame_secs in frames {
            for _ in 0..clock.advance(frame_secs) {
                sim.step();
                tick += 1;
            }
            assert_eq!(
                format!("{:?}", sim.snapshot(SnapshotDetail::Full)),
                direct[tick as usize],
                "state after {tick} steps diverged"
            );
        }
        tick
    }

    fn total_ticks(speed: Speed, frames: &[f64]) -> u64 {
        let mut clock = PresentationClock::new(STEP);
        clock.set_speed(speed);
        frames.iter().map(|&f| clock.advance(f)).sum()
    }

    #[test]
    fn real_time_takes_one_step_per_step_duration() {
        let mut clock = PresentationClock::new(STEP);
        assert_eq!(clock.advance(STEP), 1);
        assert_eq!(clock.advance(STEP * 3.0), 3);
        assert_eq!(clock.advance(STEP / 2.0), 0);
        // The half step is retained and completes on the next frame.
        assert_eq!(clock.advance(STEP / 2.0), 1);
        assert_eq!(clock.total_ticks(), 5);
    }

    #[test]
    fn fast_speed_scales_simulated_time_by_four() {
        let mut clock = PresentationClock::new(STEP);
        clock.set_speed(Speed::Fast);
        assert_eq!(clock.advance(STEP), 4);
    }

    #[test]
    fn frame_partitioning_does_not_change_whole_tick_count() {
        // One second expressed as many small frames, one large frame, and an
        // irregular partition must all buy the same whole steps.
        let total = 1.0;
        let one_frame = [total];
        let many_frames: Vec<f64> = std::iter::repeat_n(0.01, 100).collect();
        let irregular: Vec<f64> = vec![0.17, 0.03, 0.22, 0.05, 0.31, 0.001, 0.219];

        for speed in [Speed::Real, Speed::Fast] {
            let expected = total_ticks(speed, &one_frame);
            assert_eq!(total_ticks(speed, &many_frames), expected);
            assert_eq!(total_ticks(speed, &irregular), expected);
        }
    }

    #[test]
    fn a_stalled_frame_is_capped_without_banking_a_backlog() {
        let mut clock = PresentationClock::new(STEP);
        clock.set_speed(Speed::Real);
        // A twenty second stall would be 400 steps, but the per-frame cap wins
        // and the remainder is dropped rather than paid back next frame.
        assert_eq!(clock.advance(20.0), MAX_TICKS_PER_FRAME);
        assert_eq!(clock.advance(STEP), 1);
    }

    #[test]
    fn pause_contributes_no_time_based_ticks() {
        let mut clock = PresentationClock::new(STEP);
        clock.set_paused(true);
        assert_eq!(clock.advance(100.0), 0);
        clock.set_paused(false);
        assert_eq!(clock.advance(STEP), 1);
    }

    #[test]
    fn single_step_queues_exactly_one_tick_while_paused() {
        let mut clock = PresentationClock::new(STEP);
        clock.set_paused(true);
        clock.request_single_tick();
        clock.request_single_tick();
        assert_eq!(clock.advance(100.0), 2);
        assert_eq!(clock.advance(100.0), 0);
        assert_eq!(clock.total_ticks(), 2);
    }

    #[test]
    fn maximum_speed_respects_the_per_frame_cap() {
        let mut clock = PresentationClock::new(STEP);
        clock.set_speed(Speed::Maximum);
        assert_eq!(clock.advance(0.0), MAX_TICKS_PER_FRAME);
    }

    #[test]
    fn clock_driven_run_matches_direct_stepping_at_every_observed_tick() {
        // `direct[tick]` is the kernel state after exactly `tick` steps. Every
        // frame boundary in a clock-driven run must land on the matching state
        // no matter how wall time was partitioned.
        let direct = snapshots(7, 20);
        let mut sim = Simulation::new(scenario(), RunConfig::new(7)).expect("builds");
        let mut clock = PresentationClock::new(STEP);
        clock.set_speed(Speed::Real);
        assert_eq!(drive(&mut sim, &mut clock, &[0.1, 0.3, 0.6], &direct), 20);
    }

    #[test]
    fn pausing_does_not_change_states_reached_for_the_same_active_time() {
        let direct = snapshots(7, 20);

        // Unpaused: 1.0 s of active time takes 20 steps.
        let mut sim = Simulation::new(scenario(), RunConfig::new(7)).expect("builds");
        let mut running = PresentationClock::new(STEP);
        running.set_speed(Speed::Real);
        assert_eq!(drive(&mut sim, &mut running, &[1.0], &direct), 20);

        // Paused across a long interval, then resumed for the same active
        // time, reaches exactly the same states.
        let mut sim = Simulation::new(scenario(), RunConfig::new(7)).expect("builds");
        let mut paused = PresentationClock::new(STEP);
        paused.set_speed(Speed::Real);
        paused.set_paused(true);
        assert_eq!(drive(&mut sim, &mut paused, &[5.0, 0.0], &direct), 0);
        paused.set_paused(false);
        assert_eq!(drive(&mut sim, &mut paused, &[1.0], &direct), 20);
    }

    #[test]
    fn interpolation_alpha_stays_in_unit_range() {
        let mut clock = PresentationClock::new(STEP);
        clock.set_speed(Speed::Fast);
        let _ = clock.advance(0.03);
        let alpha = clock.alpha();
        assert!((0.0..=1.0).contains(&alpha));
    }

    #[test]
    fn non_finite_frame_duration_takes_no_time_based_step() {
        let mut clock = PresentationClock::new(STEP);
        assert_eq!(clock.advance(f64::NAN), 0);
        assert_eq!(clock.advance(f64::INFINITY), 0);
        assert_eq!(clock.advance(-1.0), 0);
    }
}
