#!/usr/bin/env bash
#
# Reproducible release-mode run of the end-to-end benchmark harness.
#
# The harness is `apps/hekate-cli/tests/release_benchmark.rs`, an ignored test so
# the five gates never pay for a wall-clock measurement. This script is the
# documented way to run it: it builds the release profile, runs the harness, and
# leaves the machine-readable artifact at `perf/release-bench.json`.
#
# Usage:
#   scripts/bench-release.sh
#
# Environment:
#   CARGO_TARGET_DIR  Standard cargo override; the default target/ is fine.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "${repo_root}"

echo "release benchmark: building the release profile and running the ignored harness" >&2
cargo nextest run --release -p hekate-cli --test release_benchmark \
  --run-ignored ignored-only --no-capture

echo "release benchmark artifact: ${repo_root}/perf/release-bench.json" >&2
