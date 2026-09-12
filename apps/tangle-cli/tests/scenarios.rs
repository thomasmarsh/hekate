//! Every checked-in scenario loads, validates, and compiles.
//!
//! Phase 1 Increment 1 gate: the benchmark layouts must be expressible through
//! general primitives with no scenario kind. Structural compilation of each
//! benchmark is the observable contract here; behavior is later increments.

use std::path::{Path, PathBuf};

use tangle_cli::load_scenario;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn json5_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("scenario directory is readable") {
        let entry = entry.expect("directory entry is readable");
        let path = entry.path();
        if path.is_dir() {
            json5_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "json5") {
            out.push(path);
        }
    }
}

#[test]
fn every_checked_in_scenario_loads() {
    let root = repo_path("scenarios");
    let mut files = Vec::new();
    json5_files(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no scenarios found under {}",
        root.display()
    );
    for file in &files {
        load_scenario(file).unwrap_or_else(|error| {
            panic!("scenario '{}' failed to load: {error}", file.display())
        });
    }
}

#[test]
fn benchmark_layouts_compile_from_general_primitives() {
    for name in [
        "straight_approach_v1",
        "perpendicular_conflict_v1",
        "four_leg_signal_v1",
    ] {
        let path = repo_path(&format!("scenarios/benchmarks/{name}.json5"));
        let scenario = load_scenario(&path)
            .unwrap_or_else(|error| panic!("benchmark '{name}' failed to load: {error}"));
        assert_eq!(scenario.id(), name);
        assert!(
            !scenario.paths().is_empty(),
            "benchmark '{name}' has no guide path"
        );
        // Every compiled primitive exposes its authored name in the id map.
        assert_eq!(
            scenario.boundaries().len(),
            scenario.id_map().boundaries().len()
        );
        assert_eq!(scenario.regions().len(), scenario.id_map().regions().len());
        assert_eq!(
            scenario.movements().len(),
            scenario.id_map().movements().len()
        );
        assert_eq!(
            scenario.crossings().len(),
            scenario.id_map().crossings().len()
        );
        assert_eq!(
            scenario.conflict_regions().len(),
            scenario.id_map().conflict_regions().len()
        );
        assert_eq!(scenario.rules().len(), scenario.id_map().rules().len());
        assert_eq!(scenario.signals().len(), scenario.id_map().signals().len());
        assert!(
            scenario
                .boundaries()
                .iter()
                .all(|boundary| boundary.polygon().area() > 0.0),
            "benchmark '{name}' has a degenerate boundary"
        );
    }
}

#[test]
fn four_leg_benchmark_exposes_its_signal_and_endpoints() {
    let path = repo_path("scenarios/benchmarks/four_leg_signal_v1.json5");
    let scenario = load_scenario(&path).expect("four-leg benchmark loads");

    assert_eq!(scenario.movements().len(), 2);
    assert_eq!(scenario.crossings().len(), 2);
    assert_eq!(scenario.conflict_regions().len(), 1);
    assert_eq!(scenario.movements()[0].name(), "ew_through");
    assert_eq!(scenario.id_map().movements()[0], "ew_through");

    // Entry and exit endpoints derive from the movement's portals.
    let movement = &scenario.movements()[0];
    let entry = scenario
        .portal(movement.from())
        .expect("from portal compiles");
    assert!((movement.entry() - entry.position()).length() < 1e-9);

    let signal = scenario
        .signals()
        .first()
        .expect("four-leg signal compiles");
    assert_eq!(signal.heads().len(), 2);
    assert_eq!(signal.phases().len(), 4);
    assert!((signal.cycle_s() - 58.0).abs() < 1e-9);
}
