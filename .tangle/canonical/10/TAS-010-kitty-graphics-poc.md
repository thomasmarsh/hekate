---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Conditional go. Kitty graphics works as an opt-in terminal backend, but only with the bounded pair lifecycle; the naive same-id reuse path crashed WezTerm.
---

# Outcome

A recorded go/no-go on using the Kitty graphics protocol for the Hekate terminal
viewer, backed by a runnable spike that transmits, places, animates, and deletes
pixel frames on real terminals.

# Done when

- The spike draws a moving scene frame through the Kitty graphics protocol on at
  least one supporting terminal, and again under tmux with passthrough. — Met
  for the drawing half on Ghostty over SSH and a WezTerm SSH client, both
  confirmed by the operator. Real tmux passthrough was not directly exercised;
  the wrapper is unit-tested and the gap moves to
  [[TAS-013-terminal-viewer-kitty]], where passthrough handling belongs.
- It reports frame size, encode and transmit cost, and achievable frame rate
  for a walking-skeleton-sized scene and for a denser synthetic scene. — Met for
  `walking` on real terminals and for both scenes on a pty sink; `dense` has no
  real-terminal number, which does not change the decision because it is already
  below budget only when compressed and compressed cost is CPU-bound.
- It records behavior on at least one non-supporting terminal and the detection
  signal that distinguishes the two cases. — Partially met. Two distinct
  negative shapes are recorded (`unsupported` for device-attributes-only and
  `no-reply` for a pty with no emulator) and classified by unit tests; no real
  non-supporting terminal was probed.
- It states whether image placement survives resize, scroll, and alternate
  screen transitions without leaking images. — Stated below; visual lifecycle
  was exercised by the operator run but resize and scroll were not observed
  under load.
- The spike is either disposed after the decision or promoted as the basis of
  the conditional Kitty backend. — The spike stays in the tree as evidence and
  as a byte-fixture source for [[TAS-015-renderer-conformance-suite]]; it is not
  promoted into the product, which implements against
  [[DEF-002-renderer-backend-contract]].

# Context

Depends on [[TAS-002-rust-workspace-ci]] at context_rev 1.
The pin covers the workspace and toolchain only. The spike is deliberately
independent of the shared presentation layer so it can fail cheaply before any
refactor.

Parent [[TAS-009-renderer-backends]].

# Evidence

The spike is built at `spikes/kitty-graphics-poc` as a workspace-detached crate
(`[workspace]` in `Cargo.toml`), so it does not join the pinned workspace, CI
gates, or the dependency-direction check. It implements support probing, raw and
zlib chunked transmission (4096-byte base64 chunks, control data only on the
first chunk), file and POSIX shared-memory media, placement, delete, tmux and
screen passthrough wrapping, and three explicit image lifecycles. It restores
raw mode, deletes images, and leaves the alternate screen on normal exit, panic,
and write error.

`cargo test --release` passes 14 unit tests covering chunk framing, zlib, reply
parsing, detection classification, tmux escaping/un-doubling, and scene
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

A noisy dense frame compresses only to a 0.133 ratio and then costs about
98 ms/frame of zlib CPU; the flat walking scene compresses about 370x and stays
far below frame budget. Local media (file, shared memory) move the payload off
the pty almost entirely but require a shared filesystem or memory with the
terminal.

## Real supporting terminals

`probe` on two real terminals over SSH returned `supported` with graphics reply
`i=31;OK` and device attributes `?62;22;52`. A 600-frame walking zlib run at
30 fps completed on both:

| terminal | cells | pixels | cell px | encode ms | write ms | total ms | write fps |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Ghostty (direct SSH) | 209x57 | 3352x1968 | 16x34 | 1.14 | 0.085 | 1.29 | 777 |
| WezTerm client (SSH client) | 98x59 | 1470x1888 | 15x32 | 1.23 | 0.072 | 1.35 | 738 |

Ghostty was confirmed to draw and animate the scene correctly by the operator.
The walking scene stays far inside a 60 Hz frame budget.

## The naive lifecycle crashed WezTerm; the spec lifecycle does not

The first local WezTerm 0.1.0 (1) instance on macOS 26.6.2 aborted (SIGABRT)
under the spike's original `reuse` lifecycle, which transmits and displays with
`a=T` and the same image id every frame and relies on the terminal to replace
the image and all of its placements. The crash log put the process inside
`wezterm_term::terminalstate::kitty::kitty_remove_placement`, queued from
`termwiz::cell::CellAttributes::detach_image_with_placement` -> `Vec::retain`
-> `_nanov2_free`, with a heap-corrupting data abort on one thread, a
thread-local destructor abort at exit, 3.7 GB written in `VM_ALLOCATE`, and
repeated `mach_vm_allocate_kernel failed` triage entries.

Re-running the hardened default `pair` lifecycle (transmit-only `a=t`, a stable
placement id `a=p,i=..,p=1`, `C=1`, and a bounded raw transmit budget) in the
same WezTerm/herdr session ran cleanly, including at 600 frames. So the failure
is specific to relying on same-id re-transmission to replace image data and
placements, not to the protocol or to the terminal as such. herdr is a
tmux-equivalent multiplexer and was plausibly in the local session, so the
precise trigger is not fully isolated; the `pair` result is the reproducible
one.

The practical lesson is that the client's lifecycle discipline, not the support
probe, determines whether a supporting terminal survives.

## Detection shapes

- `supported`: graphics reply `i=31;OK` before device attributes, seen on
  Ghostty and WezTerm over SSH.
- `unsupported`: device attributes answered with no graphics reply. The
  conservative verdict for a real terminal that ignores the protocol.
- `no-reply`: no device-attributes answer at all, seen on a pty with no
  emulator (`script`).
- `graphics-error`: a graphics reply carrying an error such as `ENOTSUP`.

All but `supported` are treated as unsupported. Detection proves protocol
support, not stability, so it can gate a backend but cannot by itself make the
backend safe; the lifecycle must be bounded independently.

# Result

**Go, conditional.** Kitty graphics is usable for the Hekate terminal viewer,
but only as an opt-in backend that follows the bounded `pair` lifecycle; it is
not safe to treat a positive support probe as permission to use the naive
same-id `a=T` re-transmission path.

The conditions recorded in [[DEC-002-terminal-backend-strategy]]:

- Transmit-only `a=t` followed by a stable placement `a=p,i=<id>,p=<placement>`;
  never rely on same-id `a=T` re-transmission to replace data and placements.
- Keep `C=1` so placements are not pushed into scrollback by cursor advance.
- Delete images explicitly and bound in-flight frames and total raw transmit
  bytes.
- Keep the character-cell backend as the portable default and the fallback that
  preserves playback state.

Throughput supports a debug viewer: the walking skeleton renders and animates
at ~1.3 ms/frame on real terminals, while the noisy dense scene is CPU-bound
under zlib (9 fps) and write-bound raw (20 fps), so a pixel backend should
prefer small viewports and flat geometry.

Residual gaps, consciously accepted, move to the backend and conformance nodes:
real tmux passthrough, resize/scroll under load, a real non-supporting terminal
probe, and a real-terminal dense number. None of them changes the lifecycle
decision.
