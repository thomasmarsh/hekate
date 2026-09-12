//! Headless command-line interface for Tangle.
//!
//! Increment 0 provides only the crate skeleton; the `validate` and `run`
//! subcommands arrive with the scenario and kernel work.

fn main() {
    println!(
        "tangle-cli {} (model {})",
        env!("CARGO_PKG_VERSION"),
        tangle_sim::MODEL_VERSION
    );
}
