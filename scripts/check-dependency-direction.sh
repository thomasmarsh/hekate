#!/usr/bin/env bash
#
# Enforce Hekate's one-way dependency direction.
#
# `hekate-model` and `hekate-sim` are the reusable kernel crates. They must not
# depend on Bevy, on an application crate, or on the presentation layer, even
# transitively, so the simulator stays headless and portable.
#
# `hekate-present` is the shared presentation layer. It may depend on the kernel
# but must not depend on Bevy, a terminal library, or an application crate, so
# every backend (including terminal ones) can consume the same contract.
#
# Application crates may depend on the kernel, the presentation layer, and Bevy;
# `hekate-viewer` is the only place Bevy belongs.
#
# Run from the workspace root. Requires the pinned toolchain.

set -euo pipefail

# Never allowed in any reusable substrate.
common_forbidden=(bevy hekate-cli hekate-viewer)
# Terminal libraries are allowed only in terminal backends, never in the shared
# presentation layer or the kernel.
terminal_forbidden=(crossterm ratatui termion termwiz console)

failed=0

check_forbidden() {
    local package="$1"
    shift
    # Normal and build edges only: dev-dependencies are not shipped. Keep every
    # occurrence visible so a transitive path cannot hide behind deduplication.
    local tree
    tree="$(cargo tree --package "${package}" --edges normal,build --prefix none --no-dedupe)"

    local dep
    for dep in "$@"; do
        if printf '%s\n' "${tree}" | awk '{print $1}' | grep -qx "${dep}"; then
            echo "error: ${package} depends on forbidden crate '${dep}'" >&2
            failed=1
        fi
    done
}

for kernel in hekate-model hekate-sim; do
    check_forbidden "${kernel}" "${common_forbidden[@]}" hekate-present
done

check_forbidden hekate-present "${common_forbidden[@]}" "${terminal_forbidden[@]}"

if [[ "${failed}" -ne 0 ]]; then
    echo "dependency direction check failed" >&2
    exit 1
fi

echo "dependency direction OK"
