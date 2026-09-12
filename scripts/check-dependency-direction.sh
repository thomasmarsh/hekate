#!/usr/bin/env bash
#
# Enforce Tangle's one-way dependency direction.
#
# `tangle-model` and `tangle-sim` are the reusable kernel crates. They must not
# depend on Bevy or on an application crate, even transitively, so the
# simulator stays headless and portable. Application crates may depend on the
# kernel and on Bevy; `tangle-viewer` is the only place Bevy belongs.
#
# Run from the workspace root. Requires the pinned toolchain.

set -euo pipefail

forbidden=(bevy tangle-cli tangle-viewer)
kernels=(tangle-model tangle-sim)
failed=0

for kernel in "${kernels[@]}"; do
    # Normal and build edges only: dev-dependencies are not shipped. Keep every
    # occurrence visible so a transitive path cannot hide behind deduplication.
    tree="$(cargo tree --package "${kernel}" --edges normal,build --prefix none --no-dedupe)"

    for dep in "${forbidden[@]}"; do
        if printf '%s\n' "${tree}" | awk '{print $1}' | grep -qx "${dep}"; then
            echo "error: ${kernel} depends on forbidden crate '${dep}'" >&2
            failed=1
        fi
    done
done

if [[ "${failed}" -ne 0 ]]; then
    echo "dependency direction check failed" >&2
    exit 1
fi

echo "dependency direction OK"
