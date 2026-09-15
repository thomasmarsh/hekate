//! Regenerate the checked-in JSON Schemas for the authored scenario documents.
//!
//! Run from the workspace with:
//!
//! ```sh
//! cargo run -p hekate-model --example generate-schema
//! ```

use std::path::PathBuf;

fn main() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (file, json) in [
        (
            "scenario-source.schema.json",
            hekate_model::scenario_schema_json(),
        ),
        (
            "scenario-source-v1.schema.json",
            hekate_model::scenario_schema_v1_json(),
        ),
    ] {
        let output = workspace.join("schemas").join(file);
        std::fs::write(&output, json).expect("schema is writable");
        println!("wrote {}", output.display());
    }
}
