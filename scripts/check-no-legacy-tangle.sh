#!/usr/bin/env bash
#
# Reject pre-rename Tangle crate naming.
#
# Commit e9a418d renamed every workspace crate from `tangle-*` to `hekate-*`.
# Any tracked file that still names one of the old packages, its Rust crate
# identifier, or cargo's `libtangle_*` artifacts is a stale reference or a
# regression, so this guard fails the repo. The `lib` prefix is why a plain
# `tangle*` glob misses the old rlibs; `libtangle` is matched directly.
#
# It matches three families:
#
#   tangle-cli, tangle-viewer, tangle-tui, tangle-model, tangle-sim,
#   tangle-present    the pre-rename package/manifest names.
#   tangle_cli, tangle_viewer, tangle_tui, tangle_model, tangle_sim,
#   tangle_present    the same names as Rust crate identifiers.
#   libtangle         cargo's `lib`-prefixed artifact stem.
#
# The Tangle work-ledger is not a pre-rename artifact: `.tangle/` is out of
# scope, and the leading non-word boundary keeps the pattern from matching the
# word `rectangle` (or `rectangle_cli`), the bare word `tangle`, the `tangle`
# command, `tangle_revision`, and `IDX-002-tangle-feedback`.
#
# Three files spell the forbidden patterns out on purpose - the reclaimer that
# deletes the stale artifacts and the doc that describes it
# (`scripts/clean-target.sh`, `docs/dev-loop.md`), plus this guard, which
# necessarily names every pattern - so all three are excluded explicitly.
#
# Usage:
#   scripts/check-no-legacy-tangle.sh              scan tracked files
#   scripts/check-no-legacy-tangle.sh --self-test  prove the guard rejects a fixture
#
# Run from the repository root.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "${repo_root}"

self_path=scripts/check-no-legacy-tangle.sh

# The leading `(^|[^[:alnum:]_])` is the word boundary: it admits `tangle-cli`,
# `"tangle-cli"`, and `foo/libtangle_model-*.rlib` alike while rejecting
# `rectangle` and any identifier that merely ends in `_cli`/`_model`/etc.
pattern='(^|[^[:alnum:]_])(tangle-(cli|viewer|tui|model|sim|present)|tangle_(cli|viewer|tui|model|sim|present)|libtangle)'

excluded=(
    ':(exclude).tangle/**'
    ":(exclude)${self_path}"
    ':(exclude)scripts/clean-target.sh'
    ':(exclude)docs/dev-loop.md'
)

# Print `<path>:<line>:<text>` for every match under the given pathspecs.
# `git grep` exits 1 for "no match", which is this check's success case, so only
# a real error (status > 1) is propagated.
scan() {
    local -a git_grep=(git grep -nE)
    if [ "${1:-}" = --no-index ]; then
        git_grep+=(--no-index)
        shift
    fi
    local status=0
    "${git_grep[@]}" -e "${pattern}" -- "$@" || status=$?
    if [ "${status}" -gt 1 ]; then
        echo "error: ${git_grep[*]} failed with status ${status}" >&2
        return "${status}"
    fi
}

# Report violations and fail the check.
reject() {
    echo "error: pre-rename tangle naming found in tracked files:" >&2
    printf '%s\n' "$1" >&2
    echo "rename these to the hekate-* crates; the tangle-* names are dead." >&2
    exit 1
}

# The --self-test fixture directory. It is a file-scope variable because the
# EXIT trap runs after the function that created it has returned.
self_test_dir=

cleanup_self_test() {
    if [ -n "${self_test_dir}" ]; then
        rm -rf -- "${self_test_dir}"
    fi
}

# Prove the guard fails on a violation instead of only passing on the clean
# tree: a fixture carrying a forbidden token has to be flagged and rejected,
# while the intentional `tangle` ledger spellings have to be ignored. The
# fixture lives under `target/` because `git grep --no-index` refuses paths
# outside the repository and `target/` is git-ignored.
self_test() {
    local fixture hits failures=0

    mkdir -p target
    self_test_dir="$(mktemp -d target/no-legacy-tangle-self-test.XXXXXX)"
    trap cleanup_self_test EXIT
    fixture="${self_test_dir}/fixture"

    local -a forbidden=(
        'name = "tangle-cli"'
        'name = "tangle-viewer"'
        'name = "tangle-tui"'
        'name = "tangle-model"'
        'name = "tangle-sim"'
        'name = "tangle-present"'
        'use tangle_cli;'
        'use tangle_viewer;'
        'use tangle_tui;'
        'use tangle_model;'
        'use tangle_sim;'
        'use tangle_present;'
        'target/debug/deps/libtangle_model-0f3a9b.rlib'
    )
    local -a allowed=(
        'rectangle'
        'rectangle_cli'
        'tangle'
        'the Tangle work-ledger'
        'tangle_revision'
        'IDX-002-tangle-feedback'
    )

    local token
    for token in "${forbidden[@]}"; do
        printf '%s\n' "${token}" > "${fixture}"
        hits="$(scan --no-index "${fixture}")"
        if [ -z "${hits}" ]; then
            printf 'self-test FAILED: guard did not flag %s\n' "${token}" >&2
            failures=$((failures + 1))
            continue
        fi
        # The default scan's own rejection path has to exit non-zero on the hit.
        if (reject "${hits}") >/dev/null 2>&1; then
            printf 'self-test FAILED: guard accepted %s\n' "${token}" >&2
            failures=$((failures + 1))
        fi
    done

    for token in "${allowed[@]}"; do
        printf '%s\n' "${token}" > "${fixture}"
        hits="$(scan --no-index "${fixture}")"
        if [ -n "${hits}" ]; then
            printf 'self-test FAILED: guard flagged allowed text %s as %s\n' \
                "${token}" "${hits}" >&2
            failures=$((failures + 1))
        fi
    done

    if [ "${failures}" -ne 0 ]; then
        echo "legacy tangle naming guard self-test failed" >&2
        exit 1
    fi
    printf 'legacy tangle naming guard self-test OK: %d forbidden tokens rejected, %d allowed spellings ignored\n' \
        "${#forbidden[@]}" "${#allowed[@]}"
}

case "${1:-}" in
    '')
        violations="$(scan . "${excluded[@]}")"
        if [ -n "${violations}" ]; then
            reject "${violations}"
        fi
        echo "no pre-rename tangle naming in tracked files"
        ;;
    --self-test)
        self_test
        ;;
    *)
        printf 'usage: %s [--self-test]\n' "$0" >&2
        exit 2
        ;;
esac
