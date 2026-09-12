//! Bevy top-down viewer for Tangle.
//!
//! The viewer is the only crate permitted to depend on Bevy. Increment 0
//! provides only the windowing skeleton; rendering arrives with the later
//! viewer work.

use bevy::prelude::*;

fn main() {
    App::new().add_plugins(DefaultPlugins).run();
}
