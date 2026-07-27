# Test event helpers are copy-pasted into yet another test module

Review target: `src/tests/search_regex.rs` preamble

- src/tests/search_regex.rs:6-16 (`esc`, `enter`, `backspace`)
- src/tests/search_input.rs:14-24, src/tests/search_incremental.rs:6-16 (identical definitions)
- src/tests.rs:28 (`key`, the one helper that *is* shared)

## Summary

`esc`, `enter` and `backspace` are defined identically in `search_input.rs`,
`search_incremental.rs` and now `search_regex.rs`, with `enter` alone additionally duplicated
in `search_execution.rs`, `search_reanchor.rs`, `search_with_header.rs` and `search_wrap.rs`.
This is pre-existing duplication, but the new file is the latest copy, and the trend is that
every new search test module starts by re-typing them.

`src/tests/README.md` even points readers at another module for these ("see
`search_incremental.rs` for extra event helpers"), which is a fair signal that they are meant
to be shared and just never were. `key` already lives in `src/tests.rs` and is imported via
`super::`; moving `esc`, `enter`, `backspace` (and any future `left`/`right`) next to it would
make the import line the only per-module cost and let the README point at one place.

Not urgent — the helpers are three lines each and cannot silently diverge in a harmful way —
but this change is the natural moment to stop the copy, since it adds the fourth full set.

## Assessment

Valid. Confirmed the duplication: `esc`/`enter`/`backspace` are defined identically in
`search_input.rs`, `search_incremental.rs`, `search_regex.rs`; `enter` alone is also
re-typed in `search_execution.rs`, `search_reanchor.rs`, `search_with_header.rs`,
`search_wrap.rs` — seven copies of `enter` across the suite. `key` already lives in
`src/tests.rs` and every module imports it via `super::`, so the pattern for sharing a
helper is already established; these three just never got moved in. The README's pointer
at `search_incremental.rs` "for extra event helpers" confirms they were meant to be
central, not per-module.

The fix is cheap and has no downside: three functions, each already `fn foo() -> Event`
with no per-module variation, moving next to `key` in `src/tests.rs`.

## Plans

### Plan 1: Move `esc`, `enter`, `backspace` into `src/tests.rs` (recommended)

1. Add `pub fn esc() -> Event`, `pub fn enter() -> Event`, `pub fn backspace() -> Event` to
   `src/tests.rs`, next to `pub fn key`.
2. Delete the local `esc`/`enter`/`backspace` definitions from `search_input.rs`,
   `search_incremental.rs`, `search_regex.rs`, `search_execution.rs`, `search_reanchor.rs`,
   `search_with_header.rs`, `search_wrap.rs`.
3. Update each module's `use super::{...}` import to pull in `esc`/`enter`/`backspace` as
   needed (mirroring how `key` is already imported).
4. Update `src/tests/README.md` to point at `src/tests.rs` for all shared event helpers
   instead of `search_incremental.rs`.
5. `cargo test` to confirm nothing regresses (pure refactor, no behavior change expected).

### Plan 2: Leave as-is, but stop the README from pointing to the wrong place

Just fix the README line to stop suggesting `search_incremental.rs` is the canonical
source, without moving the functions. Cheaper, but does nothing about the actual
duplication (still 7 copies of `enter`), and doesn't fix the root complaint — that every
new search test module re-types these. Not recommended as a permanent fix, only if the
consolidation is deemed not worth doing right now.

## Recommendation

Plan 1. The duplicated functions are identical, have no per-module variation, and `key`
already demonstrates the exact pattern to follow — this is a pure mechanical move with no
design risk and it directly fixes the root cause (every new module re-typing the same
three lines) rather than just the symptom (the stale README pointer).
