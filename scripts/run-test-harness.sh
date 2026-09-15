#!/usr/bin/env bash
#
# Run the slow harness: the `#[ignore = "slow: ..."]` tests the default suite
# excludes.
#
# The default `cargo nextest run --workspace` suite is quick, deterministic
# pass/fail only. Everything expensive is `#[ignore = "slow: ..."]` and is
# selected here by `--run-ignored ignored-only` under the budget-free `harness`
# profile (`.config/nextest.toml`). `docs/test-policy.md` records the split and
# how to add a new expensive test; `scripts/check-harness-inventory.sh` proves
# every ignored test is a declared target here.
#
# Usage:
#   scripts/run-test-harness.sh
#       Run the moved slow behavior and gate tests in the default build.
#   scripts/run-test-harness.sh --release-benchmarks
#       Also run the release-mode wall-clock benchmarks. They measure wall
#       clock and `release_benchmark` panics under `debug_assertions`, so they
#       only run with `--release`; `release_benchmark` rewrites
#       `perf/release-bench.json`.
#
# CI runs the default form on push and pull request, so the moved coverage is
# not lost and no pull request rewrites `perf/release-bench.json`. Run the
# release benchmarks locally or on a schedule instead.
#
# Environment:
#   CARGO_TARGET_DIR  Standard cargo override; the default target/ is fine.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "${repo_root}"

# The pin CI installs with `taiki-e/install-action`; a different nextest may
# parse the budget differently, so warn rather than fail.
nextest_pin=0.9.144

# The three release-mode wall-clock tests. `release_benchmark` panics under
# `debug_assertions` and the other two measure release-mode frame time, so the
# default-build harness run must exclude them and the release run must select
# exactly them.
release_binaries='binary(release_benchmark) | binary(presenter_frame_time) | binary(performance_counters)'

check_nextest() {
    if ! command -v cargo-nextest >/dev/null 2>&1; then
        echo "run-test-harness: cargo-nextest is not installed" >&2
        echo "run-test-harness: install it with: cargo install cargo-nextest --version ${nextest_pin} --locked" >&2
        exit 1
    fi
    local version
    version=$(cargo nextest --version | head -n 1 | awk '{print $2}')
    echo "run-test-harness: cargo-nextest ${version} (pinned ${nextest_pin})" >&2
    if [ "${version}" != "${nextest_pin}" ]; then
        echo "run-test-harness: warning: cargo-nextest ${version} is not the pinned ${nextest_pin}; the" >&2
        echo "run-test-harness: warning: slow-timeout budget may behave differently" >&2
    fi
}

run_slow_tests() {
    echo "run-test-harness: slow behavior and gate tests (default build, harness profile)" >&2
    cargo nextest run --profile harness --workspace --all-features \
        --run-ignored ignored-only \
        -E "not (${release_binaries})"
}

run_release_benchmarks() {
    echo "run-test-harness: release-mode wall-clock benchmarks (rewrites perf/release-bench.json)" >&2
    cargo nextest run --profile harness --release --workspace --all-features \
        --run-ignored ignored-only \
        -E "(${release_binaries})" \
        --no-capture
}

release_benchmarks=false
for argument in "$@"; do
    case "${argument}" in
        --release-benchmarks) release_benchmarks=true ;;
        *)
            echo "run-test-harness: unknown argument '${argument}'" >&2
            echo "usage: scripts/run-test-harness.sh [--release-benchmarks]" >&2
            exit 2
            ;;
    esac
done

check_nextest

run_slow_tests
if [ "${release_benchmarks}" = true ]; then
    run_release_benchmarks
fi
