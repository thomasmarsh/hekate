---
context_rev: 1
priority: P1
updated: 2026-09-12T13:03:54Z
summary: Spike runs on Ghostty and two SSH clients; one WezTerm crash reframes the decision as explicit-opt-in only.
next: Run probe in a non-supporting terminal and the tmux demo, then record the go/no-go.
---

# Outcome

A recorded go/no-go on using the Kitty graphics protocol for the Tangle terminal
viewer, backed by a runnable spike that transmits, places, animates, and deletes
pixel frames on real terminals.

# Done when

- The spike draws a moving scene frame through the Kitty graphics protocol on at
  least one supporting terminal, and again under tmux with passthrough.
- It reports frame size, encode and transmit cost, and achievable frame rate
  for a walking-skeleton-sized scene and for a denser synthetic scene.
- It records behavior on at least one non-supporting terminal and the detection
  signal that distinguishes the two cases.
- It states whether image placement survives resize, scroll, and alternate
  screen transitions without leaking images.
- The spike is either disposed after the decision or promoted as the basis of
  the conditional Kitty backend.

# Context

Depends on [[TAS-002-rust-workspace-ci]] at context_rev 1.
The pin covers the workspace and toolchain only. The spike is deliberately
independent of the shared presentation layer so it can fail cheaply before any
refactor.

Parent [[TAS-009-renderer-backends]].

# Progress

The spike is built at `spikes/kitty-graphics-poc` as a workspace-detached crate
(`[workspace]` in `Cargo.toml`), so it does not join the pinned workspace, CI
gates, or the dependency-direction check. It implements support probing, raw and
zlib chunked transmission (4096-byte base64 chunks, control data only on the
first chunk), file and POSIX shared-memory media, placement, delete, and tmux
and screen passthrough wrapping. It restores raw mode, deletes images, and
leaves the alternate screen on normal exit, panic, and write error.

`cargo test --release` passes 14 unit tests covering chunk framing, zlib,
reply parsing, detection classification, tmux escaping/un-doubling, and scene
determinism and compressibility. `cargo clippy --release -- -D warnings` and
`cargo fmt --check` are clean.

## Measured on a pty sink (no terminal emulator)

Measured through `script -q /dev/null`, which isolates raster, encode, and write
cost but excludes the terminal's own parse/blit cost. `walking` is 480x270 flat
debug geometry; `dense` is 960x540 with deterministic per-pixel sensor noise so
it is a genuine compression worst case.

| scene | transport | data/frame | pty/frame | seq/frame | raster ms | encode ms | write ms | total ms | write fps |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| walking | raw | 518400 | 692752 | 169 | 0.07 | 0.45 | 14.68 | 15.19 | 65.8 |
| walking | zlib | 1393 | 1903 | 1 | 0.06 | 1.32 | 0.06 | 1.44 | 694.1 |
| walking | file | 518400 | 147 | 1 | 0.06 | 0.40 | 0.01 | 0.47 | 2115.5 |
| walking | shm | 518400 | 76 | 1 | 0.04 | 0.25 | 0.00 | 0.30 | 3361.6 |
| dense | raw | 2073600 | 2770906 | 675 | 5.36 | 2.63 | 41.27 | 49.26 | 20.3 |
| dense | zlib | 276111 | 368998 | 90 | 5.33 | 97.66 | 6.52 | 109.51 | 9.1 |

Two results dominate the decision. A noisy dense frame compresses only to a
0.133 ratio and then costs about 98 ms/frame of zlib CPU, so compression alone
does not make the protocol cheap for a realistic scene; the walking-skeleton
scene compresses about 370x and stays far below frame budget. The local media
(file, shared memory) move the payload off the pty almost entirely, leaving only
the medium write and the terminal's read on the critical path, but they require
a shared filesystem or shared memory with the terminal.

A `probe` run on a pty with no terminal emulator (`script`) reports `no-reply`:
no device-attributes answer at all. That is distinct from a real terminal that
answers device attributes but not the graphics query, which must classify as
`unsupported`. Both are treated as unsupported by the spike.

## Real supporting terminals

`probe` on two real terminals over SSH returned `supported` with graphics reply
`i=31;OK` and device attributes `?62;22;52`. A 600-frame walking zlib run at
30 fps completed on both:

| terminal | cells | pixels | cell px | encode ms | write ms | total ms | write fps |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Ghostty (direct SSH) | 209x57 | 3352x1968 | 16x34 | 1.14 | 0.085 | 1.29 | 777 |
| WezTerm client (SSH client) | 98x59 | 1470x1888 | 15x32 | 1.23 | 0.072 | 1.35 | 738 |

Ghostty was confirmed to draw and animate the scene correctly by the operator.
The walking scene stays far inside a 60 Hz frame budget, so throughput is not
the limiting factor for a debug view.

## A nominally supporting terminal crashed

A **local WezTerm 0.1.0 (1) instance on macOS 26.6.2 aborted (SIGABRT)** roughly
two minutes into this test series, while the SSH client instance had just
completed 600 frames. The crash log shows the process inside
`wezterm_term::terminalstate::kitty::kitty_remove_placement`, queued from
`termwiz::cell::CellAttributes::detach_image_with_placement` -> `Vec::retain`
-> `_nanov2_free`, with a heap-corrupting data abort on one thread and a
thread-local destructor abort at exit. The VM summary reports 3.7 GB written in
`VM_ALLOCATE` and repeated `mach_vm_allocate_kernel failed` triage entries.

This was not a malformed sequence: the same bytes were accepted by Ghostty and
by the SSH-terminal WezTerm instance. It is a memory-safety failure in WezTerm's
placement-removal path, reached by the ordinary re-transmit/placement lifecycle
that any animating client must use. The operator believes the local session may
have involved a multiplexer, so the precise trigger is not isolated; the
observation stands as recorded.

Consequence for the decision: a terminal can answer the support probe `OK` and
still be unsafe. Detection proves protocol support, not stability, so a
runtime "support is proven" rule is not sufficient to auto-select the backend.
The spike now defaults to a bounded `pair` lifecycle (transmit-only plus a
stable placement id), keeps `C=1` so the cursor does not push placements into
scrollback, and stops after a 256 MiB raw transmit budget.

## Still to verify on real terminals

- A non-supporting terminal (for example macOS Terminal.app) must report
  `unsupported` and draw nothing; the only negative evidence so far is a pty
  with no emulator, which reports `no-reply`.
- tmux with `allow-passthrough on` and `--mux tmux`; passthrough off must not
  silently corrupt.
- Resize, scroll, and alternate-screen transitions must not leak images.
- A real-terminal dense-scene number, since the dense scene is the one that
  exceeds a frame budget under zlib.
