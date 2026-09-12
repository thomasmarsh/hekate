# Kitty graphics go/no-go spike (TAS-010)

A disposable, dependency-light Rust program that proves or disproves the Kitty
graphics protocol as a Tangle terminal backend. It is **detached from the
Tangle workspace** (`[workspace]` in `Cargo.toml`) on purpose: it must not join
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
`--fps N` (`0` is uncapped), `--mux auto|none|tmux|screen`, `--fence`,
`--force`, `--no-alt`, `--timeout-ms N`, `--report PATH`.

`--fence` round-trips a query after every frame and reports a
terminal-processing rate in addition to the raw write rate. It only works where
the terminal answers the query (not under a multiplexer that swallows replies).

The program deletes all images, leaves the alternate screen, and restores the
old termios settings on a normal exit, on `Ctrl-C`/panic, and after a write
error.

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
exclude the terminal's own parse/blit cost. Real-terminal numbers come from
running the demo in a supporting terminal.

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
