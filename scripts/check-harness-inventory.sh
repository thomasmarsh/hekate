#!/usr/bin/env bash
#
# Fail when an `#[ignore]` test is not a declared harness target.
#
# The default `cargo nextest run --workspace` suite runs only quick tests;
# everything expensive is `#[ignore = "slow: ..."]` and is reachable only through
# `scripts/run-test-harness.sh` (default build, budget-free `harness` profile) or
# its `--release-benchmarks` form (the release-mode wall-clock tests, which also
# `scripts/bench-release.sh` runs). That pattern has one failure mode the tests
# themselves cannot catch: marking a test `#[ignore]` and never adding it to a
# harness run silently drops its coverage. This guard closes it.
#
# The declared targets are listed below, one `<path> <target>` per line, where
# `<target>` is the test function's name, or `macro:<name>` when the `#[ignore]`
# sits in a `macro_rules!` body that generates the tests (those generated names do
# not appear in the source). The guard scans every tracked Rust file, extracts the
# target for each `#[ignore]` attribute, and fails if a target is not declared or
# a declared target no longer carries an `#[ignore]`. `docs/test-policy.md`
# records how to add a new one.
#
# Usage:
#   scripts/check-harness-inventory.sh
#
# Run from the repository root.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "${repo_root}"

# ── Declared harness targets ────────────────────────────────────────────────
# Release-mode wall-clock tests: `#[ignore]`, run with `--release` by
# `scripts/run-test-harness.sh --release-benchmarks` and `scripts/bench-release.sh`.
# `release_benchmark` panics under `debug_assertions`, so these must never enter
# the default-build harness run.
release_targets=(
    "crates/hekate-sim/tests/performance_counters.rs increment_2_profile_counters"
    "crates/hekate-present/tests/presenter_frame_time.rs increment_2_profile_frame_time"
    "apps/hekate-cli/tests/release_benchmark.rs release_benchmark_writes_the_checked_in_artifact"
)

# Slow behavior, measurement, and gate tests: `#[ignore = "slow: ..."]`, run in
# the default build by plain `scripts/run-test-harness.sh`.
slow_targets=(
    "crates/hekate-sim/tests/mixed_interaction.rs the_mixed_benchmark_gate_holds_over_every_declared_seed"
    "crates/hekate-sim/tests/vehicle_yielding.rs a_vehicle_holds_before_an_occupied_crossing_and_resumes"
    "crates/hekate-sim/tests/vehicle_yielding.rs yielding_emits_a_begin_and_an_end_transition_per_agent"
    "crates/hekate-sim/tests/vehicle_yielding.rs yielding_stays_within_the_comfort_bound_except_a_counted_cap_step"
    "crates/hekate-sim/tests/vehicle_yielding.rs the_mixed_benchmark_has_no_vehicle_pedestrian_overlap_or_deadlock"
    "crates/hekate-sim/tests/vehicle_yielding.rs the_yield_rule_removes_the_slice_b_vehicle_overlap_residual"
    "crates/hekate-sim/tests/vehicle_yielding.rs a_backward_vehicle_brakes_before_an_occupied_crossing"
    "crates/hekate-sim/tests/metrics.rs post_encroachment_time_matches_the_frame_reference_and_converges"
    "crates/hekate-sim/tests/safety_events.rs the_mixed_benchmark_stream_is_ordered_deterministic_and_both_modes"
    "apps/hekate-cli/tests/inc2_trace.rs macro:golden_run_cases"
    "apps/hekate-cli/tests/inc2_trace.rs the_cli_run_and_replay_reproduce_the_standard_goldens_of_the_autonomous_fixtures"
    "apps/hekate-cli/tests/inc2_determinism.rs the_inc2_seed_bank_batch_reproduces_every_per_seed_hash"
    "apps/hekate-cli/tests/inc2_determinism.rs the_declared_seed_bank_runs_the_same_traces_as_an_explicit_seed_list"
    "apps/hekate-cli/tests/narrow_determinism.rs the_narrow_seed_bank_batch_reproduces_every_per_seed_hash"
    "apps/hekate-cli/tests/narrow_determinism.rs each_narrow_fixture_reproduces_its_trace_hash_at_every_preset"
    "apps/hekate-cli/tests/migration_regression.rs every_scenario_migrates_or_is_a_valid_version_2_document"
    "apps/hekate-cli/tests/migration_regression.rs migrated_frozen_runs_reproduce_their_golden_bodies_and_pinned_hashes"
    "apps/hekate-cli/tests/scenarios.rs car_following_benchmark_obeys_controller_bounds_without_overlap"
    "apps/hekate-cli/tests/emergency_cap.rs red_queue_close_up_stays_below_the_accepted_emergency_cap_step"
    "apps/hekate-cli/tests/experiment_spec.rs the_checked_in_experiment_runs_paired_from_the_seed_bank"
    "apps/hekate-cli/tests/increment6_trace.rs the_increment6_variant_goldens_match_and_replay_verifies_them"
)

declared=("${release_targets[@]}" "${slow_targets[@]}")

is_declared() {
    local wanted="$1" target
    for target in "${declared[@]}"; do
        if [ "${target}" = "${wanted}" ]; then
            return 0
        fi
    done
    return 1
}

# Print `<path> <target>` for every `#[ignore]` attribute in one Rust file. The
# target is the name of the `fn` the attribute annotates, or `macro:<name>` when
# the annotated `fn` is a `macro_rules!` metavariable. Any other shape is an
# error the guard reports rather than guesses at.
extract_ignore_targets() {
    local file="$1"
    awk -v path="${file}" '
        # Track the enclosing `macro_rules!` so a generated `fn $name` can be
        # attributed to it.
        /^[[:space:]]*macro_rules![[:space:]]*[A-Za-z0-9_]+/ {
            name = $0
            sub(/^[[:space:]]*macro_rules![[:space:]]*/, "", name)
            sub(/[^A-Za-z0-9_].*$/, "", name)
            macro = name
        }
        /^[[:space:]]*#\[ignore/ {
            pending = 1
            next
        }
        pending == 1 {
            if ($0 ~ /^[[:space:]]*$/) next
            if ($0 ~ /^[[:space:]]*\/\//) next
            if ($0 ~ /^[[:space:]]*#/) next
            if ($0 ~ /^[[:space:]]*fn[[:space:]]+\$/) {
                print path " macro:" macro
                pending = 0
                next
            }
            if ($0 ~ /^[[:space:]]*fn[[:space:]]+/) {
                name = $0
                sub(/^[[:space:]]*fn[[:space:]]+/, "", name)
                sub(/[^A-Za-z0-9_].*$/, "", name)
                print path " " name
                pending = 0
                next
            }
            print path " UNKNOWN-at-line-" FNR
            pending = 0
        }
    ' "${file}"
}

found=()
while IFS= read -r file; do
    while IFS= read -r target; do
        [ -n "${target}" ] && found+=("${target}")
    done < <(extract_ignore_targets "${file}")
done < <(git ls-files '*.rs')

if [ "${#found[@]}" -eq 0 ]; then
    echo "check-harness-inventory: no #[ignore] tests found; the scan is broken" >&2
    exit 1
fi

status=0

for target in "${found[@]}"; do
    if ! is_declared "${target}"; then
        echo "check-harness-inventory: #[ignore] test is not a declared harness target:" >&2
        echo "check-harness-inventory:   ${target}" >&2
        status=1
    fi
done

for target in "${declared[@]}"; do
    declared_found=false
    for candidate in "${found[@]}"; do
        if [ "${candidate}" = "${target}" ]; then
            declared_found=true
            break
        fi
    done
    if [ "${declared_found}" = false ]; then
        echo "check-harness-inventory: declared harness target has no #[ignore]:" >&2
        echo "check-harness-inventory:   ${target}" >&2
        status=1
    fi
done

if [ "${status}" -ne 0 ]; then
    echo "check-harness-inventory: declare the target below or remove the #[ignore];" >&2
    echo "check-harness-inventory: see docs/test-policy.md" >&2
    exit 1
fi

echo "check-harness-inventory: ${#found[@]} #[ignore] sites, all declared harness targets"
