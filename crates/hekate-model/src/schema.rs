//! Generated JSON Schemas for the authored scenario documents.
//!
//! The supported schema is version 2, generated from [`ScenarioSourceV2`] and
//! checked in at `schemas/scenario-source.schema.json`. The version-1 reader
//! schema is generated from [`ScenarioSource`] and checked in at
//! `schemas/scenario-source-v1.schema.json`. A test fails when either checked-in
//! file drifts from its generated schema; regenerate both with
//! `cargo run -p hekate-model --example generate-schema`.

use schemars::Schema;

use crate::source::{ScenarioSource, ScenarioSourceV2};

/// The JSON Schema for the supported version-2 document.
pub fn scenario_schema() -> Schema {
    schemars::schema_for!(ScenarioSourceV2)
}

/// The JSON Schema for the supported version-2 document, pretty-printed with a
/// trailing newline.
pub fn scenario_schema_json() -> String {
    let mut json = serde_json::to_string_pretty(&scenario_schema())
        .expect("a generated JSON Schema is always serializable");
    json.push('\n');
    json
}

/// The JSON Schema for the version-1 document, still read directly.
pub fn scenario_schema_v1() -> Schema {
    schemars::schema_for!(ScenarioSource)
}

/// The JSON Schema for the version-1 document, pretty-printed with a trailing
/// newline.
pub fn scenario_schema_v1_json() -> String {
    let mut json = serde_json::to_string_pretty(&scenario_schema_v1())
        .expect("a generated JSON Schema is always serializable");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_checked_in(relative: &str) -> String {
        let path = format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(path).expect("checked-in schema exists")
    }

    #[test]
    fn checked_in_v2_schema_matches_the_generated_schema() {
        assert_eq!(
            read_checked_in("schemas/scenario-source.schema.json"),
            scenario_schema_json(),
            "checked-in JSON Schema is stale; run \
             `cargo run -p hekate-model --example generate-schema`"
        );
    }

    #[test]
    fn checked_in_v1_schema_matches_the_generated_schema() {
        assert_eq!(
            read_checked_in("schemas/scenario-source-v1.schema.json"),
            scenario_schema_v1_json(),
            "checked-in JSON Schema is stale; run \
             `cargo run -p hekate-model --example generate-schema`"
        );
    }
}
