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
//! The artifact carries `metric_definition_version: 2` and the run's
//! `manifest_sha256`, so every number ties back to the versioned definition
//! ([[DEF-005-metric-definition-v2]], which carries
//! [[DEF-004-metric-definition-v1]] forward) and to the manifest that produced
//! it. It holds:
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
//!   movement, each with its unit, its status, and its provenance.
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

use serde::{Deserialize, Serialize};
use tangle_sim::{
    AgentId, AgentMode, Event, InteractionMetrics, MetricMinimum, ModePair, MovementKey,
    OperationValues as ObservedValues, Simulation, StepOutput,
};

use crate::trace::sha256_hex;

/// File name of the metrics artifact inside a run directory.
pub const METRICS_FILE: &str = "metrics.json";

/// The metric definition revision this artifact reports.
///
/// Fixed at `2` by [[DEF-005-metric-definition-v2]], which carries metric
/// definition v1 ([[DEF-004-metric-definition-v1]]) forward unchanged and adds
/// the operational families — throughput, delay, and queues — that v1
/// explicitly deferred. A change to any reported metric's name, formula, unit,
/// applicability, tie-break, or disaggregation key bumps this with the code
/// change, and an artifact already written stays attributed to the revision that
/// produced it.
pub const METRIC_DEFINITION_VERSION: u32 = 2;

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
        }
    }
}

/// Collects the metric and event counts of one run as the simulation advances.
///
/// The recorder counts each step's events and remembers every agent that
/// emitted one; [`Self::finish`] then reads the interaction-metric minima off
/// the live simulation and disaggregates them. Counting while the simulation is
/// alive is what lets an event be attributed to its agent's mode and movement,
/// which the artifact records and the canonical stream does not carry.
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
        let Some((family, kind)) = counted_family(event) else {
            return;
        };
        let agent = event_agent(*event);
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
                    mode_pair: mode_pair_label(sim, pet.preceding, pet.following),
                    tick: None,
                },
                None => MetricValue::absent(absent_status(region_entries > 0)),
            },
            mode_pair_minimum_separation_m: mode_pair_minima(interaction),
            movement_minima: movement_minima(sim, interaction, &self.agents, region_entries > 0),
            event_counts: self.event_counts(sim),
            operational: operational_metrics(sim, operation),
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
fn mode_pair_label(sim: &Simulation, first: AgentId, second: AgentId) -> Option<String> {
    let first = sim.agent_mode(first)?;
    let second = sim.agent_mode(second)?;
    Some(ModePair::of(first, second).label().to_owned())
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
    operation: &tangle_sim::OperationMetrics,
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
    match *event {
        Event::Collision { contacting, .. } => {
            contacting.then_some((EventFamily::Collisions, None))
        }
        Event::NearMiss { entering, .. } => entering.then_some((EventFamily::NearMisses, None)),
        Event::Violation { kind, .. } => Some((EventFamily::Violations, Some(kind.label()))),
        Event::Entry { .. } => Some((EventFamily::RegionEntries, None)),
        Event::Exit { .. } => Some((EventFamily::RegionExits, None)),
        Event::Queue { joined, .. } => joined.then_some((EventFamily::QueueEvents, None)),
        Event::ControlTransition { control, .. } => {
            Some((EventFamily::ControlTransitions, Some(control.label())))
        }
        Event::Yielded { yielding, .. } => yielding.then_some((EventFamily::Yields, None)),
        Event::Spawned { .. } => Some((EventFamily::Spawns, None)),
        Event::Despawned { .. } => Some((EventFamily::Despawns, None)),
    }
}

/// The subject agent of an event, the agent its record attributes it to.
const fn event_agent(event: Event) -> AgentId {
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
        | Event::ControlTransition { agent, .. } => agent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
