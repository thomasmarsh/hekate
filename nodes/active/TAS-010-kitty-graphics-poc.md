---
context_rev: 1
priority: P1
updated: 2026-09-12T12:54:54Z
summary: Spike built and benchmarked on a pty sink; real-terminal verification and the go/no-go remain.
next: Run the spike in a supporting terminal and under tmux, then record the go/no-go.
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

## Still to verify on real terminals

- A supporting terminal (Ghostty is installed on this host) must report
  `supported` and draw the moving scene; record encode/transmit/frame rate.
- tmux with `allow-passthrough on` and `--mux tmux`; passthrough off must not
  silently corrupt.
- A non-supporting terminal must report `unsupported` and draw nothing, and
  `--force` must not produce garbage.
- Resize, scroll, and alternate-screen transitions must not leak images.

The visual and multiplexer checks need a person at a real terminal, so the
spike is handed off for that run before the go/no-go is recorded here.
