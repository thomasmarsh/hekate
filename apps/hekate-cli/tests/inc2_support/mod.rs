//! Shared support for the Increment 2 reproducibility and golden suites.
//!
//! `inc2_determinism.rs` and `inc2_trace.rs` prove the same things about the same
//! checked-in fixtures, so the fixture table, the run loop that drives each
//! fixture's maneuver, and the occurrence predicate that says the maneuver
//! happened live here rather than being spelled twice.
//!
//! ## Two kinds of recorded run
//!
//! Three of the six fixtures decide their maneuver with the kernel's own tactic,
//! so a plain [`canonical_trace`] run — byte for byte the trace `hekate-cli run`
//! writes — carries the whole lifecycle. The other three need a tactical leaf's
//! request before any maneuver exists: the two change-of-lane fixtures record
//! the request through [`Simulation::request_lateral_maneuver`], and the
//! wrong-way fixture records the entry through
//! [`Simulation::request_wrong_way_entry`]. **No production caller supplies
//! those requests** — no CLI command exposes either seam, so `hekate-cli run`
//! cannot produce a lane change or an opposing traversal. Those three runs are
//! therefore driven in-process here through [`TraceRecorder`], which the trace
//! contract documents as producing exactly the canonical trace bytes and hash of
//! the direct run. [`drive`] uses `TraceRecorder` for every fixture, and
//! `inc2_determinism.rs` asserts that for the three autonomous fixtures those
//! bytes equal [`canonical_trace`]'s.
//!
//! ## Per-preset occurrence
//!
//! The `CC-OVERTAKE`/`CC-OPPOSE` cells are judged at the *Standard* and *Fine*
//! presets, and the two presets step the same scenario on different tick grids.
//! Arrivals are a per-tick Bernoulli thinning of a Poisson process, so a fixture
//! authored at one grid realises a different arrival series at the other: the
//! benchmark seed a fixture is pinned to at Standard does not by itself put the
//! intended pair on the facility at Fine. Each fixture in [`FIXTURES`] therefore
//! names a *pinned bank seed*, and [`maneuver_occurs`] is the predicate that
//! seed satisfies at both presets. `scenarios/phase2/inc2/inc2_seed_bank.json`
//! declares those seeds, and `inc2_determinism.rs` re-proves the occurrence at
//! every preset before comparing hashes, so reproduction is reproduction of a
//! maneuver rather than of an empty run.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use hekate_cli::{Trace, TraceRecorder, canonical_trace, fidelity_ticks, load_scenario_provenance};
use hekate_model::{ClearanceBandId, CompiledScenario, FacilityId, PathId};
use hekate_sim::{
    AgentId, DespawnReason, Event, LateralManeuverRequest, ManeuverEdge, ManeuverReason,
    ManeuverState, RunConfig, Seconds, Simulation, SnapshotDetail,
};

/// The two Phase 2 fidelity presets a `CC-OVERTAKE`/`CC-OPPOSE` cell is judged
/// at: Standard 50 ms and Fine 20 ms. Fast is not a required preset for these
/// cells, so it is not covered here.
pub const PRESETS: [(&str, f64); 2] = [("standard", 0.05), ("fine", 0.02)];

/// The Standard preset's fixed step: the step `hekate-cli run` and
/// `hekate-cli batch` always use.
pub const STANDARD_STEP_S: f64 = PRESETS[0].1;

/// The checked-in, declared Increment 2 seed bank.
pub const SEED_BANK: &str = "scenarios/phase2/inc2/inc2_seed_bank.json";

/// How the fixture's maneuver is produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Plan {
    /// No request: the fixture's own demand and tactics decide the maneuver.
    Autonomous,
    /// The change of lane the tactical leaf records for the follower once the
    /// leader it displaces around is within `window_m` metres.
    LaneChange {
        /// Dense index of the adjacent facility the change targets.
        target_index: usize,
        /// The follower-to-leader gap, in metres, that opens the request.
        window_m: (f64, f64),
    },
    /// The wrong-way entry the tactical leaf records on each corridor, exactly
    /// as the landed `crates/hekate-sim/tests/wrong_way.rs` driver records it.
    WrongWay,
}

/// What the run must contain for the fixture's intended maneuver to have
/// happened at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expectation {
    /// Ordinary within-facility overtaking: at least one overtaking interval
    /// whose passing agent recorded the whole maneuver lifecycle, with both
    /// participants admitted and route-complete and no contact anywhere in the
    /// run. `violating_band` additionally requires the unsafe variant's authored
    /// violation the fixture exists to record.
    Overtake {
        /// Whether the pass must fall inside the fixture's violation band.
        violating_band: bool,
    },
    /// A recorded cross-facility change of lane: the request must be admissible,
    /// the lifecycle must run, and the run must record exactly `handoffs`
    /// facility crossings. `prevented` is the prohibited-boundary variant, whose
    /// crossing is refused with the contract's reason instead.
    LaneChange {
        /// Facilities the run crosses.
        handoffs: usize,
        /// Whether the crossing is prevented rather than performed.
        prevented: bool,
    },
    /// The wrong-way lifecycle: all four corridors reach their decision and the
    /// occupied corridor reaches its head-on contact.
    WrongWay,
}

/// One checked-in Increment 2 fixture, the way its maneuver is produced, and the
/// declared bank seed whose arrivals admit that maneuver at both required
/// presets.
#[derive(Debug, Clone, Copy)]
pub struct Fixture {
    /// The fixture's scenario id, which names its golden files.
    pub id: &'static str,
    /// Repository-relative path to the checked-in scenario source.
    pub path: &'static str,
    /// How this fixture produces its maneuver.
    pub plan: Plan,
    /// What the produced run must contain.
    pub expectation: Expectation,
    /// Whole steps the fixture's declared Standard run advances.
    pub standard_ticks: u64,
    /// The declared bank seed pinned for this fixture.
    pub pinned_seed: u64,
}

/// The wrong-way fixture's four corridors, in the fixture's own `a`, `b`, `c`,
/// `d` order: the permitted, prohibited-but-connected, disconnected, and
/// occupied-opposing cases.
pub const WRONG_WAY_CORRIDORS: [&str; 4] = ["guide_a", "guide_b", "guide_c", "guide_d"];

/// Every checked-in Increment 2 fixture, at the horizon its own suite declares.
///
/// `motor_lane_change_boundary_v2` and the wrong-way fixture need the longer
/// horizon the lane-change and wrong-way suites use: 2500 steps for the change
/// of lane to cross out and back, and the wrong-way fixture's disconnected
/// corridor to admit its rider (tick 3674 at Standard).
pub const FIXTURES: [Fixture; 6] = [
    Fixture {
        id: "narrow_passing_v2",
        path: "scenarios/phase2/inc2/narrow_passing_v2.json5",
        plan: Plan::Autonomous,
        expectation: Expectation::Overtake {
            violating_band: false,
        },
        standard_ticks: 1900,
        pinned_seed: 102,
    },
    Fixture {
        id: "motor_passing_narrow_v2",
        path: "scenarios/phase2/inc2/motor_passing_narrow_v2.json5",
        plan: Plan::Autonomous,
        expectation: Expectation::Overtake {
            violating_band: false,
        },
        standard_ticks: 1900,
        pinned_seed: 102,
    },
    Fixture {
        id: "motor_lane_change_v2",
        path: "scenarios/phase2/inc2/motor_lane_change_v2.json5",
        plan: Plan::LaneChange {
            target_index: 1,
            window_m: (10.0, 11.5),
        },
        expectation: Expectation::LaneChange {
            handoffs: 2,
            prevented: false,
        },
        standard_ticks: 2500,
        pinned_seed: 48,
    },
    Fixture {
        id: "narrow_passing_unsafe_v2",
        path: "scenarios/phase2/inc2/narrow_passing_unsafe_v2.json5",
        plan: Plan::Autonomous,
        expectation: Expectation::Overtake {
            violating_band: true,
        },
        standard_ticks: 1900,
        pinned_seed: 102,
    },
    Fixture {
        id: "motor_lane_change_boundary_v2",
        path: "scenarios/phase2/inc2/motor_lane_change_boundary_v2.json5",
        plan: Plan::LaneChange {
            target_index: 1,
            window_m: (9.0, 10.5),
        },
        expectation: Expectation::LaneChange {
            handoffs: 0,
            prevented: true,
        },
        standard_ticks: 2500,
        pinned_seed: 102,
    },
    Fixture {
        id: "narrow_wrong_way_v2",
        path: "scenarios/phase2/inc2/narrow_wrong_way_v2.json5",
        plan: Plan::WrongWay,
        expectation: Expectation::WrongWay,
        standard_ticks: 6000,
        pinned_seed: 11,
    },
];

// The determinism and golden suites hand-maintain one `#[test]` per
// fixture+preset, so growing either table without adding its cases would
// silently leave the new entry untested. These guards turn that into a build
// failure that points the author at the per-case lists in `inc2_determinism.rs`
// and `inc2_trace.rs`.
const _: () = assert!(
    FIXTURES.len() == 6,
    "a new fixture needs its per-case tests added to inc2_determinism.rs and inc2_trace.rs"
);
const _: () = assert!(
    PRESETS.len() == 2,
    "a new preset needs its per-case tests added to inc2_determinism.rs and inc2_trace.rs"
);

/// A repository-root-relative path resolved against the test binary's crate.
pub fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The fixed step configuration one run uses.
pub fn config(seed: u64, step_s: f64) -> RunConfig {
    RunConfig::new(seed).with_step(Seconds::from_secs(step_s))
}

/// The whole steps a preset runs so its horizon matches `standard_ticks` steps
/// of the Standard preset's step.
pub fn ticks_at(standard_ticks: u64, step_s: f64) -> u64 {
    fidelity_ticks(step_s, standard_ticks)
}

/// Load a fixture's compiled scenario from its checked-in source.
pub fn load(fixture: &Fixture) -> CompiledScenario {
    load_scenario_provenance(&repo_path(fixture.path))
        .unwrap_or_else(|error| panic!("'{}' loads: {error}", fixture.path))
        .0
}

/// The canonical trace of one fixture run at one seed and preset, produced by
/// the driver that supplies the fixture's maneuver requests.
pub fn run(fixture: &Fixture, seed: u64, step_s: f64) -> Run {
    drive(
        fixture,
        seed,
        step_s,
        ticks_at(fixture.standard_ticks, step_s),
    )
}

/// The canonical trace of one fixture run at one seed, preset, and tick count.
///
/// The trace bytes are the ones [`canonical_trace`] produces for a scenario that
/// needs no request: the run loop is the same loop driven through
/// [`TraceRecorder`], with only the fixture's own tactical-leaf request inserted
/// before the steps that need it.
pub fn drive(fixture: &Fixture, seed: u64, step_s: f64, ticks: u64) -> Run {
    let scenario = load(fixture);
    let config = config(seed, step_s);
    let mut sim = Simulation::new(scenario, config)
        .unwrap_or_else(|error| panic!("'{}' builds: {error}", fixture.path));
    let mut recorder = TraceRecorder::new(&sim, &config, ticks);
    let mut observation = Observation::default();
    // Only the wrong-way plan names the fixture's corridors, so only that plan
    // builds the corridor driver.
    let mut wrong_way =
        matches!(fixture.plan, Plan::WrongWay).then(|| WrongWayDriver::new(sim.scenario()));

    for _ in 0..ticks {
        match fixture.plan {
            Plan::Autonomous => {}
            Plan::LaneChange {
                target_index,
                window_m,
            } => {
                if observation.request.is_none() {
                    observation.request = request_lane_change(&mut sim, target_index, window_m);
                }
            }
            Plan::WrongWay => wrong_way
                .as_mut()
                .expect("the wrong-way plan builds the corridor driver")
                .request(&mut sim, &mut observation),
        }

        let (tick, events, transitions) = {
            let output = sim.step();
            recorder.record(&output);
            (
                output.time().tick(),
                output.events().to_vec(),
                output
                    .transitions()
                    .iter()
                    .map(|transition| {
                        (
                            transition.agent,
                            transition.from,
                            transition.to,
                            transition.edge,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        };
        observation.record(&sim, tick, &events, transitions);
    }

    // The declared run ends at a close boundary: an interval still open at the
    // last tick is observed as closed, exactly as `canonical_run_captured`
    // observes it. The closure emits no event, so the trace bytes are the
    // canonical ones.
    sim.close_open_close_passes();
    sim.close_open_opposing_traversals();
    observation.record_overtakes(&sim);
    let trace = recorder.finish(sim.finish());
    Run { trace, observation }
}

/// The canonical trace of one fixture at one seed and preset, through the direct
/// entry point, for the fixtures whose maneuver needs no request.
pub fn canonical(fixture: &Fixture, seed: u64, step_s: f64) -> Trace {
    canonical_trace(
        load(fixture),
        config(seed, step_s),
        ticks_at(fixture.standard_ticks, step_s),
    )
    .unwrap_or_else(|error| panic!("'{}' runs: {error}", fixture.path))
}

/// One fixture run: its canonical trace and what the run loop observed.
pub struct Run {
    /// The canonical trace bytes and hash.
    pub trace: Trace,
    /// The maneuver, interval, and lifecycle observations.
    pub observation: Observation,
}

/// The maneuver request one run recorded, if any.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    /// The agent that asked.
    pub agent: AgentId,
    /// The body it displaces around.
    pub passed: AgentId,
    /// Whether the kernel recorded the request.
    pub accepted: bool,
}

/// One wrong-way entry the driver recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct WrongWayEntry {
    /// The corridor's guide path name.
    pub corridor: &'static str,
    /// The rider the request named.
    pub agent: AgentId,
    /// Whether the kernel recorded the request (a disconnected corridor's
    /// request is refused before any draw).
    pub accepted: bool,
}

/// One closed overtaking interval, in the deterministic form its hash and
/// occurrence evidence need.
#[derive(Debug, Clone, PartialEq)]
pub struct Overtake {
    /// The passing agent.
    pub agent: AgentId,
    /// The passed body.
    pub partner: AgentId,
    /// First observed tick the pair was alongside.
    pub start_tick: u64,
    /// The tick the interval closed.
    pub end_tick: u64,
    /// The minimum exact body clearance observed.
    pub min_clearance_m: f64,
    /// The declared bands the interval accumulated, in declaration order.
    pub bands: Vec<ClearanceBandId>,
    /// The bands the interval recorded as violated.
    pub violating_bands: Vec<ClearanceBandId>,
}

/// One opposing-traversal boundary record the run emitted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpposingBoundary {
    /// The turning agent.
    pub agent: AgentId,
    /// Whether the record opened or closed the interval.
    pub entering: bool,
    /// Whether the traversal was recorded as a violation.
    pub violating: bool,
}

/// Everything one run observed besides its canonical trace.
#[derive(Debug, Default)]
pub struct Observation {
    /// The maneuver state edges each agent recorded, in step order.
    pub edges: BTreeMap<AgentId, Vec<(ManeuverState, ManeuverState, ManeuverEdge)>>,
    /// Every lateral maneuver reason observed for an agent over the run.
    pub reasons: BTreeMap<AgentId, BTreeSet<ManeuverReason>>,
    /// Agents the run admitted.
    pub spawned: BTreeSet<AgentId>,
    /// Agents the run released at the end of their guide path.
    pub completed_route: BTreeSet<AgentId>,
    /// Every contact the run recorded, as a description.
    pub contacts: Vec<String>,
    /// The facility handoffs the run performed.
    pub facility_transitions: usize,
    /// The close-pass intervals the run closed.
    pub overtakes: Vec<Overtake>,
    /// The lateral maneuver request the driver recorded, if the plan makes one.
    pub request: Option<Request>,
    /// The wrong-way entries the driver recorded, in corridor order.
    pub wrong_way: Vec<WrongWayEntry>,
    /// The wrong-way interval boundaries the run emitted.
    pub opposing_boundaries: Vec<OpposingBoundary>,
}

impl Observation {
    /// Read one completed step's events, transition records, and live tactical
    /// reasons.
    fn record(
        &mut self,
        sim: &Simulation,
        _tick: u64,
        events: &[Event],
        transitions: Vec<(AgentId, ManeuverState, ManeuverState, ManeuverEdge)>,
    ) {
        for (agent, from, to, edge) in transitions {
            self.edges.entry(agent).or_default().push((from, to, edge));
        }
        for event in events {
            match event {
                Event::Spawned { agent, .. } => {
                    self.spawned.insert(*agent);
                }
                Event::Despawned { agent, reason, .. } => {
                    if matches!(reason, DespawnReason::ExitedPath) {
                        self.completed_route.insert(*agent);
                    }
                }
                Event::FacilityTransition { .. } => self.facility_transitions += 1,
                Event::Collision {
                    agent,
                    other,
                    clearance_m,
                    contacting: true,
                } => self
                    .contacts
                    .push(format!("{agent:?}/{other:?} at {clearance_m:.4} m")),
                Event::OpposingTraversal {
                    agent,
                    entering,
                    violating,
                    ..
                } => self.opposing_boundaries.push(OpposingBoundary {
                    agent: *agent,
                    entering: *entering,
                    violating: *violating,
                }),
                _ => {}
            }
        }
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            if let Some(reason) = sim.lateral_maneuver_reason(sample.id) {
                self.reasons.entry(sample.id).or_default().insert(reason);
            }
        }
    }

    /// Read the closed overtaking intervals the tracker holds.
    fn record_overtakes(&mut self, sim: &Simulation) {
        self.overtakes = sim
            .close_pass_tracker()
            .overtakes()
            .iter()
            .map(|observation| Overtake {
                agent: observation.agent,
                partner: observation.partner,
                start_tick: observation.start_tick,
                end_tick: observation.end_tick,
                min_clearance_m: observation.min_clearance_m,
                bands: observation.bands.iter().map(|band| band.band).collect(),
                violating_bands: observation.violating_bands.clone(),
            })
            .collect();
    }

    /// Whether an agent recorded the full maneuver edges
    /// `following -> preparing -> committed -> returning -> following`, in
    /// order, with no abort. Returns the edge count seen for the agent.
    pub fn completed_maneuver(&self, agent: AgentId) -> Option<usize> {
        let edges = self.edges.get(&agent)?;
        let mut state = ManeuverState::Following;
        for (from, to, edge) in edges {
            if *from != state || *edge == ManeuverEdge::Aborted {
                return None;
            }
            state = *to;
        }
        (state == ManeuverState::Following && edges.len() >= 4).then_some(edges.len())
    }

    /// Whether the run recorded the reason for `agent`.
    pub fn recorded_reason(&self, agent: AgentId, reason: ManeuverReason) -> bool {
        self.reasons
            .get(&agent)
            .is_some_and(|reasons| reasons.contains(&reason))
    }
}

/// The wrong-way driver: the TAS-131 suite's own condition, spelled with stable
/// corridor names instead of dense path indices.
///
/// The three muted corridors (permitted, prohibited-but-connected, disconnected)
/// turn their lone rider once it reaches the authored window, and the occupied
/// corridor turns the leading body of its first adjacent pair inside the
/// stopping windows. The driver reads only corridor identity and physical state,
/// so the identical logic runs at every preset.
struct WrongWayDriver {
    /// The corridors' dense path indices, in the fixture's own order.
    corridors: Vec<PathId>,
    /// Whether each corridor has already recorded its entry.
    asked: [bool; 4],
}

impl WrongWayDriver {
    fn new(scenario: &CompiledScenario) -> Self {
        let corridors = WRONG_WAY_CORRIDORS
            .iter()
            .map(|name| {
                scenario
                    .paths()
                    .iter()
                    .position(|path| path.name() == *name)
                    .map(PathId::from_index)
                    .unwrap_or_else(|| panic!("the fixture authors a guide path named '{name}'"))
            })
            .collect();
        Self {
            corridors,
            asked: [false; 4],
        }
    }

    /// Record the entries this tick's state opens, exactly once per corridor.
    fn request(&mut self, sim: &mut Simulation, observation: &mut Observation) {
        for (corridor, name) in WRONG_WAY_CORRIDORS.iter().enumerate().take(3) {
            if self.asked[corridor] {
                continue;
            }
            if let Some(agent) = lone_rider_on_corridor(sim, self.corridors[corridor], 5.0..=20.0) {
                let accepted = sim.request_wrong_way_entry(agent);
                observation.wrong_way.push(WrongWayEntry {
                    corridor: name,
                    agent,
                    accepted,
                });
                self.asked[corridor] = true;
            }
        }
        if !self.asked[3]
            && let Some(agent) = adjacent_lead_on(sim, self.corridors[3], 25.0..=48.0, 8.0..=18.0)
        {
            let accepted = sim.request_wrong_way_entry(agent);
            observation.wrong_way.push(WrongWayEntry {
                corridor: WRONG_WAY_CORRIDORS[3],
                agent,
                accepted,
            });
            self.asked[3] = true;
        }
    }
}

/// The bodies on `path`, as `(id, arc length, body length)`, sorted by arc
/// length so adjacent pairs are consecutive.
fn bodies_on_path(sim: &Simulation, path: PathId) -> Vec<(AgentId, f64, f64)> {
    let mut bodies: Vec<(AgentId, f64, f64)> = sim
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            (motion.path == path).then_some((
                sample.id,
                motion.path_distance_m,
                motion.body_length_m,
            ))
        })
        .collect();
    bodies.sort_by(|first, second| first.1.total_cmp(&second.1));
    bodies
}

/// The single body on `path` when the corridor holds exactly one, if its arc
/// length lies in `window`.
fn lone_rider_on_corridor(
    sim: &Simulation,
    path: PathId,
    window: std::ops::RangeInclusive<f64>,
) -> Option<AgentId> {
    match bodies_on_path(sim, path)[..] {
        [(id, s_m, _)] if window.contains(&s_m) => Some(id),
        _ => None,
    }
}

/// The leading body of the first adjacent pair on `path` whose leader lies in
/// `lead_window` and whose bumper gap lies in `gap_window`.
fn adjacent_lead_on(
    sim: &Simulation,
    path: PathId,
    lead_window: std::ops::RangeInclusive<f64>,
    gap_window: std::ops::RangeInclusive<f64>,
) -> Option<AgentId> {
    let bodies = bodies_on_path(sim, path);
    for pair in bodies.windows(2) {
        let (occupancy, lead) = (pair[0], pair[1]);
        let gap_m = (lead.1 - occupancy.1).abs() - (lead.2 + occupancy.2) * 0.5;
        if lead_window.contains(&lead.1) && gap_window.contains(&gap_m) {
            return Some(lead.0);
        }
    }
    None
}

/// Record the change of lane a tactical leaf requests for the follower once the
/// leader it displaces around is inside the approach window.
///
/// The leader and the follower are the two lowest live agent ids, the same pair
/// the landed `crates/hekate-sim/tests/inc2_passing_fixtures.rs` driver requests
/// for, and the request is only recorded when the follower wants to be faster
/// than the leader it follows.
fn request_lane_change(
    sim: &mut Simulation,
    target_index: usize,
    window_m: (f64, f64),
) -> Option<Request> {
    let samples: Vec<(AgentId, f64, f64)> = sim
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .map(|sample| {
            let motion = sample.motion.as_ref().expect("full detail");
            (
                sample.id,
                motion.path_distance_m,
                sim.agent_profile(sample.id)
                    .expect("a demand vehicle carries a profile")
                    .desired_speed_mps,
            )
        })
        .collect();
    let leader = samples
        .iter()
        .min_by_key(|sample| sample.0.get())
        .copied()?;
    let follower = samples
        .iter()
        .filter(|sample| sample.0 != leader.0)
        .min_by_key(|sample| sample.0.get())
        .copied()?;
    if follower.2 <= leader.2 || !(window_m.0..=window_m.1).contains(&(leader.1 - follower.1)) {
        return None;
    }
    let accepted = sim.request_lateral_maneuver(
        follower.0,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: leader.0,
            target_facility: Some(FacilityId::from_index(target_index)),
        },
    );
    Some(Request {
        agent: follower.0,
        passed: leader.0,
        accepted,
    })
}

/// Whether `run` produced the fixture's intended maneuver, or why it did not.
///
/// This is the occurrence predicate the pinned bank seed satisfies at both
/// required presets: a hash comparison over an empty run would prove nothing, so
/// every determinism and golden assertion first requires that the maneuver
/// itself happened.
pub fn maneuver_occurs(fixture: &Fixture, run: &Run) -> Result<(), String> {
    let observation = &run.observation;
    match fixture.expectation {
        Expectation::Overtake { violating_band } => {
            if observation.overtakes.is_empty() {
                return Err("expected an overtaking interval, got none".to_owned());
            }
            if !observation.contacts.is_empty() {
                return Err(format!(
                    "the pass recorded contact: {:?}",
                    observation.contacts
                ));
            }
            let mut passing = Vec::new();
            for overtake in &observation.overtakes {
                let Some(_) = observation.completed_maneuver(overtake.agent) else {
                    continue;
                };
                for participant in [overtake.agent, overtake.partner] {
                    if !observation.spawned.contains(&participant)
                        || !observation.completed_route.contains(&participant)
                    {
                        return Err(format!(
                            "{participant:?} did not complete its route behind the pass"
                        ));
                    }
                }
                if !observation.recorded_reason(overtake.agent, ManeuverReason::SlowerLeader) {
                    return Err(format!(
                        "{:?} did not select a slower leader: {:?}",
                        overtake.agent,
                        observation.reasons.get(&overtake.agent)
                    ));
                }
                passing.push(overtake);
            }
            if passing.is_empty() {
                return Err(format!(
                    "no passing agent recorded the whole maneuver lifecycle: {:?}",
                    observation.edges
                ));
            }
            if violating_band {
                let violated = passing.iter().any(|overtake| {
                    overtake.min_clearance_m > 0.0
                        && overtake.min_clearance_m < 0.5
                        && overtake.violating_bands == vec![ClearanceBandId::from_index(0)]
                });
                if !violated {
                    return Err(format!(
                        "no pass fell inside the authored violation band: {:?}",
                        passing
                    ));
                }
            } else if !passing
                .iter()
                .any(|overtake| overtake.violating_bands.is_empty())
            {
                return Err(format!(
                    "no pass stayed outside the authored violation band: {:?}",
                    passing
                ));
            }
        }
        Expectation::LaneChange {
            handoffs,
            prevented,
        } => {
            let Some(request) = observation.request else {
                return Err("the approach never reached the request window".to_owned());
            };
            if !request.accepted {
                return Err("the tactical leaf's change of lane was refused".to_owned());
            }
            if observation.facility_transitions != handoffs {
                return Err(format!(
                    "expected {handoffs} facility handoffs, got {}",
                    observation.facility_transitions
                ));
            }
            if prevented {
                if !observation.recorded_reason(request.agent, ManeuverReason::BoundaryForbidden) {
                    return Err(format!(
                        "{:?} did not record the forbidden boundary: {:?}",
                        request.agent,
                        observation.reasons.get(&request.agent)
                    ));
                }
                if !observation.edges.is_empty() {
                    return Err(format!(
                        "a prevented crossing records no lifecycle: {:?}",
                        observation.edges
                    ));
                }
                if !observation.overtakes.is_empty() {
                    return Err(format!(
                        "the prevented crossing passes nobody, got {}",
                        observation.overtakes.len()
                    ));
                }
            } else {
                if observation.completed_maneuver(request.agent).is_none() {
                    return Err(format!(
                        "{:?} did not record the whole maneuver lifecycle: {:?}",
                        request.agent,
                        observation.edges.get(&request.agent)
                    ));
                }
                if observation.overtakes.is_empty() {
                    return Err("the change of lane must open an overtaking interval".to_owned());
                }
            }
            for participant in [request.agent, request.passed] {
                if !observation.spawned.contains(&participant)
                    || !observation.completed_route.contains(&participant)
                {
                    return Err(format!("{participant:?} did not complete its route"));
                }
            }
            if !observation.contacts.is_empty() {
                return Err(format!(
                    "the change of lane recorded contact: {:?}",
                    observation.contacts
                ));
            }
        }
        Expectation::WrongWay => {
            if observation.wrong_way.len() != 4 {
                return Err(format!(
                    "every corridor must reach its decision, got {:?}",
                    observation.wrong_way
                ));
            }
            let accepted: Vec<bool> = observation
                .wrong_way
                .iter()
                .map(|entry| entry.accepted)
                .collect();
            if accepted != [true, true, false, true] {
                return Err(format!(
                    "the permitted, prohibited-but-connected, disconnected, and occupied \
                     corridors must record entered, entered, refused, entered, got {accepted:?}"
                ));
            }
            let opened = observation
                .opposing_boundaries
                .iter()
                .any(|boundary| boundary.entering);
            let closed = observation
                .opposing_boundaries
                .iter()
                .any(|boundary| !boundary.entering);
            if !(opened && closed) {
                return Err(format!(
                    "the run must open and close an opposing traversal: {:?}",
                    observation.opposing_boundaries
                ));
            }
            if observation.contacts.is_empty() {
                return Err("the occupied corridor must reach its contact".to_owned());
            }
        }
    }
    Ok(())
}

/// Assert the fixture's intended maneuver happened, naming the fixture and the
/// preset in the failure.
pub fn assert_maneuver_occurs(fixture: &Fixture, preset: &str, run: &Run) {
    if let Err(reason) = maneuver_occurs(fixture, run) {
        panic!(
            "{} [{preset}] seed {}: the fixture's intended maneuver did not occur: {reason}",
            fixture.id, fixture.pinned_seed
        );
    }
}

/// The canonical trace lines of one `event` kind, in stream order.
pub fn events_of_kind(bytes: &[u8], kind: &str) -> Vec<serde_json::Value> {
    std::str::from_utf8(bytes)
        .expect("a canonical trace is UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("every line is JSON"))
        .filter(|record| record.get("event").and_then(|event| event.as_str()) == Some(kind))
        .collect()
}
