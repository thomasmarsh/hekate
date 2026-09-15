---
context_rev: 1
status: active
updated: 2026-09-15T14:00:22Z
summary: Stop the Kitty TUI periodic full-image delete from blanking the scene between frames
next: Run the five-gate validation and resolve.
---

Area [[IDX-001-hekate]].

# Outcome

The Kitty graphics backend never presents a frame with no live image: each
present transmits and places the new image id before deleting the previous one,
so the scene area is never blank for a transmit.

# Done when

- `KittyBackend` alternates two image ids per present (transmit+place new id,
  then delete the previous id).
- The raw-transmit budget delete (`MAX_RAW_BYTES`) is removed or neutralized so
  no periodic full-image delete interrupts the stream.
- Teardown/resize/shutdown delete every id that could still be live.
- Focused byte-order tests prove the new placement precedes the previous
  delete, and that no present deletes the current live id before placing a
  replacement.
- `cargo fmt`, `cargo clippy -p hekate-tui`, `cargo test -p hekate-tui`, and
  `./scripts/check-dependency-direction.sh` pass.

# Evidence

Diagnosis and mechanism: `~/.pi/agent/sessions/--Users-thomasmarsh-git-hekate--/subagent-artifacts/outputs/ec8dd865-2ed1-45e3-9387-145cae5f1114/recon/kitty-flash.md`.

The periodic flash is the raw-transmit budget deleting the live image and
flushing the delete on its own before the next frame is transmitted
(`apps/hekate-tui/src/kitty.rs`, `KittyBackend::present` / `delete_live_image`).

# Result

`KittyBackend` now alternates between two image ids
(`IMAGE_IDS = [1, 2]`, `apps/hekate-tui/src/kitty.rs`). Each present transmits
and places the next id, then deletes the previous id in the same buffered write,
so the previous placement stays on screen until the replacement lands. The
`MAX_RAW_BYTES` budget and its periodic full delete are removed; `live_image_id`
(`Option<u32>`) replaces the old `image_id`/`image_live` pair. The first frame
has no previous id, and `resize_terminal`/`shutdown` delete the single live id.
The `a=t`/`a=p` encoding, mux wrapping, and HUD text are unchanged.

Proof: `cargo clippy -p hekate-tui --all-targets --all-features -- -D warnings`,
`cargo test -p hekate-tui`, and `./scripts/check-dependency-direction.sh` all
pass; the new `kitty.rs` tests `a_present_places_the_new_id_before_deleting_the_previous_one`
and `no_present_deletes_a_live_id_without_first_placing_its_replacement` assert
the byte-order guarantee. `tests/golden/renderer/kitty_present.bin` is unchanged
(a first-frame present still uses id 1).

Closure edit: `apps/hekate-tui/src/lib.rs` drops the `MAX_RAW_BYTES` re-export
(the compiler-forced consequence of removing the constant).

Left out: `apps/hekate-tui/src/main.rs`'s redundant `Clear(ClearType::All)` on
Resize is untouched in this slice because it cannot be proven not to affect the
cell backend here.
