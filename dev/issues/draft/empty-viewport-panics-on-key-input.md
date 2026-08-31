---
type: bugfix
tags: [pager, resize]
---

## Overview

**Starting a search panics when the page holds no rows, and resizing the terminal to 0 rows
panics before that.**

Two independent ways to reach a page with no rows:

- An empty input (a 0-line file, or `printf '' | toss`).
- A terminal 1 row tall: `ViewportSize::new` reserves the status row, so the viewport height
  becomes 0 even though the document is not empty.

- Expected: neither an empty document nor a degenerate terminal size crashes the pager.
- Actual: `/` and `?` panic with `index out of bounds: the len is 0 but the index is 0`.
  A resize to 0 rows panics earlier still, with `attempt to subtract with overflow`.
  Every other key is fine: the scroll keys are clamped by `Pager::content_height()`, which is
  `0`; `g`, `G` and the jumps compose an empty page without indexing it; and `n` returns early
  when no search is active.

The crash is invisible on the terminal — see
`dev/issues/draft/panic-message-erased-by-alternate-screen.md`. The message is recorded in
`toss-panic.log` in the current directory.

## Reproduction

### Empty document

`printf '' | toss`, then press `/` (or `?`). The pager exits at once with code 101.

```rust
run_test_screen(TestCase {
    screen_width: 10,
    screen_height: 4,
    content: "",
    events: vec![key('/'), key('q')], // also: key('?')
    ..Default::default()
});
```

### 1-row terminal, non-empty document

```rust
run_test_screen(TestCase {
    screen_width: 10,
    screen_height: 1,
    content: "line 1\nline 2\nline 3\n",
    events: vec![key('/'), key('q')], // also: key('?')
    ..Default::default()
});
```

### Resize to 0 rows

```rust
events: vec![resize(10, 0), key('q')],
```

```
thread '...' panicked at src/pager.rs:31:21:
attempt to subtract with overflow
```

## Root Cause

Two unrelated spots, both assuming a size the caller never guarantees.

- `Pager::start_search_input` indexes `self.contiguous_rows()[0]` to pick the line the search
  starts from. `Frame::contiguous_rows` returns an empty vector for an empty page.
- `ViewportSize::new` computes `screen_height - 1` with no floor, so a 1-row terminal yields
  height `0` and a 0-row terminal underflows.

Nothing upstream rules the cases out. `src/lib.rs` states the non-empty assumption but does not
enforce it: `wait_until_exceeds_or_complete(&mut doc, 0)` blocks until the first line arrives
*or the input ends*, so it only covers the streaming case; an already-complete 0-line source
passes straight through.

## Plan

Enforce both conditions at the boundary rather than guarding the call site.

### 1. `NonEmptyDocument` newtype

```rust
impl Document {
    fn into_non_empty(self) -> Result<NonEmptyDocument, Document>;
}

Pager::new(doc: NonEmptyDocument, ...)
```

A `Document` only ever gains lines (`Document::pump` pushes onto `lines`; a `File` source has a
fixed `LineIndex`), so the property is monotone: checking once at construction keeps it valid
for the wrapper's whole lifetime. Implement `Deref<Target = Document>` so the pager internals
need no changes.

Construct it in `run_inner` right after `wait_until_exceeds_or_complete(&mut doc, 0)`, which
becomes the single place where emptiness is handled. An empty input exits without starting the
pager (exit code 0, no output), mirroring the `-F` short-circuit — a deliberate behavior change:
`less` shows an empty page instead.

`NonEmptyDocument` must not expose any line-removing operation; add a test that locks the
invariant so a future `Document` API cannot silently break it.

### 2. Minimum terminal size

A usable page needs 1 content row plus the status row, so the minimum screen height is 2.

- Startup: fail with an `AppError` (e.g. `terminal too small: at least 2 rows required`) instead
  of building a `Pager`.
- Resize: suspend rendering instead of resizing. A resize below the minimum is not applied — the
  page keeps its last valid size — and the renderer writes nothing at all while suspended. The
  next resize back to a valid size recomposes and repaints.

  Nothing legible fits in fewer than 2 rows, so there is no display to guarantee during that
  window; the point is only that the session survives. Quitting instead would discard the
  document, which is unrecoverable for a piped stdin (`cmd | toss`), and would kill the pager on
  an incidental window drag or pane split. The asymmetry with the startup error is deliberate:
  at startup nothing has been read yet and a clear error can actually be displayed.

  Without the suspend flag the page would still not crash — writes past the last row are clamped
  by the terminal — but every row would be overwritten onto the single visible line for no
  benefit.
- Make `ViewportSize::height` a `NonZeroUsize` so the guarantee is visible in the signature.
  This also removes the `screen_height - 1` underflow.
- `ScrollPhysics::configure` (`src/scroll.rs`) divides `REFERENCE_HEIGHT` by the terminal
  height and yields infinite friction/drag at 0; the same minimum covers it.

Together these make a non-empty page an invariant of `Pager`, so `start_search_input` can keep
indexing directly.

### Tests

- e2e: empty input exits without paging; a 1-row terminal fails to start; resizing to 0 or 1
  rows mid-session writes nothing and resizing back repaints the whole page.
- unit: `Document::into_non_empty` for empty/non-empty and streaming sources.

This supersedes the separate `resize-to-zero-height-panics` draft, whose content is folded in
above.
