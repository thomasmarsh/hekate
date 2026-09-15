---
context_rev: 1
status: resolved
updated: 2026-09-15T14:07:54Z
summary: Stop the Kitty TUI periodic full-image delete from blanking the scene between frames
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

## Gate validation (HEAD bd5a906, 2026-09-15T14:07:54Z)

TOON, from the repository root:

```
gates[5]{gate,exit,wall_s,result}:
  "cargo fmt --all --check",0,2,passed
  "cargo clippy --workspace --all-targets --all-features -- -D warnings",0,5,passed
  "cargo test --workspace --all-features",0,354,passed
  "./scripts/check-dependency-direction.sh",0,0,passed
  "tangle check",0,1,passed
```

Full suite: 97 result lines, 1056 passed, 0 failed, 3 ignored (no `FAILED`,
no `error[`). Focused: `kitty::tests` 16 passed, 0 failed.

## Clause disposition

- **Two ids per present** — met. `IMAGE_IDS = [1, 2]`; `next_image_id` returns
the id that is not live, and `present` appends `push_delete(previous_id)` after
the transmit/place sequences in the same `out` buffer, before the single
`write_all`/`flush`. Ids alternate `1, 2, 1, …`.
- **No quota-driven periodic delete** — met. `MAX_RAW_BYTES`, the budget block
in `present`, and the `lib.rs` re-export are gone; the only remaining
references are this node's own prose. `raw_bytes_sent` is accounting only.
- **Teardown deletes every id that could still be live** — met. The in-frame
delete keeps at most one id live, so `resize_terminal` and `shutdown` delete
that one id via `delete_live_image`; `resize_deletes_the_old_placement` and
`shutdown_deletes_the_live_image` pass. The TUI calls `shutdown` on exit
(`apps/hekate-tui/src/main.rs`).
- **Byte-order tests** — met.
`a_present_places_the_new_id_before_deleting_the_previous_one` and
`no_present_deletes_a_live_id_without_first_placing_its_replacement` assert the
placement index precedes the previous id's delete index, and that the first
frame deletes nothing; both pass in the workspace run.
- **Focused fmt/clippy/test/dependency-direction** — met, and strengthened to
workspace scope in the table above.
- **Kitty golden unchanged** — met. `git diff a7c31c3 bd5a906 -- tests/golden`
is empty and `tests/golden/renderer/kitty_present.bin` still holds (the golden
is a single first-frame present, which still uses id 1).

Residual risk: the ordering guarantee is proven by byte-stream assertions only;
no live Kitty terminal was exercised, so terminal-side coalescing is inferred
from the protocol. `main.rs`'s redundant `Clear(ClearType::All)` on Resize
remains out of scope.
