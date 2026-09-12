//! Regenerate the checked-in JSON Schema for the authored scenario document.
//!
//! Run from the workspace with:
//!
//! ```sh
//! cargo run -p tangle-model --example generate-schema
//! ```

use std::path::PathBuf;

fn main() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = workspace.join("schemas/scenario-source.schema.json");
    std::fs::write(&output, tangle_model::scenario_schema_json()).expect("schema is writable");
    println!("wrote {}", output.display());
}
