#!/usr/bin/env bash
#
# Measure the always-on interaction-metrics pass as a share of the tick.
#
# Phase 1 Increment 4 recorded roughly 22 microseconds per tick for the online
# interaction-metrics pass against roughly 7 microseconds for the rest of the
# tick on `mixed_interaction_v1` ([[TAS-030-phase-1-increment-4-geometry-queries-safety-events]]).
# Phase 1 Increment 5 slice E ([[TAS-042-phase-1-increment-5-release-benchmarks-profiling]])
# re-measures it independently, before any optimization, as the increment
# baseline.
#
# The pass has no switch: `InteractionMetrics::begin_tick` and `observe` are
# crate-private and `Simulation::advance_one_tick` always calls them, so the
# only way to weigh "the pass" against "the rest of the tick" is to run the same
# fixed-step loop with and without it. This script does that without touching a
# file of this repository: it unpacks `git archive HEAD` into a temporary
# directory, writes a throwaway example driver there, builds the kernel twice —
# as committed, and with the two `self.metrics.*` calls removed from its copy of
# `crates/hekate-sim/src/sim.rs` — and times the identical loop in each build.
# The repository's own `crates/` must be clean before and after, and the two
# builds must report the same event-stream hash and event count, so the ablation
# is shown to change no behavior that a run can observe.
#
# Limitation: removing the calls lets the compiler re-codegen the rest of the
# loop (inlining, register allocation, branch layout), so "rest" is the rest of
# the tick as compiled without the pass, not as compiled with it.
#
# Usage:
#   scripts/measure-tick-phases.sh [SCENARIO] [REPEATS]
#
# Environment:
#   TICKS_WINDOWS  Space-separated tick counts to measure. The default covers
#                  2500 ticks (the short window Increment 4's figure was taken
#                  over), one simulated hour (72 000 Standard steps, the window
#                  `perf/release-bench.json` reports), and 300 000 ticks (the
#                  window `perf/profiles/` sampled).
#   OUTPUT         Artifact path; defaults to perf/tick-phases.json.
#
# Requires: git, tar, python3, a Rust toolchain, and a warm cargo cache.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
scenario=${1:-scenarios/benchmarks/mixed_interaction_v1.json5}
repeats=${2:-5}
windows=${TICKS_WINDOWS:-"2500 72000 300000"}
output=${OUTPUT:-"${repo_root}/perf/tick-phases.json"}

if [[ ! -f "${repo_root}/${scenario}" ]]; then
    echo "error: no such scenario: ${scenario}" >&2
    exit 1
fi

if [[ -n "$(git -C "${repo_root}" status --porcelain -- crates)" ]]; then
    echo "error: crates/ is dirty; the ablation copy must start from HEAD" >&2
    exit 1
fi
base_commit=$(git -C "${repo_root}" rev-parse HEAD)

work=$(mktemp -d "${TMPDIR:-/tmp}/hekate-tick-phases.XXXXXX")
cleanup() { rm -rf "${work}"; }
trap cleanup EXIT

echo "unpacking HEAD (${base_commit:0:12}) into ${work}" >&2
git -C "${repo_root}" archive --format=tar HEAD | tar -x -C "${work}"

# The throwaway driver is written into the temporary copy only. It times the
# bare fixed-step loop, and it hashes the events of one untimed pass so the
# ablation can be checked against the committed kernel.
mkdir -p "${work}/crates/hekate-sim/examples"
cat > "${work}/crates/hekate-sim/examples/tick_phase_bench.rs" <<'RUST'
//! Throwaway ablation driver, written by scripts/measure-tick-phases.sh.
//!
//! Times `Simulation::step` for one scenario and prints key=value lines. The
//! script builds this example twice from one temporary copy of HEAD — once as
//! committed and once with the interaction-metrics pass calls removed — and
//! differences the two builds.

use std::time::Instant;

use hekate_model::{CompiledScenario, parse_scenario_source};
use hekate_sim::{RunConfig, Simulation};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let ticks: u64 = args[2].parse().expect("ticks");
    let repeats: usize = args[3].parse().expect("repeats");

    let text = std::fs::read_to_string(path).expect("the scenario is readable");
    let source = parse_scenario_source(&text).expect("the scenario parses");
    let scenario = CompiledScenario::compile(source).expect("the scenario compiles");
    let config = RunConfig::new(0);

    // One untimed pass: event count, live-agent count, and an FNV-1a hash of
    // the event stream, so the two builds can be shown to observe the same run.
    let mut sim = Simulation::new(scenario.clone(), config).expect("the simulation builds");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut events: u64 = 0;
    let mut agent_steps: u64 = 0;
    for _ in 0..ticks {
        agent_steps += sim.agent_count() as u64;
        let step = sim.step();
        for event in step.events() {
            events += 1;
            for byte in format!("{event:?}").bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }

    // Warm-up, so the measured passes never pay first-touch costs.
    let mut warm = Simulation::new(scenario.clone(), config).expect("the simulation builds");
    for _ in 0..2_000_u64.min(ticks) {
        warm.step();
    }
    drop(warm);

    let mut walls = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let mut sim = Simulation::new(scenario.clone(), config).expect("the simulation builds");
        let start = Instant::now();
        for _ in 0..ticks {
            sim.step();
        }
        walls.push(start.elapsed().as_secs_f64());
    }
    walls.sort_by(f64::total_cmp);

    println!("ticks={ticks}");
    println!("repeats={repeats}");
    println!("wall_min_s={}", walls[0]);
    println!("wall_median_s={}", walls[walls.len() / 2]);
    println!("agent_steps={agent_steps}");
    println!("events={events}");
    println!("event_hash={hash:016x}");
}
RUST

build() {
    (
        cd "${work}"
        CARGO_TARGET_DIR="${work}/target" cargo build --release --offline \
            -p hekate-sim --example tick_phase_bench >/dev/null
    )
}

# The committed kernel, built once and kept aside so both builds can be timed
# against any window without a further rebuild.
build
cp "${work}/target/release/examples/tick_phase_bench" "${work}/tick_phase_bench_standard"

# Ablate the pass in the temporary copy only. Each snippet must appear exactly
# once, so a kernel edit that moves or renames the calls fails loudly here
# instead of silently measuring nothing.
python3 - "${work}/crates/hekate-sim/src/sim.rs" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
removals = [
    "        self.metrics.begin_tick(&self.agents);\n",
    "        self.metrics\n            .observe(&self.agents, self.tick, self.config, &self.events);\n",
]
for snippet in removals:
    if text.count(snippet) != 1:
        sys.exit(f"ablation target is not present exactly once: {snippet!r}")
    text = text.replace(snippet, "")
path.write_text(text)
PY

build
cp "${work}/target/release/examples/tick_phase_bench" "${work}/tick_phase_bench_ablated"

field() {
    printf '%s\n' "$1" | awk -F= -v key="$2" '$1 == key { print $2 }'
}

run() {
    # run <binary> <ticks>
    "${1}" "${repo_root}/${scenario}" "${2}" "${repeats}"
}

mkdir -p "$(dirname "${output}")"
windows_json="${work}/windows.json"
: > "${windows_json}"

for ticks in ${windows}; do
    echo "measuring ${ticks} ticks: committed build" >&2
    standard=$(run "${work}/tick_phase_bench_standard" "${ticks}")
    echo "measuring ${ticks} ticks: ablated build" >&2
    ablated=$(run "${work}/tick_phase_bench_ablated" "${ticks}")

    standard_hash=$(field "${standard}" event_hash)
    ablated_hash=$(field "${ablated}" event_hash)
    standard_events=$(field "${standard}" events)
    ablated_events=$(field "${ablated}" events)
    if [[ "${standard_hash}" != "${ablated_hash}" || "${standard_events}" != "${ablated_events}" ]]; then
        echo "error: the ablated build observes a different run (${standard_events} vs ${ablated_events} events)" >&2
        exit 1
    fi
    agent_steps=$(field "${standard}" agent_steps)

    std_min=$(field "${standard}" wall_min_s)
    std_median=$(field "${standard}" wall_median_s)
    abl_min=$(field "${ablated}" wall_min_s)
    abl_median=$(field "${ablated}" wall_median_s)

    # Every derived number is computed in awk so the shell never does float math.
    derived=$(awk -v n="${ticks}" -v smin="${std_min}" -v smed="${std_median}" \
        -v amin="${abl_min}" -v amed="${abl_median}" 'BEGIN {
            printf "%.4f %.4f %.4f %.4f %.4f %.4f %.4f %.4f %.4f %.4f",
                smin * 1e6 / n, smed * 1e6 / n, amin * 1e6 / n, amed * 1e6 / n,
                (smed - amed) * 1e6 / n, (smed - amed) / smed,
                (smin - amin) * 1e6 / n, (smin - amin) / smin,
                (smed - amed) / amed, (smin - amin) / amin
        }')
    read -r std_min_us std_median_us abl_min_us abl_median_us \
        pass_us pass_share pass_us_min pass_share_min ratio_median ratio_min <<<"${derived}"

    simulated_seconds=$(awk -v n="${ticks}" 'BEGIN { printf "%.1f", n * 0.05 }')
    cat >> "${windows_json}" <<JSON
    {
      "ticks": ${ticks},
      "simulated_seconds": ${simulated_seconds},
      "agent_steps": ${agent_steps},
      "events": ${standard_events},
      "event_hash_committed": "${standard_hash}",
      "event_hash_ablated": "${ablated_hash}",
      "event_stream_identical": true,
      "committed": {
        "wall_min_s": ${std_min},
        "wall_median_s": ${std_median},
        "us_per_tick_min": ${std_min_us},
        "us_per_tick_median": ${std_median_us}
      },
      "ablated": {
        "wall_min_s": ${abl_min},
        "wall_median_s": ${abl_median},
        "us_per_tick_min": ${abl_min_us},
        "us_per_tick_median": ${abl_median_us}
      },
      "pass_us_per_tick": ${pass_us},
      "rest_us_per_tick": ${abl_median_us},
      "pass_share_of_tick": ${pass_share},
      "pass_over_rest_ratio": ${ratio_median},
      "pass_us_per_tick_from_minimum": ${pass_us_min},
      "pass_share_of_tick_from_minimum": ${pass_share_min}
    },
JSON
    echo "  ${ticks} ticks: committed ${std_median_us} us/tick, ablated ${abl_median_us} us/tick, pass ${pass_us} us/tick (${pass_share} of the tick)" >&2
done

# A trailing comma from the loop above is invalid JSON; strip the last one.
python3 - "${windows_json}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text().rstrip()
if text.endswith(","):
    text = text[:-1]
path.write_text(text)
PY

cpu=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)
os_version=$(sw_vers -productVersion 2>/dev/null || uname -r)
logical_cpus=$(sysctl -n hw.logicalcpu 2>/dev/null || echo unknown)
generated=$(date +%s)

if [[ -n "$(git -C "${repo_root}" status --porcelain -- crates)" ]]; then
    echo "error: the repository's crates/ changed during the measurement" >&2
    exit 1
fi

cat > "${output}" <<JSON
{
  "artifact_version": 1,
  "kind": "interaction-metrics-pass-cost",
  "generated_unix_s": ${generated},
  "scenario": "${scenario}",
  "step_s": 0.05,
  "seed": 0,
  "build_profile": "release",
  "machine": {
    "os": "macos",
    "os_version": "${os_version}",
    "arch": "$(uname -m)",
    "cpu": "${cpu}",
    "logical_cpus": ${logical_cpus},
    "rustc": "$(rustc -V)"
  },
  "method": {
    "isolation": "the interaction-metrics pass has no switch: InteractionMetrics::begin_tick and observe are crate-private and Simulation::advance_one_tick always calls them. The measured split is an A/B ablation.",
    "base_commit": "${base_commit}",
    "procedure": [
      "git archive HEAD into a temporary directory",
      "write a throwaway crates/hekate-sim/examples/tick_phase_bench.rs driver into that copy",
      "cargo build --release --offline -p hekate-sim --example tick_phase_bench",
      "remove the two self.metrics.* calls from the copy's crates/hekate-sim/src/sim.rs and rebuild",
      "time the identical bare step loop in both builds over ${repeats} passes per window and report the minimum and the median"
    ],
    "limitation": "removing the calls lets the compiler re-codegen the rest of the loop, so rest is the rest of the tick as compiled without the pass, not as compiled with it",
    "integrity_check": "the repository's crates/ is verified clean before and after, and the two builds must report the same event count and event-stream hash",
    "clock": "std::time::Instant around the bare step loop of a fresh Simulation, excluding scenario load, warm-up, and process start-up",
    "window_note": "the pass has no switch, so no absolute number here is a gate"
  },
  "integrity": {
    "repo_crates_clean_before": true,
    "repo_crates_clean_after": true,
    "ablated_build_observes_the_same_run": true
  },
  "windows": [
$(cat "${windows_json}")
  ],
  "tas_030_reference": {
    "pass_us_per_tick": 22.0,
    "rest_us_per_tick": 7.0,
    "source": ".tangle/resolved/TAS-030-phase-1-increment-4-geometry-queries-safety-events.md, Measured cost",
    "note": "Increment 4's figure, re-measured here independently; this artifact is the Increment 5 baseline"
  }
}
JSON

echo "tick-phase artifact: ${output}" >&2
