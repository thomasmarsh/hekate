//! Generated JSON Schema for the authored scenario document.
//!
//! The schema is generated from the Serde structures with Schemars and checked
//! in at `schemas/scenario-source.schema.json`. A test fails when the checked-in
//! file drifts from the generated schema; regenerate it with
//! `cargo run -p tangle-model --example generate-schema`.

use schemars::Schema;

use crate::source::ScenarioSource;

/// The JSON Schema for [`ScenarioSource`].
pub fn scenario_schema() -> Schema {
    schemars::schema_for!(ScenarioSource)
}

/// The JSON Schema for [`ScenarioSource`], pretty-printed with a trailing newline.
pub fn scenario_schema_json() -> String {
    let mut json = serde_json::to_string_pretty(&scenario_schema())
        .expect("a generated JSON Schema is always serializable");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_schema_matches_the_generated_schema() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/scenario-source.schema.json"
        );
        let checked_in = std::fs::read_to_string(path)
            .expect("schemas/scenario-source.schema.json is checked in");
        assert_eq!(
            checked_in,
            scenario_schema_json(),
            "checked-in JSON Schema is stale; run \
             `cargo run -p tangle-model --example generate-schema`"
        );
    }
}
