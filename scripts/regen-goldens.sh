#!/usr/bin/env bash
#
# Regenerate the renderer golden fixtures: the shared scene projection and
# view-command snapshots, the character-cell grids, and the Kitty protocol byte
# fixtures — and the Increment 2 canonical trace goldens under
# `tests/golden/inc2/`.
#
# Run from the repository root. Inspect the diff before committing it: an
# unexpected change is a regression, not a stale fixture. The Phase 1 and
# Increment 6 CLI trace goldens have their own regeneration commands in
# `apps/hekate-cli/tests/golden_trace.rs` and
# `apps/hekate-cli/tests/increment6_trace.rs`.

set -euo pipefail

cd "$(dirname "$0")/.."

export UPDATE_GOLDENS=1

cargo test -p hekate-present --test scene_golden
cargo test -p hekate-tui --test golden_cells --test golden_kitty

# The Increment 2 trace goldens are recorded by their own suite, from the
# in-process driver that supplies each fixture's maneuver request and from the
# `run` command for the three fixtures the CLI can record. The three Standard
# goldens of the CLI-recordable fixtures are also written byte for byte by the
# explicit `hekate-cli run` lines in that suite's module docs.
cargo test -p hekate-cli --test inc2_trace

echo "goldens regenerated"
