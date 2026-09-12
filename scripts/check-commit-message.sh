#!/bin/sh
# Validate commit messages against Conventional Commits.
#
# Usage:
#   scripts/check-commit-message.sh <path-to-commit-message-file>
#   scripts/check-commit-message.sh --range <git-range>
#
# The file form is used by the commit-msg hook; the range form is used by CI.
# Exits non-zero if any subject is invalid.

set -eu

types='build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test'
pattern="^($types)(\([a-z0-9._/-]+\))?!?: .+"

check_subject() {
    subject=$1

    # Allow subjects git or the rebase machinery generates itself.
    case $subject in
        'Merge '*|'Revert '*|'fixup! '*|'squash! '*|'amend! '*)
            return 0
            ;;
    esac

    if printf '%s\n' "$subject" | grep -Eq "$pattern"; then
        return 0
    fi

    printf 'Invalid Conventional Commit subject: %s\n' "$subject" >&2
    printf '  expected: <type>(<scope>)!: <description>\n' >&2
    printf '  types:    %s\n' "$(printf '%s' "$types" | tr '|' ' ')" >&2
    return 1
}

if [ "${1:-}" = "--range" ]; then
    [ $# -eq 2 ] || { printf 'usage: %s --range <git-range>\n' "$0" >&2; exit 2; }
    range=$2
    tmp=$(mktemp)
    trap 'rm -f "$tmp"' EXIT
    git log --format=%s "$range" > "$tmp"
    while IFS= read -r subject; do
        check_subject "$subject" || exit 1
    done < "$tmp"
    exit 0
fi

[ $# -eq 1 ] || { printf 'usage: %s <commit-message-file>\n' "$0" >&2; exit 2; }

subject=
while IFS= read -r line; do
    case $line in
        '#'*) continue ;;
        '')   [ -n "$subject" ] && break; continue ;;
    esac
    subject=$line
    break
done < "$1"

[ -n "$subject" ] || { printf 'Empty commit message\n' >&2; exit 1; }
check_subject "$subject" || exit 1
