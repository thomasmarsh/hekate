#!/usr/bin/env bash
#
# Reclaim the regenerable build output a long-lived checkout accumulates in
# `target/`. Three things go:
#
#   target/**/tangle*        fingerprints, rlibs, and test binaries from before
#                            the workspace crates were renamed from `tangle-*`
#                            to `hekate-*`; no current manifest names a
#                            `tangle-*` crate, so nothing reads them again.
#   target/debug/incremental  the per-hash incremental cache.
#   target/doc                the rustdoc output.
#
# Everything removed here is build output cargo rebuilds on demand, and the
# script only ever descends from `target/`, so source, checked-in goldens,
# scenarios, schemas, and `Cargo.lock` are never candidates. Re-running it is
# safe and becomes a no-op once the tree is clean.
#
# Usage:
#   scripts/clean-target.sh            prune and report the reclaimed space
#   scripts/clean-target.sh --dry-run  list exactly what would be removed
#
# Run from the repository root.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "${repo_root}"

target_dir=target
dry_run=0

while [ $# -gt 0 ]; do
    case $1 in
        -n|--dry-run)
            dry_run=1
            ;;
        *)
            printf 'usage: %s [--dry-run|-n]\n' "$0" >&2
            exit 2
            ;;
    esac
    shift
done

if [ ! -d "${target_dir}" ]; then
    echo "no ${target_dir}/ directory at ${repo_root}; nothing to do"
    exit 0
fi

before_kib=$(du -sk "${target_dir}" | cut -f1)
echo "before: $(du -sh "${target_dir}")"

# The incremental and rustdoc trees are removed whole below, so matches inside
# them are left off this listing rather than reported twice.
echo "pre-rename tangle* artifacts:"
tangle_count=0
while IFS= read -r -d '' path; do
    tangle_count=$((tangle_count + 1))
    if [ "${dry_run}" -eq 1 ]; then
        echo "  would remove ${path}"
    else
        rm -rf -- "${path}"
    fi
done < <(find "${target_dir}" \
    -not -path "${target_dir}/debug/incremental/*" \
    -not -path "${target_dir}/doc/*" \
    -name 'tangle*' -print0)
if [ "${dry_run}" -eq 1 ]; then
    echo "  ${tangle_count} path(s) would be removed"
else
    echo "  removed ${tangle_count} path(s)"
fi

prune_dir() {
    local dir=$1
    if [ ! -d "${dir}" ]; then
        echo "  ${dir}/ is already absent"
        return 0
    fi
    if [ "${dry_run}" -eq 1 ]; then
        echo "  would remove ${dir}/ ($(du -sh "${dir}" | cut -f1))"
    else
        echo "  removed ${dir}/ ($(du -sh "${dir}" | cut -f1))"
        rm -rf -- "${dir}"
    fi
}

echo "per-hash incremental cache:"
prune_dir "${target_dir}/debug/incremental"

echo "rustdoc output:"
prune_dir "${target_dir}/doc"

if [ "${dry_run}" -eq 1 ]; then
    echo "dry run: nothing removed"
    exit 0
fi

after_kib=$(du -sk "${target_dir}" | cut -f1)
echo "after: $(du -sh "${target_dir}")"
awk -v kib="$((before_kib - after_kib))" 'BEGIN {
    if (kib >= 1048576) printf "reclaimed: %.1f GiB\n", kib / 1048576
    else printf "reclaimed: %.1f MiB\n", kib / 1024
}'
