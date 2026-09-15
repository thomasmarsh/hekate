//! The run directory's `metrics.json`: the versioned metric values of one run.
//!
//! A completed run directory records the canonical event stream and the sampled
//! trajectories, but the interaction-metric values the kernel observes live
//! only in the process and differ per seed. Multi-seed aggregation therefore has
//! nothing to read. This module makes the run directory self-describing for
//! metrics: it captures the run's metric minima and event counts from the live
//! [`Simulation`] and writes them as one `metrics.json` artifact alongside
//! `manifest.json`, `summary.json`, `events.jsonl.gz`, and
//! `trajectories.parquet`.
//!
//! The artifact carries `metric_definition_version: 3` and the run's
//! `manifest_sha256`, so every number ties back to the versioned definition
//! ([[DEF-006-metric-definition-v3]], which carries
//! [[DEF-005-metric-definition-v2]] and [[DEF-004-metric-definition-v1]]
//! forward) and to the manifest that produced it. It holds:
//!
//! - the run-level interaction-metric minima — minimum time to collision,
//!   minimum surface separation, and minimum post-encroachment time — each with
//!   an explicit status: a metric that has no value is `not_applicable` or
//!   `not_observed`, never a false `0`;
//! - the separation minimum of each [`ModePair`] (`vehicle_vehicle`,
//!   `vehicle_pedestrian`, `pedestrian_pedestrian`);
//! - the metric minima disaggregated by **movement**, keyed by the pair's two
//!   movement keys — the tagged union of `MovementId` for vehicles and
//!   `PedestrianRouteId` for pedestrians that [[DEF-004-metric-definition-v1]]
//!   chose;
//! - the countable event families, with the mode and movement slices the records
//!   and the compiled scenario allow;
//! - the operational families [[DEF-005-metric-definition-v2]] adds —
//!   throughput, delay (travel time, stopped delay, control delay), and queues
//!   (length and duration) — at the run level, per [`AgentMode`], and per
//!   movement, each with its unit, its status, and its provenance;
//! - the overtaking and close-pass families metric definition v3
//!   ([[DEF-006-metric-definition-v3]]) adds, each with its explicit
//!   applicability: the overtaking attempt, commit, completion, and abort counts
//!   of the `Maneuver` records, and the close-pass count, minimum clearance
//!   (with the time and relative speed at it), per-band duration series, and
//!   violation count read from the closed observations the tracker holds — over
//!   the run, per mode pair, movement, and facility.
//!
//! ## Close-pass bucket rule
//!
//! The close-pass families are the closed [`OvertakeObservation`] records
//! ([`Simulation::close_pass_tracker`]), not the `ClosePass` events: an event is
//! gated on a compiled shared facility and a passing mode, while the observation
//! always closes, and metric definition v3 fixes the metric set as the tracker's
//! slice.
//!
//! A bucket that cannot host a pass reports no clearance value at all: its
//! minimum clearance and every band duration are `not_applicable`, never a `0`.
//! Metric definition v3 fixes the two predicates — a mode pair in which neither
//! body's mode class declares the `overtake` or `pass` tactic, and a facility
//! whose compiled `lateral_use` is `centered`, so only nominal travel is
//! possible — and a bucket that could host a pass but recorded no observation is
//! `not_observed`. The countable families stay counts, as v3 fixes: an absent
//! count is an observed zero (`close_passes: 0`), never an absent value. The
//! disaggregation keys are v1's: the pair's two movement keys sorted and joined
//! with `|`, the shared facility spelled `facility:<name>`, and the two bodies'
//! [`ModePair`]. An observation with no shared compiled facility contributes to
//! no facility bucket, exactly as v1 omits a body with no route.
//!
//! ## Movement bucket rule
//!
//! A movement key is spelled `movement:<name>` for a vehicle movement and
//! `pedestrian_route:<name>` for a pedestrian route, where `<name>` is the
//! authored identifier the compiled scenario exposes through
//! `CompiledScenario::movement_name` and
//! `CompiledScenario::pedestrian_route_name`. A pairwise metric belongs to the
//! bucket named by its two bodies' movement keys, sorted lexicographically and
//! joined with `|`, so a pair and its mirror share one bucket; an operational
//! metric is a property of one agent alone, so it belongs to the bucket of that
//! agent's own movement key. A bucket's value is the least observed value over
//! every pair whose two movement keys name it. A body with no assigned movement
//! (the initial static population) has no movement key, so a pair with such a
//! body contributes to the run-level and mode-pair minima but to no movement
//! bucket.
//!
//! The per-bucket minima come from the kernel's per-pair minima
//! ([`InteractionMetrics::pair_minimum_separation_m`] and
//! [`InteractionMetrics::pair_minimum_ttc_s`]) over the pairs whose bodies both
//! carry a movement key, and from the recorded post-encroachment successions
//! ([`InteractionMetrics::post_encroachments`]). Enumerating the pairs is a
//! completion-time step bounded by the square of the number of spawned agents,
//! which is what the reported movement minima cost.
//!
//! ## Immutability
//!
//! `metrics.json` is written once, before the `manifest.json` completion marker,
//! like every other run artifact, so it is present exactly when the directory
//! reads as complete and no rerun mutates it.

use std::collections::{BTreeMap, BTreeSet};

use hekate_model::{
    AgentFamily, CompiledClearanceBand, FacilityId, LateralUse, TacticKind, TacticalCapability,
};
use hekate_sim::{
    AgentId, AgentMode, Event, InteractionMetrics, ManeuverEdge, ManeuverState, MetricMinimum,
    ModePair, MovementKey, OperationValues as ObservedValues, OvertakeObservation, Simulation,
    StepOutput,
};
use serde::{Deserialize, Serialize};

use crate::trace::sha256_hex;

/// File name of the metrics artifact inside a run directory.
pub const METRICS_FILE: &str = "metrics.json";

/// The metric definition revision this artifact reports.
///
/// Fixed at `3` by [[DEF-006-metric-definition-v3]], which carries metric
/// definition v2 ([[DEF-005-metric-definition-v2]]) forward unchanged and adds
/// the close-pass families — the overtaking attempt, commit, completion, and
/// abort counts and the close-pass observation, minimum-clearance, clearance-
/// band-duration, and violation families — that v2 did not report. A change to
/// any reported metric's name, formula, unit, applicability, tie-break, or
/// disaggregation key bumps this with the code change, and an artifact already
/// written stays attributed to the revision that produced it.
pub const METRIC_DEFINITION_VERSION: u32 = 3;

/// The reporting status of one metric value.
///
/// The three statuses metric definition v1 distinguishes and v2 carries
/// forward, kept distinct so an artifact never conflates "no claim was made"
/// with "the interaction was safe".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    /// The metric has a value for this run, pair, or bucket.
    Reported,
    /// The predicate that would define a value is false here: a time to
    /// collision for a pair that never closed, or a post-encroachment time for
    /// no valid succession. The value is absent, never `0`.
    NotApplicable,
    /// No observation was made: no pair came within the reporting range, or no
    /// region occupancy was recorded. There is no value and no safety claim.
    NotObserved,
}

/// One metric value with its reporting status and, when reported, the least
/// observation's provenance.
///
/// `value` carries the metric's unit by its field name at the call site —
/// seconds for a time to collision or a post-encroachment time, metres for a
/// separation — as [[DEF-004-metric-definition-v1]] fixes. The provenance fields
/// are present only on a reported value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricValue {
    /// Whether this run recorded a value.
    pub status: MetricStatus,
    /// The value, in the metric's unit. Absent unless [`Self::status`] is
    /// [`MetricStatus::Reported`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Lower [`AgentId`] of the pair that produced the value, spelled as the
    /// collision records spell it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<u32>,
    /// Higher [`AgentId`] of the pair that produced the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<u32>,
    /// The [`ModePair`] of the pair that produced the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_pair: Option<String>,
    /// Completed tick the value was observed on, for a run-level minimum or a
    /// queue statistic. A per-movement value omits it, because the per-pair
    /// accessor records the pair's minimum and not the tick that produced it,
    /// and an aggregate over a run (a total, mean, or rate) is not observed on
    /// one tick at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick: Option<u64>,
}

impl MetricValue {
    /// A reported value with its pair provenance; `tick` is `Some` only for a
    /// run-level minimum.
    fn reported(value: f64, agent: u32, other: u32, mode_pair: &str, tick: Option<u64>) -> Self {
        Self {
            status: MetricStatus::Reported,
            value: Some(value),
            agent: Some(agent),
            other: Some(other),
            mode_pair: Some(mode_pair.to_owned()),
            tick,
        }
    }

    /// A reported value that is a property of the run rather than of one pair:
    /// a total, a mean, or a rate. Absent provenance is exactly that.
    fn reported_run(value: f64) -> Self {
        Self {
            status: MetricStatus::Reported,
            value: Some(value),
            agent: None,
            other: None,
            mode_pair: None,
            tick: None,
        }
    }

    /// A reported value observed on one completed tick, without an agent: the
    /// standing-agent count a queue length reports.
    fn reported_tick(value: f64, tick: u64) -> Self {
        Self {
            tick: Some(tick),
            ..Self::reported_run(value)
        }
    }

    /// A reported value one agent produced, observed on one completed tick: the
    /// longest stopped state a queue duration reports.
    fn reported_agent(value: f64, agent: AgentId, tick: u64) -> Self {
        Self {
            agent: Some(agent.get()),
            ..Self::reported_tick(value, tick)
        }
    }

    /// A reported run-level minimum, carrying its pair and tick.
    fn reported_minimum(minimum: MetricMinimum) -> Self {
        Self::reported(
            minimum.value,
            minimum.agent.get(),
            minimum.other.get(),
            minimum.mode_pair.label(),
            Some(minimum.tick),
        )
    }

    /// A reported per-movement minimum, carrying its pair but no tick.
    fn reported_pair(minimum: PairMinimum) -> Self {
        Self::reported(
            minimum.value,
            minimum.agent,
            minimum.other,
            minimum.mode_pair.label(),
            None,
        )
    }

    /// An applicable metric with no value recorded: an explicit no-value status.
    fn not_applicable() -> Self {
        Self::absent(MetricStatus::NotApplicable)
    }

    /// A metric with no observation at all.
    fn not_observed() -> Self {
        Self::absent(MetricStatus::NotObserved)
    }

    fn absent(status: MetricStatus) -> Self {
        Self {
            status,
            value: None,
            agent: None,
            other: None,
            mode_pair: None,
            tick: None,
        }
    }

    /// A run-level minimum, or `absent` when the run recorded no value.
    fn minimum(minimum: Option<MetricMinimum>, absent: MetricStatus) -> Self {
        match minimum {
            Some(minimum) => Self::reported_minimum(minimum),
            None => Self::absent(absent),
        }
    }
}

/// The metric minima of one movement bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementMinima {
    /// The bucket's two movement keys, sorted lexicographically.
    pub movement_keys: [String; 2],
    /// Least surface separation in metres of a pair in this bucket.
    pub minimum_separation_m: MetricValue,
    /// Least time to collision in seconds of a pair in this bucket.
    pub minimum_ttc_s: MetricValue,
    /// Least post-encroachment time in seconds of a succession in this bucket.
    pub minimum_post_encroachment_s: MetricValue,
}

/// The countable event families of metric definition v1, in the order v1
/// declares them.
///
/// A run artifact writes all ten into [`EventCounts::by_family`] — `0` when the
/// family is absent — so a consumer that enumerates the families reads a
/// complete set from this list rather than from a sparse `by_family_mode` or
/// `by_family_movement` slice. A count of an absent family is an observed zero,
/// never an absent value. The artifact's own maps sort their keys, so this list
/// fixes the family set rather than an artifact key order.
pub const EVENT_FAMILY_LABELS: [&str; 10] = [
    "collisions",
    "near_misses",
    "violations",
    "region_entries",
    "region_exits",
    "queue_events",
    "control_transitions",
    "yields",
    "spawns",
    "despawns",
];

/// The countable event families of metric definition v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EventFamily {
    Collisions,
    NearMisses,
    Violations,
    RegionEntries,
    RegionExits,
    QueueEvents,
    ControlTransitions,
    Yields,
    Spawns,
    Despawns,
}

impl EventFamily {
    /// Every family, in the artifact's stable order.
    const ALL: [Self; 10] = [
        Self::Collisions,
        Self::NearMisses,
        Self::Violations,
        Self::RegionEntries,
        Self::RegionExits,
        Self::QueueEvents,
        Self::ControlTransitions,
        Self::Yields,
        Self::Spawns,
        Self::Despawns,
    ];

    /// The family's stable artifact key.
    const fn label(self) -> &'static str {
        match self {
            Self::Collisions => "collisions",
            Self::NearMisses => "near_misses",
            Self::Violations => "violations",
            Self::RegionEntries => "region_entries",
            Self::RegionExits => "region_exits",
            Self::QueueEvents => "queue_events",
            Self::ControlTransitions => "control_transitions",
            Self::Yields => "yields",
            Self::Spawns => "spawns",
            Self::Despawns => "despawns",
        }
    }

    /// The variant-kind labels this family is split by, in stable order; empty
    /// for a family without a kind.
    const fn kinds(self) -> &'static [&'static str] {
        match self {
            Self::Violations => &["ran_red_light", "crossed_against_signal"],
            Self::ControlTransitions => &["signal_stop", "crossing_wait"],
            _ => &[],
        }
    }
}

/// Event counts by family, with the mode and movement slices the records allow.
///
/// `by_family` holds every family, `0` when the family is absent (a run that
/// records no collision reports `collisions: 0`). The slices are sparse: a mode
/// or movement with no counted record is omitted. A pair event is attributed to
/// its `agent`, the lower identifier, exactly as the record spells it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventCounts {
    /// Counted records over every family.
    pub total: u64,
    /// Counted records per family, every family present.
    pub by_family: BTreeMap<String, u64>,
    /// Counted records per variant kind, for the two kinded families.
    pub by_family_kind: BTreeMap<String, BTreeMap<String, u64>>,
    /// Counted records per family and subject-agent mode.
    pub by_family_mode: BTreeMap<String, BTreeMap<String, u64>>,
    /// Counted records per family and subject-agent movement key.
    pub by_family_movement: BTreeMap<String, BTreeMap<String, u64>>,
}

/// The operational values of one disaggregation bucket of metric definition v2.
///
/// Every field is a metric value with the v1 statuses: `reported` with its
/// value and provenance, `not_applicable` when the predicate that would define
/// the value is false, and `not_observed` when the bucket made no observation.
/// A reported zero is a value (`0.0`), never a marker for an absent one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationalValues {
    /// Served agents per simulated second.
    pub throughput_agents_per_s: MetricValue,
    /// Mean trip time in seconds of a served agent.
    pub mean_travel_time_s: MetricValue,
    /// Total trip time in seconds over the bucket's served agents.
    pub total_travel_time_s: MetricValue,
    /// Mean stopped delay in seconds per served agent.
    pub mean_stopped_delay_s: MetricValue,
    /// Total stopped delay in seconds over the bucket's served agents.
    pub total_stopped_delay_s: MetricValue,
    /// Mean control delay in seconds per served agent.
    pub mean_control_delay_s: MetricValue,
    /// Total control delay in seconds over the bucket's served agents.
    pub total_control_delay_s: MetricValue,
    /// Most standing agents the bucket held at one tick end.
    pub maximum_queue_length_agents: MetricValue,
    /// Longest stopped state the bucket closed, in seconds.
    pub maximum_queue_duration_s: MetricValue,
    /// Mean duration in seconds of the stopped states the bucket closed.
    pub mean_queue_duration_s: MetricValue,
}

impl OperationalValues {
    /// The values of a bucket that observed nothing: every metric is
    /// `not_observed`, which is what a run that never held an agent reports.
    ///
    /// A consumer reading an artifact uses this shape to interpret a bucket no
    /// agent reached; nothing here is a reported zero.
    pub fn not_observed() -> Self {
        let absent = || MetricValue::not_observed();
        Self {
            throughput_agents_per_s: absent(),
            mean_travel_time_s: absent(),
            total_travel_time_s: absent(),
            mean_stopped_delay_s: absent(),
            total_stopped_delay_s: absent(),
            mean_control_delay_s: absent(),
            total_control_delay_s: absent(),
            maximum_queue_length_agents: absent(),
            maximum_queue_duration_s: absent(),
            mean_queue_duration_s: absent(),
        }
    }

    /// The artifact values of one observed bucket, with each metric's status.
    ///
    /// The absence rule comes from the observation the kernel reports and never
    /// from the value: a bucket that held agents but served none reports a
    /// throughput of `0.0`, while a bucket that never held an agent reports no
    /// observation at all. Throughput is the one metric whose value needs
    /// elapsed time, so a run that has elapsed none is `not_applicable` rather
    /// than reported or unobserved.
    fn of(observed: &ObservedValues) -> Self {
        let unobserved = observed.observed_agents == 0;
        let unserved = observed.served_agents == 0;
        let delay = |value: f64| match unserved {
            true => MetricValue::not_observed(),
            false => MetricValue::reported_run(value),
        };
        Self {
            throughput_agents_per_s: match observed.throughput_agents_per_s {
                Some(value) => MetricValue::reported_run(value),
                None if unobserved => MetricValue::not_observed(),
                None => MetricValue::not_applicable(),
            },
            mean_travel_time_s: match observed.mean_travel_time_s {
                Some(value) => MetricValue::reported_run(value),
                None => MetricValue::not_observed(),
            },
            total_travel_time_s: delay(observed.total_travel_time_s),
            mean_stopped_delay_s: match observed.mean_stopped_delay_s {
                Some(value) => MetricValue::reported_run(value),
                None => MetricValue::not_observed(),
            },
            total_stopped_delay_s: delay(observed.total_stopped_delay_s),
            mean_control_delay_s: match observed.mean_control_delay_s {
                Some(value) => MetricValue::reported_run(value),
                None => MetricValue::not_observed(),
            },
            total_control_delay_s: delay(observed.total_control_delay_s),
            maximum_queue_length_agents: match observed.maximum_queue_length {
                Some(length) => MetricValue::reported_tick(length.agents as f64, length.tick),
                None => MetricValue::not_observed(),
            },
            maximum_queue_duration_s: match observed.maximum_queue_duration {
                Some(duration) => {
                    MetricValue::reported_agent(duration.seconds, duration.agent, duration.tick)
                }
                None => MetricValue::not_observed(),
            },
            mean_queue_duration_s: match observed.mean_queue_duration_s {
                Some(value) => MetricValue::reported_run(value),
                None => MetricValue::not_observed(),
            },
        }
    }
}

/// Metric definition v2's operational metrics: over the run, per mode, and per
/// movement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationalMetrics {
    /// The values over the whole run.
    pub run: OperationalValues,
    /// The values per [`AgentMode`] label (`vehicle`, `pedestrian`).
    pub by_mode: BTreeMap<String, OperationalValues>,
    /// The values per movement key, for the movements the run observed.
    pub by_movement: BTreeMap<String, OperationalValues>,
}

/// The close-pass metric families of metric definition v3: over the run, per
/// mode pair, per movement, and per facility.
///
/// The four bucket maps are the family set metric definition v3 fixes. The
/// mode-pair map holds all three [`ModePair`] labels and the facility map holds
/// every compiled facility, so a bucket that could host a pass reports its
/// explicit no-observation status rather than vanishing; a movement bucket is
/// the pairwise key v1 chose and exists once a counted record names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosePassMetrics {
    /// The values over the whole run.
    pub run: ClosePassValues,
    /// The values per [`ModePair`] label.
    pub by_mode_pair: BTreeMap<String, ClosePassValues>,
    /// The values per pairwise movement key.
    pub by_movement: BTreeMap<String, ClosePassValues>,
    /// The values per facility key (`facility:<name>`).
    pub by_facility: BTreeMap<String, ClosePassValues>,
}

impl ClosePassMetrics {
    /// The families of a run that closed no observation: every countable family
    /// is `0` — an absent count is an observed zero, as metric definition v3
    /// fixes — and every value family carries the no-observation status.
    ///
    /// The run bucket and the three [`ModePair`] buckets a scenario always
    /// declares are present; the movement and facility maps are empty, because
    /// a bucket of either exists only once a run attributes a counted record to
    /// it and a compiled scenario is what names it.
    pub fn not_observed() -> Self {
        let values = || ClosePassAccumulator::default().finish(true, &[]);
        Self {
            run: values(),
            by_mode_pair: [
                ModePair::VehicleVehicle,
                ModePair::VehiclePedestrian,
                ModePair::PedestrianPedestrian,
            ]
            .into_iter()
            .map(|pair| (pair.label().to_owned(), values()))
            .collect(),
            by_movement: BTreeMap::new(),
            by_facility: BTreeMap::new(),
        }
    }
}

/// The overtaking and close-pass families of one bucket of metric definition
/// v3.
///
/// A countable family ([`Self::overtake_attempts`], [`Self::overtake_commits`],
/// [`Self::overtake_completions`], [`Self::overtake_aborts`],
/// [`Self::close_passes`], and [`Self::close_pass_violations`]) is always a
/// count — `0` when the bucket recorded none, never an absent value — which is
/// v3's countable-family rule. The two value families carry the bucket's
/// applicability: [`Self::close_pass_minimum_clearance_m`] and each entry of
/// [`Self::clearance_band_durations_s`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosePassValues {
    /// Count of the maneuvering agents' `TacticKind::Overtake` attempt edges.
    pub overtake_attempts: MetricValue,
    /// Count of the overtaking commit edges.
    pub overtake_commits: MetricValue,
    /// Count of the overtaking completion edges (`committed -> returning`).
    pub overtake_completions: MetricValue,
    /// Count of the overtaking abort edges.
    pub overtake_aborts: MetricValue,
    /// Count of the closed overtaking observations in the bucket.
    pub close_passes: MetricValue,
    /// Least signed surface clearance in metres over the bucket's observations,
    /// with the time and relative speed at that minimum.
    pub close_pass_minimum_clearance_m: ClosePassMinimum,
    /// Seconds each declared clearance band accumulated over the bucket's
    /// observations, keyed by the band's stable [`ClearanceBandId`] and reported
    /// in the scenario's declaration order.
    ///
    /// [`ClearanceBandId`]: hekate_model::ClearanceBandId
    pub clearance_band_durations_s: BTreeMap<u32, MetricValue>,
    /// Count of the observations that recorded a violating band.
    pub close_pass_violations: MetricValue,
}

/// The least close-pass observation of one bucket: the signed surface clearance
/// and the time and relative speed at that minimum.
///
/// Metric definition v3 fixes the clearance's unit as metres — positive is the
/// disjoint surface distance, zero is contact, negative is penetration — and
/// requires the record of the minimum to carry the tick's simulated time and the
/// pair's relative speed along the shared reference, so the two are fields here
/// rather than reported values of their own. The statuses are the v1 statuses,
/// applied to the bucket: `reported` with the minimum's pair provenance,
/// `not_applicable` for a bucket that cannot host a pass, and `not_observed` for
/// a bucket that could host one but recorded no observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosePassMinimum {
    /// Whether this bucket recorded an observation to report.
    pub status: MetricStatus,
    /// The least signed clearance in metres. Absent unless [`Self::status`] is
    /// [`MetricStatus::Reported`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The passing agent of the pair that produced the minimum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<u32>,
    /// The passed body of that pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<u32>,
    /// The [`ModePair`] of that pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_pair: Option<String>,
    /// Simulated time of the minimum, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_s: Option<f64>,
    /// Relative speed along the shared reference at the minimum, in metres per
    /// second: the passing agent's speed minus the passed body's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_speed_mps: Option<f64>,
}

impl ClosePassMinimum {
    /// The bucket's least observation, with its pair, time, and relative speed.
    fn reported(reading: ClosePassReading) -> Self {
        Self {
            status: MetricStatus::Reported,
            value: Some(reading.min_clearance_m),
            agent: Some(reading.agent),
            other: Some(reading.other),
            mode_pair: reading.mode_pair.map(str::to_owned),
            time_s: Some(reading.time_s),
            relative_speed_mps: Some(reading.relative_speed_mps),
        }
    }

    /// The minimum's no-value status: `not_applicable` for a bucket that cannot
    /// host a pass, `not_observed` for a bucket that could host one and recorded
    /// none.
    fn absent(status: MetricStatus) -> Self {
        Self {
            status,
            value: None,
            agent: None,
            other: None,
            mode_pair: None,
            time_s: None,
            relative_speed_mps: None,
        }
    }
}

/// The metric values one completed run reports (the artifact body).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Minimum time to collision in seconds over the run.
    pub minimum_ttc_s: MetricValue,
    /// Minimum surface separation in metres over the run.
    pub minimum_separation_m: MetricValue,
    /// Minimum post-encroachment time in seconds over the run.
    pub minimum_post_encroachment_s: MetricValue,
    /// Minimum surface separation in metres of each [`ModePair`].
    pub mode_pair_minimum_separation_m: BTreeMap<String, MetricValue>,
    /// Metric minima keyed by the pair's two movement keys.
    pub movement_minima: BTreeMap<String, MovementMinima>,
    /// Event counts with their mode and movement slices.
    pub event_counts: EventCounts,
    /// Throughput, delay, and queue values over the run, per mode, and per
    /// movement.
    pub operational: OperationalMetrics,
    /// The overtaking and close-pass families over the run, per mode pair,
    /// movement, and facility.
    ///
    /// [`RunMetricsArtifact`] serializes this block beside the others, so the
    /// families survive the immutable run directory and a later aggregation or
    /// comparison reads them from there.
    pub close_pass: ClosePassMetrics,
}

/// `metrics.json`: the metric values of one run, linked to its metric
/// definition revision and its manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunMetricsArtifact {
    /// The metric definition revision these values are reported at.
    pub metric_definition_version: u32,
    /// SHA-256 of the run manifest's exact bytes, which ties every value here to
    /// the provenance that produced it.
    pub manifest_sha256: String,
    /// Minimum time to collision in seconds over the run.
    pub minimum_ttc_s: MetricValue,
    /// Minimum surface separation in metres over the run.
    pub minimum_separation_m: MetricValue,
    /// Minimum post-encroachment time in seconds over the run.
    pub minimum_post_encroachment_s: MetricValue,
    /// Minimum surface separation in metres of each [`ModePair`].
    pub mode_pair_minimum_separation_m: BTreeMap<String, MetricValue>,
    /// Metric minima keyed by the pair's two movement keys.
    pub movement_minima: BTreeMap<String, MovementMinima>,
    /// Event counts with their mode and movement slices.
    pub event_counts: EventCounts,
    /// Throughput, delay, and queue values over the run, per mode, and per
    /// movement.
    pub operational: OperationalMetrics,
    /// The overtaking and close-pass families over the run, per mode pair,
    /// movement, and facility.
    pub close_pass: ClosePassMetrics,
}

impl RunMetricsArtifact {
    /// The artifact for a completed run, tied to the manifest bytes it
    /// describes.
    pub(crate) fn new(manifest_json: &str, metrics: &RunMetrics) -> Self {
        Self {
            metric_definition_version: METRIC_DEFINITION_VERSION,
            manifest_sha256: sha256_hex(manifest_json.as_bytes()),
            minimum_ttc_s: metrics.minimum_ttc_s.clone(),
            minimum_separation_m: metrics.minimum_separation_m.clone(),
            minimum_post_encroachment_s: metrics.minimum_post_encroachment_s.clone(),
            mode_pair_minimum_separation_m: metrics.mode_pair_minimum_separation_m.clone(),
            movement_minima: metrics.movement_minima.clone(),
            event_counts: metrics.event_counts.clone(),
            operational: metrics.operational.clone(),
            close_pass: metrics.close_pass.clone(),
        }
    }
}

/// Collects the metric and event counts of one run as the simulation advances.
///
/// The recorder counts each step's events and remembers every agent that
/// emitted one; [`Self::finish`] then reads the interaction-metric minima off
/// the live simulation, the close-pass families off its close-pass tracker, and
/// disaggregates both. Counting while the simulation is alive is what lets an
/// event be attributed to its agent's mode and movement, which the artifact
/// records and the canonical stream does not carry.
#[derive(Debug, Default)]
pub struct RunMetricsRecorder {
    /// Counted records per family.
    by_family: BTreeMap<&'static str, u64>,
    /// Counted records per family and variant kind.
    by_family_kind: BTreeMap<&'static str, BTreeMap<&'static str, u64>>,
    /// Counted records per family and subject agent.
    by_family_agent: BTreeMap<(&'static str, u32), u64>,
    /// Every agent that emitted a record, so pairs can be enumerated at finish.
    agents: BTreeSet<u32>,
    /// Counted overtaking maneuver records, before their mode, movement, and
    /// facility keys are resolved at finish.
    overtaking: BTreeMap<OvertakingRecord, u64>,
}

impl RunMetricsRecorder {
    /// A recorder that has counted nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Count one completed step's events.
    pub fn record(&mut self, output: &StepOutput<'_>) {
        for event in output.events() {
            self.record_event(event);
        }
    }

    /// Count one event and remember its subject agent.
    fn record_event(&mut self, event: &Event) {
        if let Some(record) = overtaking_record(event) {
            *self.overtaking.entry(record).or_insert(0) += 1;
        }
        let Some((family, kind)) = counted_family(event) else {
            return;
        };
        let agent = event_agent(event);
        *self.by_family.entry(family.label()).or_insert(0) += 1;
        if let Some(kind) = kind {
            *self
                .by_family_kind
                .entry(family.label())
                .or_default()
                .entry(kind)
                .or_insert(0) += 1;
        }
        *self
            .by_family_agent
            .entry((family.label(), agent.get()))
            .or_insert(0) += 1;
        self.agents.insert(agent.get());
    }

    /// Read the live simulation's metric minima and close the capture.
    ///
    /// Call before the simulation is consumed.
    pub fn finish(self, sim: &Simulation) -> RunMetrics {
        let interaction = sim.interaction_metrics();
        let operation = interaction.operation();
        let candidate_observed = interaction.minimum_separation_m().is_some();
        let region_entries = self
            .by_family
            .get(EventFamily::RegionEntries.label())
            .copied()
            .unwrap_or(0);

        RunMetrics {
            minimum_separation_m: MetricValue::minimum(
                interaction.minimum_separation_m(),
                MetricStatus::NotObserved,
            ),
            minimum_ttc_s: MetricValue::minimum(
                interaction.minimum_ttc_s(),
                absent_status(candidate_observed),
            ),
            minimum_post_encroachment_s: match interaction.minimum_post_encroachment_s() {
                Some(pet) => MetricValue {
                    status: MetricStatus::Reported,
                    value: Some(pet.seconds),
                    agent: Some(pet.preceding.get()),
                    other: Some(pet.following.get()),
                    mode_pair: mode_pair_label(sim, pet.preceding, pet.following)
                        .map(str::to_owned),
                    tick: None,
                },
                None => MetricValue::absent(absent_status(region_entries > 0)),
            },
            mode_pair_minimum_separation_m: mode_pair_minima(interaction),
            movement_minima: movement_minima(sim, interaction, &self.agents, region_entries > 0),
            event_counts: self.event_counts(sim),
            operational: operational_metrics(sim, operation),
            close_pass: self.close_pass_metrics(sim),
        }
    }

    /// The event counts and their mode and movement slices.
    fn event_counts(&self, sim: &Simulation) -> EventCounts {
        let mut by_family = BTreeMap::new();
        for family in EventFamily::ALL {
            by_family.insert(
                family.label().to_owned(),
                self.by_family.get(family.label()).copied().unwrap_or(0),
            );
        }
        let total = by_family.values().sum();

        let mut by_family_kind = BTreeMap::new();
        for family in [EventFamily::Violations, EventFamily::ControlTransitions] {
            let counted = self.by_family_kind.get(family.label());
            let mut kinds = BTreeMap::new();
            for kind in family.kinds() {
                kinds.insert(
                    (*kind).to_owned(),
                    counted.and_then(|map| map.get(kind)).copied().unwrap_or(0),
                );
            }
            by_family_kind.insert(family.label().to_owned(), kinds);
        }

        let mut by_family_mode: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
        let mut by_family_movement: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
        for ((family, agent), count) in &self.by_family_agent {
            let agent = AgentId::from_index(*agent as usize);
            if let Some(mode) = sim.agent_mode(agent) {
                *by_family_mode
                    .entry((*family).to_owned())
                    .or_default()
                    .entry(mode.label().to_owned())
                    .or_insert(0) += count;
            }
            if let Some(key) = movement_key(sim, agent) {
                *by_family_movement
                    .entry((*family).to_owned())
                    .or_default()
                    .entry(key)
                    .or_insert(0) += count;
            }
        }

        EventCounts {
            total,
            by_family,
            by_family_kind,
            by_family_mode,
            by_family_movement,
        }
    }

    /// The overtaking and close-pass families: the counted overtaking maneuver
    /// records, the closed observations the tracker holds, and the explicit
    /// applicability of every bucket.
    fn close_pass_metrics(&self, sim: &Simulation) -> ClosePassMetrics {
        let scenario = sim.scenario();
        let capability = pass_capability(scenario);
        let bands = scenario.clearance_bands();
        let mut buckets = ClosePassBuckets::default();
        let mut facility_applicability: BTreeMap<String, bool> = BTreeMap::new();

        // The mode pairs and facilities a scenario declares are the buckets that
        // could host a pass, so each exists with its explicit applicability even
        // when nothing was recorded in it; a movement bucket is named by a
        // counted record.
        for pair in [
            ModePair::VehicleVehicle,
            ModePair::VehiclePedestrian,
            ModePair::PedestrianPedestrian,
        ] {
            buckets.by_mode_pair.entry(pair).or_default();
        }
        for facility in scenario.facilities() {
            let Some(key) = facility_key(sim, facility.id()) else {
                continue;
            };
            buckets.by_facility.entry(key.clone()).or_default();
            facility_applicability.insert(key, facility_hosts_a_pass(facility.lateral_use()));
        }

        // The overtaking families: one count per counted maneuver record, keyed
        // by the maneuvering agent's mode and movement and its partner's.
        for (record, count) in &self.overtaking {
            let keys = overtaking_keys(sim, record);
            buckets.apply(&keys, |bucket| {
                *bucket.counted.entry(record.family).or_insert(0) += count;
            });
        }

        // The close-pass families: one contribution per closed observation.
        for observation in sim.close_pass_tracker().overtakes() {
            let keys = observation_keys(sim, observation);
            let reading = close_pass_reading(sim, observation);
            buckets.apply(&keys, |bucket| {
                bucket.observed = true;
                bucket.close_passes += 1;
                if !observation.violating_bands.is_empty() {
                    bucket.close_pass_violations += 1;
                }
                keep_reading(&mut bucket.minimum, reading);
                for band in &observation.bands {
                    *bucket.band_durations.entry(band.band.get()).or_insert(0.0) += band.duration_s;
                }
            });
        }

        ClosePassMetrics {
            run: buckets.run.finish(capability.any(), bands),
            by_mode_pair: buckets
                .by_mode_pair
                .into_iter()
                .map(|(pair, bucket)| {
                    (
                        pair.label().to_owned(),
                        bucket.finish(capability.pair(pair), bands),
                    )
                })
                .collect(),
            // A movement bucket carries no applicability predicate of its own:
            // v3 names none, so a bucket that recorded nothing reports the
            // no-observation status rather than the inapplicable one.
            by_movement: buckets
                .by_movement
                .into_iter()
                .map(|(key, bucket)| (key, bucket.finish(true, bands)))
                .collect(),
            by_facility: buckets
                .by_facility
                .into_iter()
                .map(|(key, bucket)| {
                    let applicable = facility_applicability.get(&key).copied().unwrap_or(false);
                    (key, bucket.finish(applicable, bands))
                })
                .collect(),
        }
    }
}

/// The four overtaking families of metric definition v3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OvertakingFamily {
    Attempts,
    Commits,
    Completions,
    Aborts,
}

/// One counted overtaking maneuver record, before its mode, movement, and
/// facility keys are resolved at finish.
///
/// The record keeps the whole attribution a bucket needs: the maneuvering
/// agent, the partner the family is keyed by when there is one, and the facility
/// the maneuver started on, which is the facility key metric definition v3 uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct OvertakingRecord {
    family: OvertakingFamily,
    agent: u32,
    partner: Option<u32>,
    source_facility: u32,
}

/// The counted overtaking family of a maneuver record, or `None` when the record
/// is not one metric definition v3 counts.
///
/// The four families are the `TacticKind::Overtake` edges: the attempt
/// (`following -> preparing`), the commit (`preparing -> committed`), the
/// completion (`committed -> returning`), and the abort (an edge into
/// `aborted`). The `returning -> following` edge completes the return rather
/// than the pass, so it is no family's record.
fn overtaking_record(event: &Event) -> Option<OvertakingRecord> {
    let Event::Maneuver {
        agent,
        kind,
        from,
        to,
        edge,
        partner,
        source_facility,
        ..
    } = event
    else {
        return None;
    };
    if *kind != TacticKind::Overtake {
        return None;
    }
    let family = match (*edge, *from, *to) {
        (ManeuverEdge::Attempted, ..) => OvertakingFamily::Attempts,
        (ManeuverEdge::Committed, ..) => OvertakingFamily::Commits,
        (ManeuverEdge::Completed, ManeuverState::Committed, ManeuverState::Returning) => {
            OvertakingFamily::Completions
        }
        (ManeuverEdge::Aborted, ..) => OvertakingFamily::Aborts,
        (ManeuverEdge::Completed, ..) => return None,
    };
    Some(OvertakingRecord {
        family,
        agent: agent.get(),
        partner: partner.map(AgentId::get),
        source_facility: source_facility.get(),
    })
}

/// One bucket's least close-pass observation, before its statuses resolve.
#[derive(Debug, Clone, Copy)]
struct ClosePassReading {
    min_clearance_m: f64,
    time_s: f64,
    relative_speed_mps: f64,
    agent: u32,
    other: u32,
    mode_pair: Option<&'static str>,
}

/// Keep the least reading seen, preferring the first on a tie.
fn keep_reading(slot: &mut Option<ClosePassReading>, candidate: ClosePassReading) {
    if slot.is_none_or(|current| candidate.min_clearance_m < current.min_clearance_m) {
        *slot = Some(candidate);
    }
}

/// The least-observation reading of one closed observation.
fn close_pass_reading(sim: &Simulation, observation: &OvertakeObservation) -> ClosePassReading {
    ClosePassReading {
        min_clearance_m: observation.min_clearance_m,
        time_s: observation.min_clearance_time_s,
        relative_speed_mps: observation.relative_speed_mps,
        agent: observation.agent.get(),
        other: observation.partner.get(),
        mode_pair: mode_pair_label(sim, observation.agent, observation.partner),
    }
}

/// Whether a compiled mode class declares the pass capability a bucket needs to
/// host a pass, per mode class.
#[derive(Debug, Clone, Copy, Default)]
struct PassCapability {
    vehicle: bool,
    pedestrian: bool,
}

impl PassCapability {
    /// Whether a whole run can host a pass: any mode class declares the
    /// capability.
    const fn any(self) -> bool {
        self.vehicle || self.pedestrian
    }

    /// Whether a mode-pair bucket can host a pass.
    ///
    /// Metric definition v3's predicate is "neither body's compiled mode
    /// template declares the `overtake` or `pass` tactic", so a pair is
    /// applicable when either body's mode class declares it, whichever side of
    /// the pair already recorded the capability.
    const fn pair(self, pair: ModePair) -> bool {
        match pair {
            ModePair::VehicleVehicle => self.vehicle,
            ModePair::VehiclePedestrian => self.vehicle || self.pedestrian,
            ModePair::PedestrianPedestrian => self.pedestrian,
        }
    }
}

/// The pass capability the compiled scenario's mode templates declare.
///
/// The class of a template is the one the kernel already derives it from: a
/// holonomic circle is a pedestrian body and carries no route coordinates, and
/// every wheeled family is a vehicle ([[TAS-140-accumulate-close-pass-metric-families]]
/// reads the same predicate `Simulation::close_pass_event` reads).
fn pass_capability(scenario: &hekate_model::CompiledScenario) -> PassCapability {
    let mut capability = PassCapability::default();
    for template in scenario.mode_templates() {
        let tactics = template.tactics();
        if !(tactics.supports(TacticalCapability::Pass)
            || tactics.supports(TacticalCapability::Overtake))
        {
            continue;
        }
        if template.family() == Some(AgentFamily::HolonomicCircle) {
            capability.pedestrian = true;
        } else {
            capability.vehicle = true;
        }
    }
    capability
}

/// Whether a compiled facility offers the free lateral motion a pass needs.
///
/// Metric definition v3 makes a `centered` facility — one whose bodies travel
/// nominally on the reference with no lateral freedom — a bucket that cannot
/// host a pass.
const fn facility_hosts_a_pass(lateral_use: LateralUse) -> bool {
    !matches!(lateral_use, LateralUse::Centered)
}

/// The report spelling of a compiled facility: `facility:<name>`.
fn facility_key(sim: &Simulation, facility: FacilityId) -> Option<String> {
    sim.scenario()
        .facility(facility)
        .map(|facility| format!("facility:{}", facility.name()))
}

/// The disaggregation keys one counted record contributes to.
///
/// An absent key means the record cannot be attributed on that dimension: a
/// maneuver record with no partner has no mode pair and no pair movement key,
/// and an observation with no shared facility has no facility bucket.
#[derive(Debug, Clone, Default)]
struct ClosePassKeys {
    mode_pair: Option<ModePair>,
    movement: Option<String>,
    facility: Option<String>,
}

/// One bucket's accumulated close-pass evidence, before its statuses resolve.
#[derive(Debug, Default)]
struct ClosePassAccumulator {
    /// Counted overtaking records by family.
    counted: BTreeMap<OvertakingFamily, u64>,
    /// Closed observations in the bucket.
    close_passes: u64,
    /// Observations whose violating-band list is not empty.
    close_pass_violations: u64,
    /// The least observation, with the time and relative speed at its minimum.
    minimum: Option<ClosePassReading>,
    /// Seconds each participating band accumulated, by band id.
    band_durations: BTreeMap<u32, f64>,
    /// Whether the bucket recorded a closed observation.
    observed: bool,
}

impl ClosePassAccumulator {
    /// The bucket's reported families: every count as a count, and the two value
    /// families with the status the bucket's applicability and observation give
    /// them.
    ///
    /// `hosts_a_pass` is the bucket's applicability: `false` for a bucket metric
    /// definition v3 fixes as unable to host a pass.
    fn finish(self, hosts_a_pass: bool, bands: &[CompiledClearanceBand]) -> ClosePassValues {
        let Self {
            counted,
            close_passes,
            close_pass_violations,
            minimum,
            band_durations,
            observed,
        } = self;
        let count = |family: OvertakingFamily| {
            MetricValue::reported_run(counted.get(&family).copied().unwrap_or(0) as f64)
        };
        let minimum = match minimum {
            Some(reading) => ClosePassMinimum::reported(reading),
            None if hosts_a_pass => ClosePassMinimum::absent(MetricStatus::NotObserved),
            None => ClosePassMinimum::absent(MetricStatus::NotApplicable),
        };
        let durations = bands
            .iter()
            .map(|band| {
                let value = match band_durations.get(&band.id().get()) {
                    Some(seconds) => MetricValue::reported_run(*seconds),
                    // A band no observation of a bucket participated in is
                    // excluded by its own mode gate, which is no applicable
                    // value for that bucket; a bucket with no observation at all
                    // is either one that cannot host a pass or one that recorded
                    // none.
                    None if observed || !hosts_a_pass => MetricValue::not_applicable(),
                    None => MetricValue::not_observed(),
                };
                (band.id().get(), value)
            })
            .collect();
        ClosePassValues {
            overtake_attempts: count(OvertakingFamily::Attempts),
            overtake_commits: count(OvertakingFamily::Commits),
            overtake_completions: count(OvertakingFamily::Completions),
            overtake_aborts: count(OvertakingFamily::Aborts),
            close_passes: MetricValue::reported_run(close_passes as f64),
            close_pass_minimum_clearance_m: minimum,
            clearance_band_durations_s: durations,
            close_pass_violations: MetricValue::reported_run(close_pass_violations as f64),
        }
    }
}

/// The close-pass accumulators: one over the run and one per bucket of each
/// disaggregation dimension.
#[derive(Debug, Default)]
struct ClosePassBuckets {
    run: ClosePassAccumulator,
    by_mode_pair: BTreeMap<ModePair, ClosePassAccumulator>,
    by_movement: BTreeMap<String, ClosePassAccumulator>,
    by_facility: BTreeMap<String, ClosePassAccumulator>,
}

impl ClosePassBuckets {
    /// Apply one contribution to the run bucket and to every bucket its keys
    /// name.
    fn apply(&mut self, keys: &ClosePassKeys, update: impl Fn(&mut ClosePassAccumulator)) {
        update(&mut self.run);
        if let Some(pair) = keys.mode_pair {
            update(self.by_mode_pair.entry(pair).or_default());
        }
        if let Some(movement) = &keys.movement {
            update(self.by_movement.entry(movement.clone()).or_default());
        }
        if let Some(facility) = &keys.facility {
            update(self.by_facility.entry(facility.clone()).or_default());
        }
    }
}

/// The disaggregation keys of one closed observation.
fn observation_keys(sim: &Simulation, observation: &OvertakeObservation) -> ClosePassKeys {
    ClosePassKeys {
        mode_pair: pair_mode_pair(sim, observation.agent, observation.partner),
        movement: pair_movement_key(sim, observation.agent, observation.partner),
        facility: observation
            .facility
            .and_then(|facility| facility_key(sim, facility)),
    }
}

/// The disaggregation keys of one counted overtaking record.
///
/// A record with no `partner` has no mode pair and is keyed on the movement
/// dimension by the maneuvering agent's own movement key alone, exactly as
/// metric definition v3 fixes; its facility key is the `source_facility` the
/// record carries.
fn overtaking_keys(sim: &Simulation, record: &OvertakingRecord) -> ClosePassKeys {
    let agent = AgentId::from_index(record.agent as usize);
    let partner = record
        .partner
        .map(|partner| AgentId::from_index(partner as usize));
    ClosePassKeys {
        mode_pair: partner.and_then(|partner| pair_mode_pair(sim, agent, partner)),
        movement: match partner {
            Some(partner) => pair_movement_key(sim, agent, partner),
            None => movement_key(sim, agent),
        },
        facility: facility_key(sim, FacilityId::from_index(record.source_facility as usize)),
    }
}

/// The [`ModePair`] of two agents, if both modes are known.
fn pair_mode_pair(sim: &Simulation, first: AgentId, second: AgentId) -> Option<ModePair> {
    Some(ModePair::of(
        sim.agent_mode(first)?,
        sim.agent_mode(second)?,
    ))
}

/// The pairwise movement key of two bodies: the two movement keys sorted and
/// joined, so a pair and its mirror share one bucket. `None` when either body
/// carries no assigned movement.
fn pair_movement_key(sim: &Simulation, first: AgentId, second: AgentId) -> Option<String> {
    let first = movement_key(sim, first)?;
    let second = movement_key(sim, second)?;
    Some(bucket_key(&first, &second))
}

/// The status of an applicable metric with no recorded value.
fn absent_status(applicable: bool) -> MetricStatus {
    if applicable {
        MetricStatus::NotApplicable
    } else {
        MetricStatus::NotObserved
    }
}

/// The mode-pair label of two agents, if both modes are known.
fn mode_pair_label(sim: &Simulation, first: AgentId, second: AgentId) -> Option<&'static str> {
    Some(pair_mode_pair(sim, first, second)?.label())
}

/// The separation minimum of every mode pair.
fn mode_pair_minima(interaction: &InteractionMetrics) -> BTreeMap<String, MetricValue> {
    [
        ModePair::VehicleVehicle,
        ModePair::VehiclePedestrian,
        ModePair::PedestrianPedestrian,
    ]
    .into_iter()
    .map(|pair| {
        (
            pair.label().to_owned(),
            MetricValue::minimum(
                interaction.mode_pair_minimum_separation_m(pair),
                MetricStatus::NotObserved,
            ),
        )
    })
    .collect()
}

/// The metric minima keyed by each pair's two movement keys.
fn movement_minima(
    sim: &Simulation,
    interaction: &InteractionMetrics,
    agents: &BTreeSet<u32>,
    region_observed: bool,
) -> BTreeMap<String, MovementMinima> {
    let mut buckets: BTreeMap<String, MovementAccumulator> = BTreeMap::new();

    // Pair metrics: the kernel's per-pair minima over every pair whose bodies
    // both carry a movement key. A pair that was never a candidate records no
    // separation and no time to collision, so it is skipped.
    let agents: Vec<u32> = agents.iter().copied().collect();
    for (position, &first) in agents.iter().enumerate() {
        for &second in &agents[position + 1..] {
            let first = AgentId::from_index(first as usize);
            let second = AgentId::from_index(second as usize);
            let separation = interaction.pair_minimum_separation_m(first, second);
            let ttc = interaction.pair_minimum_ttc_s(first, second);
            if separation.is_none() && ttc.is_none() {
                continue;
            }
            let (Some(first_key), Some(second_key)) =
                (movement_key(sim, first), movement_key(sim, second))
            else {
                continue;
            };
            let (Some(first_mode), Some(second_mode)) =
                (sim.agent_mode(first), sim.agent_mode(second))
            else {
                continue;
            };
            let mode_pair = ModePair::of(first_mode, second_mode);
            let bucket = buckets
                .entry(bucket_key(&first_key, &second_key))
                .or_insert_with(|| MovementAccumulator::new(first_key, second_key));
            if let Some(value) = separation {
                keep_minimum(
                    &mut bucket.separation,
                    PairMinimum {
                        value,
                        agent: first.get(),
                        other: second.get(),
                        mode_pair,
                    },
                );
            }
            if let Some(value) = ttc {
                keep_minimum(
                    &mut bucket.ttc,
                    PairMinimum {
                        value,
                        agent: first.get(),
                        other: second.get(),
                        mode_pair,
                    },
                );
            }
        }
    }

    // Post-encroachment times: keyed by the preceding and following bodies'
    // movement keys, exactly like the pair metrics.
    for pet in interaction.post_encroachments() {
        let (Some(preceding_key), Some(following_key)) = (
            movement_key(sim, pet.preceding),
            movement_key(sim, pet.following),
        ) else {
            continue;
        };
        let (Some(preceding_mode), Some(following_mode)) =
            (sim.agent_mode(pet.preceding), sim.agent_mode(pet.following))
        else {
            continue;
        };
        let bucket = buckets
            .entry(bucket_key(&preceding_key, &following_key))
            .or_insert_with(|| MovementAccumulator::new(preceding_key, following_key));
        keep_minimum(
            &mut bucket.pet,
            PairMinimum {
                value: pet.seconds,
                agent: pet.preceding.get(),
                other: pet.following.get(),
                mode_pair: ModePair::of(preceding_mode, following_mode),
            },
        );
    }

    buckets
        .into_iter()
        .map(|(key, accumulator)| (key, accumulator.finish(region_observed)))
        .collect()
}

/// One bucket's least pair or succession value.
#[derive(Debug, Clone, Copy)]
struct PairMinimum {
    value: f64,
    agent: u32,
    other: u32,
    mode_pair: ModePair,
}

/// Keep the least value seen, preferring the first on a tie.
fn keep_minimum(slot: &mut Option<PairMinimum>, candidate: PairMinimum) {
    if slot.is_none_or(|current| candidate.value < current.value) {
        *slot = Some(candidate);
    }
}

/// The running least values of one movement bucket.
#[derive(Debug)]
struct MovementAccumulator {
    keys: [String; 2],
    separation: Option<PairMinimum>,
    ttc: Option<PairMinimum>,
    pet: Option<PairMinimum>,
}

impl MovementAccumulator {
    fn new(first: String, second: String) -> Self {
        let keys = if first <= second {
            [first, second]
        } else {
            [second, first]
        };
        Self {
            keys,
            separation: None,
            ttc: None,
            pet: None,
        }
    }

    fn finish(self, region_observed: bool) -> MovementMinima {
        let candidate = self.separation.is_some() || self.ttc.is_some();
        MovementMinima {
            movement_keys: self.keys,
            minimum_separation_m: match self.separation {
                Some(minimum) => MetricValue::reported_pair(minimum),
                None => MetricValue::not_observed(),
            },
            minimum_ttc_s: match self.ttc {
                Some(minimum) => MetricValue::reported_pair(minimum),
                None if candidate => MetricValue::not_applicable(),
                None => MetricValue::not_observed(),
            },
            minimum_post_encroachment_s: match self.pet {
                Some(minimum) => MetricValue::reported_pair(minimum),
                None if region_observed => MetricValue::not_applicable(),
                None => MetricValue::not_observed(),
            },
        }
    }
}

/// The movement key of an agent: the tagged union of `MovementId` and
/// `PedestrianRouteId` that metric definition v1 chose.
fn movement_key(sim: &Simulation, agent: AgentId) -> Option<String> {
    operational_movement_key(sim, agent_movement_key(sim, agent)?)
}

/// The movement identity of an agent, as the kernel's operational observer keys
/// its movement buckets.
fn agent_movement_key(sim: &Simulation, agent: AgentId) -> Option<MovementKey> {
    match sim.agent_mode(agent)? {
        AgentMode::Vehicle => sim.agent_route(agent).map(MovementKey::Vehicle),
        AgentMode::Pedestrian => sim
            .agent_pedestrian_route(agent)
            .map(MovementKey::Pedestrian),
    }
}

/// The report spelling of one movement key, identical to the spelling a pair
/// bucket uses for the same movement.
fn operational_movement_key(sim: &Simulation, key: MovementKey) -> Option<String> {
    match key {
        MovementKey::Vehicle(movement) => sim
            .scenario()
            .movement(movement)
            .map(|movement| format!("movement:{}", movement.name())),
        MovementKey::Pedestrian(route) => sim
            .scenario()
            .pedestrian_route(route)
            .map(|route| format!("pedestrian_route:{}", route.name())),
    }
}

/// The operational values of the run, of each mode, and of each observed
/// movement, spelled as their artifact keys.
fn operational_metrics(
    sim: &Simulation,
    operation: &hekate_sim::OperationMetrics,
) -> OperationalMetrics {
    OperationalMetrics {
        run: OperationalValues::of(&operation.values()),
        by_mode: [AgentMode::Vehicle, AgentMode::Pedestrian]
            .into_iter()
            .map(|mode| {
                (
                    mode.label().to_owned(),
                    OperationalValues::of(&operation.mode_values(mode)),
                )
            })
            .collect(),
        by_movement: operation
            .movements()
            .filter_map(|(key, values)| {
                let spelled = operational_movement_key(sim, key)?;
                Some((spelled, OperationalValues::of(&values)))
            })
            .collect(),
    }
}

/// The bucket key of two movement keys: sorted lexicographically and joined,
/// so a pair and its mirror share one bucket.
fn bucket_key(first: &str, second: &str) -> String {
    if first <= second {
        format!("{first}|{second}")
    } else {
        format!("{second}|{first}")
    }
}

/// The countable family and variant kind of an event, or `None` when the
/// event's edge is not a counted record.
///
/// The counted edges are metric definition v1's: a collision is counted when it
/// is contacting, a near miss when it is entering, a queue event when it is
/// joined, and a yield when it is yielding; every violation, region entry or
/// exit, control transition, spawn, and despawn is counted.
fn counted_family(event: &Event) -> Option<(EventFamily, Option<&'static str>)> {
    match event {
        Event::Collision { contacting, .. } => {
            (*contacting).then_some((EventFamily::Collisions, None))
        }
        Event::NearMiss { entering, .. } => (*entering).then_some((EventFamily::NearMisses, None)),
        Event::Violation { kind, .. } => Some((EventFamily::Violations, Some(kind.label()))),
        Event::Entry { .. } => Some((EventFamily::RegionEntries, None)),
        Event::Exit { .. } => Some((EventFamily::RegionExits, None)),
        Event::Queue { joined, .. } => (*joined).then_some((EventFamily::QueueEvents, None)),
        Event::ControlTransition { control, .. } => {
            Some((EventFamily::ControlTransitions, Some(control.label())))
        }
        Event::Yielded { yielding, .. } => (*yielding).then_some((EventFamily::Yields, None)),
        Event::Spawned { .. } => Some((EventFamily::Spawns, None)),
        Event::Despawned { .. } => Some((EventFamily::Despawns, None)),
        // The increment-2 maneuver and rule records carry no metric definition
        // v1 family: the overtaking counts are read by [`overtaking_record`],
        // the close-pass families off the tracker's closed observations, and the
        // wrong-way families by their own leaf.
        Event::Maneuver { .. }
        | Event::FacilityTransition { .. }
        | Event::OpposingTraversal { .. }
        | Event::ClosePass { .. } => None,
    }
}

/// The subject agent of an event, the agent its record attributes it to.
const fn event_agent(event: &Event) -> AgentId {
    match event {
        Event::Spawned { agent, .. }
        | Event::Despawned { agent, .. }
        | Event::Yielded { agent, .. }
        | Event::Collision { agent, .. }
        | Event::NearMiss { agent, .. }
        | Event::Violation { agent, .. }
        | Event::Entry { agent, .. }
        | Event::Exit { agent, .. }
        | Event::Queue { agent, .. }
        | Event::ControlTransition { agent, .. }
        | Event::Maneuver { agent, .. }
        | Event::FacilityTransition { agent, .. }
        | Event::OpposingTraversal { agent, .. }
        | Event::ClosePass { agent, .. } => *agent,
    }
}

#[cfg(test)]
mod tests {
    use hekate_model::CrossingId;
    use hekate_sim::{ManeuverReasonCode, PassSide};

    use super::*;

    /// One `TacticKind::Overtake` maneuver record of the given edge, as the
    /// kernel emits it.
    fn maneuver(
        kind: TacticKind,
        edge: ManeuverEdge,
        from: ManeuverState,
        to: ManeuverState,
    ) -> Event {
        Event::Maneuver {
            agent: AgentId::from_index(0),
            kind,
            from,
            to,
            edge,
            partner: Some(AgentId::from_index(1)),
            source_facility: FacilityId::from_index(0),
            target_facility: None,
            target_offset_m: 1.0,
            side: PassSide::Left,
            reason: ManeuverReasonCode::SlowerLeader,
        }
    }

    /// The counted overtaking families are exactly the `TacticKind::Overtake`
    /// edges metric definition v3 names, and the return's own completion is no
    /// family's record.
    #[test]
    fn the_counted_overtaking_families_are_the_v3_edges() {
        let family = |event: &Event| overtaking_record(event).map(|record| record.family);
        assert_eq!(
            family(&maneuver(
                TacticKind::Overtake,
                ManeuverEdge::Attempted,
                ManeuverState::Following,
                ManeuverState::Preparing,
            )),
            Some(OvertakingFamily::Attempts)
        );
        assert_eq!(
            family(&maneuver(
                TacticKind::Overtake,
                ManeuverEdge::Committed,
                ManeuverState::Preparing,
                ManeuverState::Committed,
            )),
            Some(OvertakingFamily::Commits)
        );
        assert_eq!(
            family(&maneuver(
                TacticKind::Overtake,
                ManeuverEdge::Completed,
                ManeuverState::Committed,
                ManeuverState::Returning,
            )),
            Some(OvertakingFamily::Completions)
        );
        assert_eq!(
            family(&maneuver(
                TacticKind::Overtake,
                ManeuverEdge::Completed,
                ManeuverState::Returning,
                ManeuverState::Following,
            )),
            None,
            "the return's completion is not the pass's"
        );
        assert_eq!(
            family(&maneuver(
                TacticKind::Overtake,
                ManeuverEdge::Aborted,
                ManeuverState::Committed,
                ManeuverState::Aborted,
            )),
            Some(OvertakingFamily::Aborts)
        );
        assert_eq!(
            family(&maneuver(
                TacticKind::Follow,
                ManeuverEdge::Attempted,
                ManeuverState::Following,
                ManeuverState::Preparing,
            )),
            None,
            "another tactic's maneuver is no overtaking family"
        );
        assert_eq!(
            family(&Event::Yielded {
                agent: AgentId::from_index(0),
                crossing: CrossingId::from_index(0),
                yielding: true,
            }),
            None
        );
    }

    /// The published family list is the counted family set, in order, so a
    /// consumer that enumerates the labels covers every family exactly once.
    #[test]
    fn the_published_family_labels_are_the_counted_families() {
        assert_eq!(
            EVENT_FAMILY_LABELS,
            EventFamily::ALL.map(|family| family.label())
        );
    }

    /// The bucket key sorts its two movement keys, so a pair and its mirror
    /// share one bucket.
    #[test]
    fn a_bucket_key_is_symmetric_in_its_movement_keys() {
        assert_eq!(
            bucket_key("movement:a", "pedestrian_route:b"),
            "movement:a|pedestrian_route:b"
        );
        assert_eq!(
            bucket_key("pedestrian_route:b", "movement:a"),
            "movement:a|pedestrian_route:b"
        );
        assert_eq!(
            bucket_key("movement:a", "movement:a"),
            "movement:a|movement:a"
        );
    }

    /// The artifact, including its absent-value statuses, round-trips through
    /// JSON.
    #[test]
    fn the_artifact_round_trips_through_json() {
        let artifact = RunMetricsArtifact {
            metric_definition_version: METRIC_DEFINITION_VERSION,
            manifest_sha256: "0".repeat(64),
            minimum_ttc_s: MetricValue::not_applicable(),
            minimum_separation_m: MetricValue::reported_pair(PairMinimum {
                value: 1.5,
                agent: 0,
                other: 1,
                mode_pair: ModePair::VehicleVehicle,
            }),
            minimum_post_encroachment_s: MetricValue::not_observed(),
            mode_pair_minimum_separation_m: BTreeMap::new(),
            movement_minima: BTreeMap::new(),
            event_counts: EventCounts {
                total: 0,
                by_family: BTreeMap::new(),
                by_family_kind: BTreeMap::new(),
                by_family_mode: BTreeMap::new(),
                by_family_movement: BTreeMap::new(),
            },
            operational: OperationalMetrics {
                run: OperationalValues::not_observed(),
                by_mode: ["vehicle", "pedestrian"]
                    .into_iter()
                    .map(|mode| (mode.to_owned(), OperationalValues::not_observed()))
                    .collect(),
                by_movement: BTreeMap::new(),
            },
            close_pass: ClosePassMetrics::not_observed(),
        };
        let json = serde_json::to_string_pretty(&artifact).expect("artifact serializes");
        assert!(json.contains("\"status\": \"not_applicable\""));
        assert!(
            !json.contains("\"value\": null"),
            "an absent value is omitted"
        );
        let decoded: RunMetricsArtifact = serde_json::from_str(&json).expect("artifact parses");
        assert_eq!(decoded, artifact);
    }
}
