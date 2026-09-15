//! Accepted residual: the emergency anti-overlap position backstop during
//! low-speed red-queue close-up.
//!
//! `four_leg_signal_v1` seed 0 forms a red-light queue with a stopped leader.
//! The kernel's last-resort position cap (`new_speed = min(new_speed, gap / dt)`)
//! is a position clamp with no explicit acceleration limit, so a centimetre-scale
//! gap at a queue close-up implies an unbounded single-step deceleration. Round 1
//! measured the worst step at −28.2549 m/s² (tick 1221, agent 24, comfortable
//! brake 2.256, body 4.99 m) against 11 engaged cap steps; the engagements at
//! ticks 1137, 1138, 1211, 1212, 1213, 1214, 1220, 1221, 1222, 1223, and 1224
//! are all low-speed queue close-ups, not a high-speed emergency brake.
//!
//! `TAS-029-bound-red-queue-emergency-cap` accepts that residual as the backstop
//! ceiling and keeps it under test: the measure below fails if the accepted bound
//! is exceeded, and the controlled `car_following_v1` benchmark must not engage
//! the cap at all.
//!
//! Run with `scripts/run-test-harness.sh`.

use std::collections::HashMap;
use std::path::PathBuf;

use hekate_cli::load_scenario;
use hekate_sim::{AgentId, RunConfig, Simulation, SnapshotDetail};

/// Accepted worst one-step deceleration the emergency anti-overlap position cap
/// may command during low-speed red-queue close-up.
///
/// This is a backstop ceiling, not a comfort target. Round 1 measured
/// −28.2549 m/s² at tick 1221 for agent 24 in `four_leg_signal_v1` seed 0; the
/// accepted bound sits above that so ordinary measurement variation does not
/// fail the regression, while a materially harder step does. The nominal
/// following regime stays bounded by the profile's comfortable deceleration —
/// `car_following_v1` must not engage the cap at all.
const ACCEPTED_CAP_STEP_MPS2: f64 = 32.0;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Outcome of one benchmark run: the worst one-step deceleration with the tick
/// and agent that produced it, plus the emergency-cap engagements.
struct CapRun {
    worst_accel_mps2: f64,
    worst_tick: usize,
    worst_agent: AgentId,
    /// Whether the worst step's own tick engaged the emergency cap.
    worst_tick_engaged_cap: bool,
    /// The run's total emergency-cap step count.
    cap_steps: u64,
    /// Every tick whose step engaged the cap, in order.
    cap_ticks: Vec<usize>,
}

/// Run one benchmark at seed 0 and report its worst one-step deceleration, the
/// tick and agent that produced it, the run's emergency-cap step count, and the
/// complete list of ticks that engaged the cap.
fn worst_step(scenario: &str, ticks: usize) -> CapRun {
    let path = repo_path(&format!("scenarios/benchmarks/{scenario}.json5"));
    let loaded = load_scenario(&path)
        .unwrap_or_else(|error| panic!("scenario '{}' failed to load: {error}", path.display()));
    let mut sim = Simulation::new(loaded, RunConfig::new(0)).expect("benchmark runs");
    let step = sim.config().step().as_secs();
    let mut previous: HashMap<u32, f64> = HashMap::new();
    let mut worst = f64::INFINITY;
    let mut worst_tick = 0usize;
    let mut worst_agent = AgentId::from_index(0);
    let mut worst_tick_engaged_cap = false;
    let mut cap_ticks: Vec<usize> = Vec::new();
    for tick in 0..ticks {
        let caps_before = sim.emergency_cap_steps();
        sim.step();
        let mut worst_recorded_this_tick = false;
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let motion = sample.motion.as_ref().expect("full detail");
            if let Some(previous_speed) = previous.insert(sample.id.get(), motion.speed_mps) {
                let accel = (motion.speed_mps - previous_speed) / step;
                if accel < worst {
                    worst = accel;
                    worst_tick = tick + 1;
                    worst_agent = sample.id;
                    worst_recorded_this_tick = true;
                }
            }
        }
        let caps_after = sim.emergency_cap_steps();
        let engaged = caps_after > caps_before;
        if engaged {
            cap_ticks.push(tick + 1);
        }
        if worst_recorded_this_tick {
            worst_tick_engaged_cap = engaged;
        }
    }
    CapRun {
        worst_accel_mps2: worst,
        worst_tick,
        worst_agent,
        worst_tick_engaged_cap,
        cap_steps: sim.emergency_cap_steps(),
        cap_ticks,
    }
}

/// The red-light queue engages the emergency position cap, and the worst step it
/// commands stays under the bound accepted for that backstop.
#[test]
#[ignore = "slow: long-horizon signalized queue-formation measurement; run scripts/run-test-harness.sh"]
fn red_queue_close_up_stays_below_the_accepted_emergency_cap_step() {
    let red_queue = worst_step("four_leg_signal_v1", 4000);
    println!(
        "four_leg_signal_v1 seed 0: worst step {:.4} m/s^2 at tick {}, agent {} \
         (emergency cap steps {}, cap ticks {:?})",
        red_queue.worst_accel_mps2,
        red_queue.worst_tick,
        red_queue.worst_agent.get(),
        red_queue.cap_steps,
        red_queue.cap_ticks,
    );
    assert!(
        red_queue.cap_steps >= 1,
        "the signalized queue must reproduce the emergency-cap engagement, found {}; \
         no engagement means the accepted ceiling is obsolete and this test and the \
         known-limitations Motion bullet must be re-scoped",
        red_queue.cap_steps
    );
    assert!(
        red_queue.worst_tick_engaged_cap,
        "the worst step {:.4} m/s^2 at tick {} (agent {}) did not come from a \
         cap-engaged tick, so it is not the backstop residual this bound accepts",
        red_queue.worst_accel_mps2,
        red_queue.worst_tick,
        red_queue.worst_agent.get()
    );
    assert!(
        red_queue.worst_accel_mps2 >= -ACCEPTED_CAP_STEP_MPS2,
        "worst step {:.4} m/s^2 at tick {}, agent {} exceeded the accepted \
         backstop ceiling {ACCEPTED_CAP_STEP_MPS2} m/s^2",
        red_queue.worst_accel_mps2,
        red_queue.worst_tick,
        red_queue.worst_agent.get()
    );

    // The residual is scoped to signalized queue formation: the controlled
    // car-following benchmark must stay inside the profile bounds entirely.
    let car_following = worst_step("car_following_v1", 4000);
    assert_eq!(
        car_following.cap_steps, 0,
        "car_following_v1 seed 0 engaged the emergency position cap"
    );
}
