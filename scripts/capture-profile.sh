#!/usr/bin/env bash
#
# Capture a sampled profile of the release-mode hot path.
#
# The host TAS-042 recorded has `/usr/bin/sample` and `xcrun`, and has neither
# `samply`, `cargo instruments`, nor `perf`, so the capture is `sample`'s default
# 1 ms sampler attached to the release binary while it runs the mixed benchmark
# long enough to be sampled in steady state. `scripts/profile-symbols.py` then
# folds the capture's call graph into a phase table: every child of
# `Simulation::step` with its share of the tick, so the always-on
# interaction-metrics pass can be read straight off the profile.
#
# Usage:
#   scripts/capture-profile.sh [SCENARIO] [TICKS] [SECONDS]
#
# Outputs, all under perf/profiles/:
#   <scenario>-release.sample.txt   the raw `sample` capture
#   <scenario>-release.summary.txt  the phase table folded from the capture
#   <scenario>-release.run.txt      the run's stderr, including its trace hash
#
# Requires: macOS `sample`, cargo, and the pinned toolchain.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
scenario=${1:-scenarios/benchmarks/mixed_interaction_v1.json5}
ticks=${2:-300000}
seconds=${3:-8}

if ! command -v sample >/dev/null 2>&1; then
    echo "error: sample is not on PATH; see perf/README.md for the fallback" >&2
    exit 1
fi

stem=$(basename "${scenario}" .json5)
out_dir="${repo_root}/perf/profiles"
capture="${out_dir}/${stem}-release.sample.txt"
summary="${out_dir}/${stem}-release.summary.txt"
run_log="${out_dir}/${stem}-release.run.txt"

mkdir -p "${out_dir}"
cd "${repo_root}"
cargo build --release -p tangle-cli

echo "profiling ${ticks} ticks of ${scenario} for ${seconds} s" >&2
./target/release/tangle-cli run "${scenario}" --ticks "${ticks}" --output /dev/null \
    2> "${run_log}" &
pid=$!
# If sampling fails the run is killed rather than left spinning.
trap 'kill "${pid}" 2>/dev/null || true' EXIT

sleep 1
if ! sample "${pid}" "${seconds}" -file "${capture}"; then
    echo "error: sample failed for pid ${pid}" >&2
    exit 1
fi
wait "${pid}"
trap - EXIT

python3 "${repo_root}/scripts/profile-symbols.py" "${capture}" > "${summary}"
cat "${summary}"
echo "sample capture: ${capture}" >&2
echo "phase table:    ${summary}" >&2
