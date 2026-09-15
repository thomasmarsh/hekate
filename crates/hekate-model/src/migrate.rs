//! Deterministic version-1 to version-2 migration.
//!
//! The transform is a pure function of the authored version-1 document: it
//! reads no clock, no randomness, and no file, so an unchanged source always
//! rewrites to the same normalized bytes.
//! `docs/schema-v2-contract.md` fixes the exact mapping this module implements;
//! change it only with that document.

use std::collections::BTreeMap;

use crate::source::{
    AccessSource, DemandChoiceSource, DemandPopulationSpawnSource, DemandRateSpawnSource,
    DemandSourceV2, DemandSpawnSource, FacilityKind, ModeBodySource, ModeTemplateSource,
    MotionKind, MovementDirection, MovementSource, MovementSourceV2, OccupancyKind, PathEnd,
    PortalSource, ProfileRangeSource, SUPPORTED_SCHEMA_VERSION, ScenarioSource, ScenarioSourceV2,
    TacticKind, TimeIntervalSource,
};

/// Version of the version-1 to version-2 migration transform.
///
/// A source that is already version 2 is recorded as migration version `0` (no
/// migration applied). Every change to the mapping in [`migrate_v1_to_v2`]
/// increments this number, so a run can be attributed to the exact transform
/// that produced it.
pub const MIGRATION_VERSION: u32 = 1;

/// Id of the passenger-car mode template the migration always emits.
const PASSENGER_CAR_TEMPLATE_ID: &str = "passenger_car";

/// Id of the pedestrian mode template the migration always emits.
const PEDESTRIAN_TEMPLATE_ID: &str = "pedestrian";

/// Id of the walking-skeleton population demand entry.
const POPULATION_DEMAND_ID: &str = "population";

/// Rewrite a version-1 scenario document as its normalized version-2 form.
///
/// The mapping is total and deterministic for every version-1 document; the
/// result is canonical once [`to_canonical_v2_json`] serializes it. For a
/// document that names a `from` portal the version-1 validator would reject,
/// the movement direction falls back to `forward` rather than failing here,
/// because validation belongs to the reader, not the transform.
pub fn migrate_v1_to_v2(source: &ScenarioSource) -> ScenarioSourceV2 {
    // The walking-skeleton population is used exactly when the source declares
    // no demand of either mode; otherwise the population is unused.
    let population_in_use = source.demand.is_empty() && source.pedestrian_demand.is_empty();

    ScenarioSourceV2 {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        id: source.id.clone(),
        coordinate_system: source.coordinate_system.clone(),
        paths: source.paths.clone(),
        portals: source.portals.clone(),
        boundaries: source.boundaries.clone(),
        regions: source.regions.clone(),
        movements: source
            .movements
            .iter()
            .map(|movement| migrate_movement(movement, &source.portals))
            .collect(),
        crossings: source.crossings.clone(),
        waiting_areas: source.waiting_areas.clone(),
        pedestrian_routes: source.pedestrian_routes.clone(),
        conflict_regions: source.conflict_regions.clone(),
        rules: source.rules.clone(),
        signals: source.signals.clone(),
        mode_templates: vec![
            passenger_car_template(source, population_in_use),
            pedestrian_template(source),
        ],
        facilities: Vec::new(),
        facility_connectors: Vec::new(),
        facility_adjacencies: Vec::new(),
        permissions: Vec::new(),
        clearance_bands: Vec::new(),
        maneuver_policy: None,
        demand: migrate_demand(source, population_in_use),
    }
}

/// Serialize a version-2 source in the contract's canonical normalized form.
///
/// The form is canonical JSON: 2-space indentation, the version-2 field order,
/// and a trailing newline — the same form `Baseline::to_pretty_json` writes. It
/// is deterministic, so re-serializing a parsed document reproduces exactly
/// these bytes and their SHA-256 identifies the normalized document.
pub fn to_canonical_v2_json(source: &ScenarioSourceV2) -> String {
    let mut json = serde_json::to_string_pretty(source).expect("a version-2 source serializes");
    json.push('\n');
    json
}

/// Carry one version-1 movement forward with its explicit nominal direction.
fn migrate_movement(movement: &MovementSource, portals: &[PortalSource]) -> MovementSourceV2 {
    MovementSourceV2 {
        id: movement.id.clone(),
        from: movement.from.clone(),
        to: movement.to.clone(),
        path: movement.path.clone(),
        priority: movement.priority,
        stop_line_m: movement.stop_line_m,
        direction: movement_direction(movement, portals),
    }
}

/// The direction the movement's `from` portal fixes on its reference path.
///
/// The version-1 validator already guarantees the two portals attach to
/// opposite ends of the movement's path, so this derivation is single-valued;
/// an unknown portal falls back to `forward`.
fn movement_direction(movement: &MovementSource, portals: &[PortalSource]) -> MovementDirection {
    match portals.iter().find(|portal| portal.id == movement.from) {
        Some(portal) if portal.end == PathEnd::End => MovementDirection::Reverse,
        _ => MovementDirection::Forward,
    }
}

/// The Increment 0 passenger-car template, with the resolved version-1 profiles
/// materialized into it.
///
/// When the source uses the walking-skeleton population, the population governs
/// the vehicle body and speed, so those replace the profile values; the
/// population does not constrain acceleration, braking, time gap, or
/// compliance, so those keep their profile values.
fn passenger_car_template(source: &ScenarioSource, population_in_use: bool) -> ModeTemplateSource {
    let profile = &source.profiles;
    let population = &source.population;
    let (length_m, width_m, speed_mps) = if population_in_use {
        (
            constant_range(population.vehicle_length_m),
            constant_range(population.vehicle_width_m),
            constant_range(population.vehicle_speed_mps),
        )
    } else {
        (profile.length_m, profile.width_m, profile.speed_mps)
    };

    let mut profiles = BTreeMap::new();
    profiles.insert("speed_mps".to_owned(), speed_mps);
    profiles.insert("max_accel_mps2".to_owned(), profile.max_accel_mps2);
    profiles.insert(
        "comfortable_brake_mps2".to_owned(),
        profile.comfortable_brake_mps2,
    );
    profiles.insert("time_gap_s".to_owned(), profile.time_gap_s);
    profiles.insert("compliance".to_owned(), profile.compliance);

    ModeTemplateSource {
        id: PASSENGER_CAR_TEMPLATE_ID.to_owned(),
        body: ModeBodySource::Box { length_m, width_m },
        motion: MotionKind::SingleBodyWheeled,
        tactics: vec![TacticKind::Follow, TacticKind::Stop, TacticKind::Yield],
        access: AccessSource {
            facility_kinds: vec![FacilityKind::Path],
            nominal_direction: None,
            speed_policy: None,
        },
        occupancy: OccupancyKind::OperatorOnly,
        profiles,
        // Increment 2: a migrated document authors no lateral maneuver.
        lateral: None,
        wheelbase_m: None,
        steering_angle_max_rad: None,
    }
}

/// The Increment 0 pedestrian template, with the resolved version-1 pedestrian
/// profiles materialized into it.
fn pedestrian_template(source: &ScenarioSource) -> ModeTemplateSource {
    let profile = &source.pedestrian_profiles;

    let mut profiles = BTreeMap::new();
    profiles.insert("speed_mps".to_owned(), profile.speed_mps);
    profiles.insert("compliance".to_owned(), profile.compliance);

    ModeTemplateSource {
        id: PEDESTRIAN_TEMPLATE_ID.to_owned(),
        body: ModeBodySource::Circle {
            radius_m: profile.radius_m,
        },
        motion: MotionKind::HolonomicWalking,
        tactics: vec![TacticKind::Follow, TacticKind::Stop, TacticKind::Yield],
        access: AccessSource {
            facility_kinds: vec![
                FacilityKind::Path,
                FacilityKind::Crossing,
                FacilityKind::WaitingArea,
            ],
            nominal_direction: None,
            speed_policy: None,
        },
        occupancy: OccupancyKind::OperatorOnly,
        profiles,
        // Increment 2: a migrated document authors no lateral maneuver.
        lateral: None,
        wheelbase_m: None,
        steering_angle_max_rad: None,
    }
}

/// Absorb the version-1 demand generators and population into mode-tagged
/// version-2 demand.
///
/// Vehicle demand comes first, then pedestrian demand, each in authored order.
/// The population entry is emitted exactly when the source declares no demand
/// of either mode.
fn migrate_demand(source: &ScenarioSource, population_in_use: bool) -> Vec<DemandSourceV2> {
    if population_in_use {
        return vec![DemandSourceV2 {
            id: POPULATION_DEMAND_ID.to_owned(),
            mode: PASSENGER_CAR_TEMPLATE_ID.to_owned(),
            spawn: DemandSpawnSource::Population(DemandPopulationSpawnSource {
                // The walking skeleton populates the scenario's first guide path.
                path: source
                    .paths
                    .first()
                    .map(|path| path.id.clone())
                    .unwrap_or_default(),
                count: source.population.vehicle_count,
                speed_mps: source.population.vehicle_speed_mps,
                spacing_m: source.population.vehicle_spacing_m,
            }),
        }];
    }

    let mut demand = Vec::with_capacity(source.demand.len() + source.pedestrian_demand.len());
    for entry in &source.demand {
        demand.push(DemandSourceV2 {
            id: entry.id.clone(),
            mode: PASSENGER_CAR_TEMPLATE_ID.to_owned(),
            spawn: DemandSpawnSource::Rate(DemandRateSpawnSource {
                portal: entry.portal.clone(),
                rate_per_hour: entry.rate_vph,
                interval_s: whole_run_interval(),
                choice: DemandChoiceSource::Movements(entry.routes.clone()),
            }),
        });
    }
    for entry in &source.pedestrian_demand {
        demand.push(DemandSourceV2 {
            id: entry.id.clone(),
            mode: PEDESTRIAN_TEMPLATE_ID.to_owned(),
            spawn: DemandSpawnSource::Rate(DemandRateSpawnSource {
                portal: entry.portal.clone(),
                rate_per_hour: entry.rate_pph,
                interval_s: whole_run_interval(),
                choice: DemandChoiceSource::Routes(entry.routes.clone()),
            }),
        });
    }
    demand
}

/// The interval that reproduces a version-1 whole-run demand: unbounded from zero.
fn whole_run_interval() -> TimeIntervalSource {
    TimeIntervalSource {
        start_s: 0.0,
        end_s: None,
    }
}

/// A degenerate range holding one value, the constant a population sets.
fn constant_range(value: f64) -> ProfileRangeSource {
    ProfileRangeSource {
        min: value,
        max: value,
    }
}
