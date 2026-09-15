# Kitty graphics go/no-go spike (TAS-010)

A disposable, dependency-light Rust program that proves or disproves the Kitty
graphics protocol as a Hekate terminal backend. It is **detached from the
Hekate workspace** (`[workspace]` in `Cargo.toml`) on purpose: it must not join
the pinned workspace, the CI gates, or the dependency-direction check, and it is
independent of the shared presentation layer so it can fail cheaply.

The spike transmits, places, animates, and deletes synthetic RGBA frames, and
measures frame size, encode cost, transmit cost, and achievable frame rate for a
walking-skeleton-sized scene and a denser, noisy worst-case scene.

## Build

```sh
cd spikes/kitty-graphics-poc
cargo build --release
cargo test --release
```

Unit tests cover chunk framing (4096-byte base64 chunks, control data only on
the first chunk, multiple-of-four rule), zlib compression, reply parsing,
detection classification, tmux passthrough escaping and un-doubling, and scene
determinism/compressibility.

## Proving support

```sh
./target/release/kitty-graphics-poc probe --report /tmp/tas010-probe.json
```

Verdicts:

| verdict | meaning |
| --- | --- |
| `supported` | terminal answered `a=q` with `OK` before device attributes |
| `unsupported` | device attributes answered, no graphics reply |
| `graphics-error` | graphics reply carried an error such as `ENOTSUP` |
| `no-reply` | no device-attributes reply at all (a pty with no emulator, e.g. `script`) |

Anything other than `supported` must be treated as unsupported. Exit status is
`0` for supported, `3` for unsupported, `4` when there is no controlling tty.

## Running the spike

```sh
# Animated, watchable demo (30 fps, alternate screen, auto-exits):
./target/release/kitty-graphics-poc run --scene walking --transport zlib \
  --frames 600 --fps 30 --report /tmp/tas010-walking.json

# Worst-case synthetic scene (incompressible sensor noise baked in):
./target/release/kitty-graphics-poc run --scene dense --transport zlib \
  --frames 300 --fps 30 --report /tmp/tas010-dense.json

# Uncapped throughput:
./target/release/kitty-graphics-poc run --scene walking --transport zlib \
  --frames 300 --fence --report /tmp/tas010-bench.json

# Compare transports:
for t in raw zlib file shm; do
  ./target/release/kitty-graphics-poc run --scene walking --transport "$t" \
    --frames 120 --report "/tmp/tas010-$t.json"
done
```

Options: `--transport raw|zlib|file|shm`, `--scene walking|dense`, `--frames N`,
`--fps N` (`0` is uncapped), `--lifecycle pair|reuse|rotate`, `--cursor stay|move`,
`--budget-mb N` (`0` disables), `--mux auto|none|tmux|screen`, `--fence`,
`--force`, `--no-alt`, `--timeout-ms N`, `--report PATH`.

### Lifecycle is the dangerous part

The protocol's image/placement lifecycle has the sharpest failure modes, so the
spike makes it explicit and bounded:

- `pair` (default): transmit-only `a=t` then place with a stable placement id
  `a=p,i=..,p=1`. This is the spec's flicker-free path; re-transmitting the id
  replaces the data and the stable placement id replaces rather than stacks.
- `reuse`: transmit-and-display `a=T` with one id every frame, trusting the
  terminal to replace the image and all its placements (the spec says it must).
- `rotate`: alternate two ids and delete the id before reuse, so at most two
  images are live even if replacement is buggy.

`--cursor stay` (default) adds `C=1` so the terminal does not advance the
cursor past the placement and push it into scrollback. `--budget-mb` (default
256) stops a run after that much raw frame data so a run cannot drive a
terminal into unbounded memory growth.

The program deletes all images, leaves the alternate screen, and restores the
old termios settings on a normal exit, on `Ctrl-C`/panic, and after a write
error.

## Known terminal failure: WezTerm 0.1.0 on macOS

A local WezTerm 0.1.0 (1) instance aborted (SIGABRT) under this spike on macOS
26.6.2, after the same SSH client had completed 600 frames successfully. The
crash log shows the terminal in
`wezterm_term::terminalstate::kitty::kitty_remove_placement` queued from
`CellAttributes::detach_image_with_placement` -> `Vec::retain` -> `_nanov2_free`,
with a heap-corrupting data abort on one thread and a thread-local destructor
abort on exit, and a VM region summary of 3.7 GB written in `VM_ALLOCATE`. That
is a memory-safety failure inside WezTerm's placement-removal path, not a
malformed escape sequence; the same bytes were accepted by Ghostty and by the
SSH client over the same session. Treat WezTerm 0.1.0 as unsafe for this
protocol until it is retested, and never run an unbounded frame count there.

## Under tmux

tmux must forward the protocol, so it requires `allow-passthrough on` **and**
the `--mux tmux` wrapper (DCS `ESC Ptmux;` with every ESC doubled):

```sh
tmux set-option -g allow-passthrough on
./target/release/kitty-graphics-poc run --scene walking --transport zlib \
  --frames 600 --fps 30 --mux tmux --report /tmp/tas010-tmux.json
```

Run the same command with `--mux none` to see the passthrough-off behavior
(garbled or missing images). GNU screen uses `--mux screen`; its per-DCS size
limit makes large frames unreliable, which the spike does not hide.

## Visual verification checklist

With the demo running in a supporting terminal:

- A dark scene appears at the top-left with a coloured border; six cars move
  left to right on a road; a progress bar grows along the bottom.
- The image replaces itself in place each frame with no visible tearing or
  growing number of stacked copies.
- Resizing the window re-rasterizes the frame at the new pixel size.
- After the demo exits, the previous screen content is restored and no image
  remains (alternate screen, image deleted).
- In a non-supporting terminal, `probe` reports `unsupported` and nothing is
  drawn; with `--force`, no garbage should appear either (APC is ignored).

## Measured evidence

Values below are from a pty sink with no terminal emulator behind it
(`script -q /dev/null`), so they isolate raster, encode, and write cost; they
exclude the terminal's own parse/blit cost. The `reuse` rows are the original
lifecycle; the hardened `pair` default sends one extra placement sequence per
frame.

| scene | transport | frame | bytes/frame (data) | bytes/frame (pty) | seq/frame | raster ms | encode ms | write ms | total ms | write fps |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| walking | raw | 480x270 | 518400 | 692752 | 169 | 0.07 | 0.45 | 14.68 | 15.19 | 65.8 |
| walking | zlib | 480x270 | 1393 | 1903 | 1 | 0.06 | 1.32 | 0.06 | 1.44 | 694.1 |
| walking | file | 480x270 | 518400 | 147 | 1 | 0.06 | 0.40 | 0.01 | 0.47 | 2115.5 |
| walking | shm | 480x270 | 518400 | 76 | 1 | 0.04 | 0.25 | 0.00 | 0.30 | 3361.6 |
| dense | raw | 960x540 | 2073600 | 2770906 | 675 | 5.36 | 2.63 | 41.27 | 49.26 | 20.3 |
| dense | zlib | 960x540 | 276111 | 368998 | 90 | 5.33 | 97.66 | 6.52 | 109.51 | 9.1 |

The dense scene carries deterministic per-pixel sensor noise, so zlib reaches
only a 0.133 ratio and then costs ~98 ms/frame of CPU; a flat debug scene
compresses ~370x. This is the central trade-off the decision turns on.

## Real supporting terminals

Both runs were over SSH with `--scene walking --transport zlib --frames 600
--fps 30`; the probe reported `i=31;OK` with device attributes `?62;22;52`.

| terminal | cells | pixels | cell px | encode ms | write ms | total ms | write fps |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Ghostty (direct SSH) | 209x57 | 3352x1968 | 16x34 | 1.14 | 0.085 | 1.29 | 777 |
| WezTerm client (SSH client) | 98x59 | 1470x1888 | 15x32 | 1.23 | 0.072 | 1.35 | 738 |

In both, the walking scene stayed far inside a 60 Hz frame budget; the image
rendered and animated correctly, and Ghostty was reported working visually.
Real-terminal dense-scene and tmux numbers are not yet recorded.
