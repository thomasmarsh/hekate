//! The version-negotiating load path over the checked-in Increment 1 fixtures.
//!
//! `hekate_present::load_scenario` is the only load path either viewer uses, so
//! every `scenarios/phase2/inc1/*_v2.json5` fixture that loads here opens in
//! `hekate-tui` and `hekate-viewer`. Each fixture must load as a native schema
//! version 2, project its facilities (region ring plus reference path), and
//! drive narrow wheeled agents that project a capsule body.

use std::path::{Path, PathBuf};

use hekate_model::{BodyKind, CompiledScenario};
use hekate_present::{BodyShape, LoadError, SceneBody, SceneGeometry, load_scenario};
use hekate_sim::{RunConfig, Simulation, SnapshotDetail};

/// The six checked-in Increment 1 version-2 fixtures both viewers must open.
const V2_FIXTURES: [&str; 6] = [
    "narrow_isolated_straight_v2.json5",
    "narrow_isolated_curve_v2.json5",
    "narrow_isolated_braking_v2.json5",
    "narrow_following_v2.json5",
    "narrow_signal_v2.json5",
    "narrow_crossing_v2.json5",
];

const SEED: u64 = 0;
/// Ticks a fixture may take to put a narrow agent on the road: 10 s.
const MAX_TICKS: u64 = 200;

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn fixture(name: &str) -> PathBuf {
    repo_path("scenarios/phase2/inc1").join(name)
}

/// Every checked-in version-2 fixture loads, projects its facilities, and
/// carries the narrow capsule bodies its demand spawns.
#[test]
fn every_increment_1_fixture_loads_with_its_facilities_and_narrow_bodies() {
    for name in V2_FIXTURES {
        let scenario = load_scenario(&fixture(name))
            .unwrap_or_else(|error| panic!("{name} did not load: {error}"));
        assert_eq!(
            scenario.schema_version(),
            2,
            "{name} is not native version 2"
        );
        assert!(
            !scenario.facilities().is_empty(),
            "{name} declares no facility"
        );

        let geometry = SceneGeometry::from_scenario(&scenario);
        assert_eq!(
            geometry.facilities().len(),
            scenario.facilities().len(),
            "{name} projects a different facility count"
        );
        for (projected, compiled) in geometry.facilities().iter().zip(scenario.facilities()) {
            assert_eq!(projected.id(), compiled.id(), "{name}");
            assert_eq!(projected.region(), compiled.region(), "{name}");
            // The facility's region is the authored one, so the drawn band is
            // the traversable region the kernel uses.
            let ring = scenario.region(compiled.region()).unwrap_or_else(|| {
                panic!("{name}: facility '{}' names no region", compiled.name())
            });
            assert_eq!(
                projected.points(),
                ring.polygon().ring(),
                "{name}: facility '{}' projects a foreign region",
                compiled.name()
            );
            let reference = projected.reference().unwrap_or_else(|| {
                panic!(
                    "{name}: facility '{}' projects no reference path",
                    compiled.name()
                )
            });
            assert_eq!(
                Some(reference.path()),
                compiled.reference_path(),
                "{name}: facility '{}'",
                compiled.name()
            );
            assert!(
                reference.points().len() >= 2,
                "{name}: facility '{}' projects a reference path with no polyline",
                compiled.name()
            );
        }

        let capsules = narrow_capsule_bodies(&scenario);
        assert!(
            !capsules.is_empty(),
            "{name} put no narrow capsule body on the road in {MAX_TICKS} ticks"
        );
        for body in &capsules {
            // A narrow wheeled agent is one unsegmented envelope, so it draws
            // exactly one capsule and not the boxes of an articulated chain.
            assert_eq!(
                body.shapes(),
                vec![BodyShape::Capsule {
                    center: body.position,
                    heading_rad: body.heading_rad,
                    length_m: body.length_m,
                    radius_m: body.width_m * 0.5,
                }],
                "{name}: body {} does not draw its capsule",
                body.id
            );
        }
    }
}

/// Every capsule body a fixture's demand puts on the road, stepped until one
/// appears or the tick budget runs out.
fn narrow_capsule_bodies(scenario: &CompiledScenario) -> Vec<SceneBody> {
    let mut sim = Simulation::new(scenario.clone(), RunConfig::new(SEED)).expect("scenario builds");
    for _ in 0..MAX_TICKS {
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let capsules: Vec<SceneBody> = snapshot
            .agents()
            .iter()
            .map(|sample| SceneBody::project(&[], sample, 0.0))
            .filter(|body| body.body_kind == BodyKind::Capsule)
            .collect();
        if !capsules.is_empty() {
            return capsules;
        }
    }
    Vec::new()
}

/// A version-1 document keeps the migration path: it loads, reports schema
/// version 1, and projects no facility.
#[test]
fn a_version_1_fixture_keeps_the_migration_path() {
    let scenario = load_scenario(&repo_path("scenarios/walking/walking_guide_v1.json5"))
        .expect("walking scenario loads");
    assert_eq!(scenario.schema_version(), 1);
    assert!(scenario.facilities().is_empty());
    let geometry = SceneGeometry::from_scenario(&scenario);
    assert!(geometry.facilities().is_empty());
    assert_eq!(geometry.paths().len(), scenario.paths().len());
}

/// A document declaring a schema version this build cannot read is reported as
/// an unsupported version, not as a parse failure of the version it resembles.
#[test]
fn an_unsupported_schema_version_is_reported_as_such() {
    let path = std::env::temp_dir().join(format!("hekate-present-v3-{}.json5", std::process::id()));
    std::fs::write(&path, "{ schema_version: 3, id: 'future' }").expect("write scenario");
    let error = load_scenario(&path).expect_err("schema version 3 is unsupported");
    std::fs::remove_file(&path).expect("remove scenario");
    assert!(
        matches!(
            error,
            LoadError::UnsupportedSchemaVersion { version: 3, .. }
        ),
        "{error}"
    );
}
