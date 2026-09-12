#!/usr/bin/env bash
#
# Regenerate the renderer golden fixtures: the shared scene projection and
# view-command snapshots, the character-cell grids, and the Kitty protocol byte
# fixtures.
#
# Run from the repository root. Inspect the diff before committing it: an
# unexpected change is a regression, not a stale fixture. The checked-in CLI
# trace golden has its own regeneration command in
# `apps/tangle-cli/tests/golden_trace.rs`.

set -euo pipefail

cd "$(dirname "$0")/.."

export UPDATE_GOLDENS=1

cargo test -p tangle-present --test scene_golden
cargo test -p tangle-tui --test golden_cells --test golden_kitty

echo "renderer goldens regenerated"
