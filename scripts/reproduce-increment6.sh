#!/usr/bin/env bash
#
# Reproduce Increment 6's checked-in evidence from a clean checkout: the
# comparison report, the Fast/Standard/Fine convergence evidence and its
# summary, `replay --verify` on every manifest behind them, and the checked-in
# golden traces.
#
# Run from the repository root. Nothing here needs a run directory that is
# already present: the experiment spec, the seed bank, the scenario sources, and
# the golden traces are checked in, and every run directory the script writes
# under experiments/increment6_signal_timing_v1/runs/ is gitignored and
# regenerable. The artifacts are written in place, so `git status` afterwards
# shows whether the checked-in evidence is the evidence this tree produces.
#
#   scripts/reproduce-increment6.sh
#   TANGLE_CLI=target/release/tangle-cli scripts/reproduce-increment6.sh
#
# JOBS sets the whole-run concurrency (default 8); every run stays
# single-threaded, and a parallel batch produces the same per-run trace hashes
# as a serial one.

set -euo pipefail

cd "$(dirname "$0")/.."

EXPERIMENT=experiments/increment6_signal_timing_v1
RUN_ROOT="$EXPERIMENT/runs"
JOBS="${JOBS:-8}"

# The declared golden run: seed 1 over 1200 steps of the Standard 50 ms preset,
# one 58 s signal cycle of the variants.
GOLDEN_SEED=1
GOLDEN_TICKS=1200

CLI="${TANGLE_CLI:-}"
if [ -z "$CLI" ]; then
  echo "== building the release CLI"
  cargo build --release -p tangle-cli
  CLI=target/release/tangle-cli
fi

echo "== comparison report, convergence evidence, and convergence summary"
"$CLI" experiment "$EXPERIMENT/experiment.json" \
  --run-root "$RUN_ROOT" \
  --jobs "$JOBS" \
  --output "$EXPERIMENT/comparison_report.json" \
  --convergence "$EXPERIMENT/convergence_evidence.json" \
  --summary "$EXPERIMENT/convergence_summary.md"

echo "== replay --verify on every recorded manifest"
shopt -s nullglob
verified=0
for variant in ew_priority ns_priority; do
  for run_dir in \
    "$RUN_ROOT/$variant"/seed-* \
    "$RUN_ROOT/convergence/$variant"/fast/seed-* \
    "$RUN_ROOT/convergence/$variant"/fine/seed-*; do
    "$CLI" replay "$run_dir" --verify >/dev/null
    verified=$((verified + 1))
  done
done
if [ "$verified" -eq 0 ]; then
  echo "no run directory was verified: the comparison and convergence runs are missing" >&2
  exit 1
fi
echo "verified $verified run directories"

echo "== golden traces"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
for variant in ew_priority ns_priority; do
  golden="tests/golden/four_leg_pedestrian_${variant}_v1.trace"
  "$CLI" run "scenarios/experiments/four_leg_pedestrian_${variant}_v1.json5" \
    --seed "$GOLDEN_SEED" --ticks "$GOLDEN_TICKS" \
    --output "$scratch/trace.jsonl" \
    --hash-file "$scratch/trace.sha256"
  diff -u "$golden.jsonl" "$scratch/trace.jsonl"
  diff -u "$golden.sha256" "$scratch/trace.sha256"
  echo "$variant: the canonical event stream and its hash match the golden"
done

echo
echo "reproduced. The checked-in artifacts were written in place;"
echo "'git status experiments/' shows whether this tree produces them unchanged."
