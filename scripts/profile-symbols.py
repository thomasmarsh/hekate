#!/usr/bin/env python3
"""Fold a macOS `sample` capture into a tick-phase table.

`sample` writes one row per frame with the number of samples that frame was on
the stack for, in Rust's legacy mangled form, and it splits one symbol into
several rows when the frames under it differ. This folds the capture into the
phase table `perf/README.md` documents: the samples under `Simulation::step`,
every direct child of it with its share of the tick, the tick body's own self
time, the always-on interaction-metrics pass as one line, and the frames inside
that pass with their share of it.

The within-pass table folds rows by terminal symbol name, because a symbol is
split across rows and an inlined instantiation carries the name of the function
it was leaf-copied into: the TTC bisection's `first_fraction` closure is one row
per call site and reports the bisection, not `first_fraction`. The rows overlap
— a frame's samples include the calls it makes — so the column does not sum to
the pass total.

Sampling attributes inlined callee time to the enclosing frame, so read the table
as an attribution of the hot path, not as an exact split: the split that does not
depend on inlining is the ablation in `perf/tick-phases.json`.

Usage: profile-symbols.py CAPTURE [TOP]
"""

import collections
import re
import sys

FRAME = re.compile(r"^(\s*)([+!:|\s]*?)\s*(\d+)\s")
MARKERS = re.compile(r"[+!:|]")
DIGITS = re.compile(r"(\d+)")
# Rust legacy mangling is length-prefixed, but the hash segments before the
# crate path also contain digits. Decoding starts at a known crate root so the
# hash cannot be mistaken for identifiers.
ROOT = re.compile(r"(?:^|_)(3std|5alloc|4core|6object|11rand_chacha|10tangle_sim|10tangle_cli|12tangle_model)")
STEP = re.compile(r"_10tangle_sim3simNtB5_10Simulation4step$")
# The always-on per-tick interaction-metrics pass, as its frame paths start.
PASS = "tangle_sim::metrics::"


def pretty(symbol: str) -> str:
    """Decode Rust legacy mangling into a `crate::module::function` path."""
    root = ROOT.search(symbol)
    tail = symbol[root.start(1):] if root is not None else symbol
    parts = []
    position = 0
    while True:
        match = DIGITS.search(tail, position)
        if match is None:
            break
        start = match.end()
        length = int(match.group(1))
        name = tail[start:start + length]
        # A path element is `<length><identifier>` and always starts with a
        # letter. A mangling marker such as `NtB5_` contains digits too, but the
        # run after it starts with `_` or is not exactly `length` long.
        if len(name) == length and name[:1].isalpha():
            # Stop at the first element that cannot be a path element, so a
            # base62 hash segment inside a generic instantiation does not leak
            # into the path.
            if re.fullmatch(r"[a-z][a-z0-9_]*|[A-Z][A-Za-z0-9]*", name) is None:
                break
            parts.append(name)
            position = start + length
        else:
            position = start
    if root is None:
        return tail.lstrip("_")
    return "::".join(parts) if parts else tail


def header(lines, field):
    for line in lines:
        if line.startswith(field):
            return line[len(field):].strip()
    return "unknown"


def frames(lines):
    """Parse the call graph into `(depth, samples, symbol)` rows in print order."""
    try:
        start = lines.index("Call graph:")
    except ValueError:
        sys.exit("no call graph in the capture")
    end = len(lines)
    for index in range(start, len(lines)):
        if lines[index].startswith("Total number in stack"):
            end = index
            break
    rows = []
    for line in lines[start + 1:end]:
        match = FRAME.match(line)
        if match is None:
            continue
        depth = len(MARKERS.findall(match.group(2)))
        symbol = line[match.end():].split("  (in ")[0].strip()
        symbol = re.sub(r" \+ [0-9,\.]+.*$", "", symbol)
        rows.append((depth, int(match.group(3)), symbol))
    return rows


def self_time_frames(lines):
    """The `Sort by top of stack` table: samples by leaf frame."""
    try:
        start = lines.index("Sort by top of stack, same collapsed (when >= 5):")
    except ValueError:
        return []
    rows = []
    for line in lines[start + 1:]:
        if line.startswith("Binary Images"):
            break
        match = re.match(r"^\s*(\S.*?)\s+\(in .*\)\s+(\d+)\s*$", line)
        if match is not None:
            rows.append((int(match.group(2)), match.group(1)))
    return rows


def in_pass(rows, parents, pass_frames):
    """The rows below a pass frame, excluding the pass frames themselves.

    A pass frame's samples already are the pass's cost, so the table reads as
    what the pass's frames call: the rows a frame reaches, at any depth.
    """
    frames = set(pass_frames)
    inside = []
    for index in range(len(rows)):
        parent = parents[index]
        while parent is not None:
            if parent in frames:
                inside.append(index)
                break
            parent = parents[parent]
    return inside


def main() -> None:
    path = sys.argv[1]
    top = int(sys.argv[2]) if len(sys.argv) > 2 else 12
    lines = open(path).read().splitlines()
    rows = frames(lines)
    if not rows:
        sys.exit("no frames in the capture")

    # Parent links, by marker depth.
    parents = []
    stack = []
    for index, (depth, _, _) in enumerate(rows):
        while stack and stack[-1][0] >= depth:
            stack.pop()
        parents.append(stack[-1][1] if stack else None)
        stack.append((depth, index))

    step_rows = [index for index, (_, _, symbol) in enumerate(rows) if STEP.search(symbol)]
    if not step_rows:
        sys.exit("no Simulation::step frame in the capture")
    root_steps = [index for index in step_rows if not any(
        other in step_rows and index != other and is_ancestor(parents, index, other)
        for other in step_rows
    )]
    tick_samples = sum(rows[index][1] for index in root_steps)

    children = collections.Counter()
    for index, (_, samples, symbol) in enumerate(rows):
        parent = parents[index]
        if parent in root_steps and not STEP.search(symbol):
            children[pretty(symbol)] += samples
    child_samples = sum(children.values())
    tick_self = tick_samples - child_samples
    pass_frames = [
        index
        for index in range(len(rows))
        if parents[index] in root_steps and pretty(rows[index][2]).startswith(PASS)
    ]
    pass_samples = sum(rows[index][1] for index in pass_frames)
    rest_samples = tick_samples - pass_samples

    # Folded by terminal symbol name: an inlined instantiation reports the
    # symbol of the function it was copied into, so one callee appears under
    # several paths and both belong to the callee's cost.
    within = collections.Counter()
    for index in in_pass(rows, parents, pass_frames):
        within[pretty(rows[index][2]).rsplit("::", 1)[-1]] += rows[index][1]

    def share(samples):
        return f"{100.0 * samples / tick_samples:5.1f}%"

    def pass_share(samples):
        return f"{100.0 * samples / pass_samples:5.1f}%"

    print("# release-mode sampled profile phase table")
    print(f"capture: {path}")
    print(f"process: {header(lines, 'Process:')}")
    print(f"date: {header(lines, 'Date/Time:')}")
    print(f"os: {header(lines, 'OS Version:')}")
    print(f"tool: {header(lines, 'Analysis Tool:')} (default 1 ms sampling interval)")
    print(f"thread_samples: {rows[0][1]}")
    print()
    print("## tick phases: every direct child of Simulation::step")
    print(f"tick_samples: {tick_samples}")
    print(f"{'frame':<72}{'samples':>9}{'share':>8}")
    for name, samples in children.most_common():
        print(f"{name[:70]:<72}{samples:>9}{share(samples):>8}")
    print(f"{'Simulation::step (self: inlined and non-frame work)':<72}{tick_self:>9}{share(tick_self):>8}")
    print()
    print(f"{'interaction-metrics pass (tangle_sim::metrics::*)':<72}{pass_samples:>9}{share(pass_samples):>8}")
    print(f"{'rest of the tick':<72}{rest_samples:>9}{share(rest_samples):>8}")
    print()
    print("## within-pass frames: what the interaction-metrics pass calls")
    print(f"pass_samples: {pass_samples}")
    print(f"{'frame (terminal symbol)':<72}{'samples':>9}{'share':>8}")
    for name, samples in within.most_common()[:top]:
        print(f"{name[:70]:<72}{samples:>9}{pass_share(samples):>8}")
    print()
    print("## top self-time frames (top of stack)")
    print(f"{'frame':<72}{'samples':>9}")
    for samples, symbol in sorted(self_time_frames(lines), reverse=True)[:top]:
        print(f"{pretty(symbol)[:70]:<72}{samples:>9}")


def is_ancestor(parents, index, candidate):
    """Whether `candidate` is on the parent chain of `index`."""
    current = parents[index]
    while current is not None:
        if current == candidate:
            return True
        current = parents[current]
    return False


if __name__ == "__main__":
    main()
