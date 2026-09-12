//! End-to-end check that the checked-in walking-skeleton scenario parses,
//! validates, and compiles with the string-to-dense ID mapping intact.

use tangle_model::{CompiledScenario, PathId, PortalId, parse_scenario_source};

fn walking_source() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/walking/walking_guide_v1.json5"
    );
    std::fs::read_to_string(path).expect("walking scenario is checked in")
}

#[test]
fn checked_in_walking_scenario_compiles() {
    let source = parse_scenario_source(&walking_source()).expect("walking scenario parses");
    let compiled = CompiledScenario::compile(source).expect("walking scenario compiles");

    assert_eq!(compiled.id(), "walking_guide_v1");
    assert_eq!(compiled.schema_version(), 1);

    let id_map = compiled.id_map();
    assert_eq!(id_map.path_name(PathId::from_index(0)), Some("guide"));
    assert_eq!(
        id_map.portal_name(PortalId::from_index(0)),
        Some("west_entry")
    );
    assert_eq!(
        id_map.portal_name(PortalId::from_index(1)),
        Some("east_exit")
    );

    let path = compiled.path(PathId::from_index(0)).expect("guide path");
    assert!((path.length() - 120.0).abs() < 1e-9);
    assert_eq!(compiled.portals().len(), 2);
}
