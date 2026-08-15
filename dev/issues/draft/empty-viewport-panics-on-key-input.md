---
type: bugfix
tags: [pager, resize]
---

## Overview

**`Pager` assumes the viewport always holds at least one row, and panics on key input when it
does not.**

Two independent ways to reach an empty viewport:

- An empty input (a 0-line file, or `printf '' | toss`).
- A terminal 1 row tall: `ViewportSize::new` reserves the status row, so the viewport height
  becomes 0 even though the document is not empty.

A third, related case panics even earlier: a resize to 0 rows underflows the same subtraction.

- Expected: neither an empty document nor a degenerate terminal size crashes the pager.
- Actual: `g`, `G`, `/` and `?` panic with `index out of bounds: the len is 0 but the index is
  0`. `j`, `k`, `f`, `b`, `d`, `u`, `n` and `q` are unaffected: the scroll keys are clamped by
  `Pager::content_height()`, which is 0, and `n` returns early when no search is active.

The crash is invisible on the terminal — see
`dev/issues/draft/panic-message-erased-by-alternate-screen.md`. The message is recorded in
`toss-panic.log` in the current directory.

## Reproduction

### Empty document

`printf '' | toss`, then press `G` (or `g`, `/`, `?`). The pager exits at once with code 101.

```rust
run_test_screen(TestCase {
    screen_width: 10,
    screen_height: 4,
    content: "",
    events: vec![key('G'), key('q')], // also: key('g'), key('/'), key('?')
    ..Default::default()
});
```

### 1-row terminal, non-empty document

```rust
run_test_screen(TestCase {
    screen_width: 10,
    screen_height: 1,
    content: "line 1\nline 2\nline 3\n",
    events: vec![key('G'), key('q')], // also: key('g'), key('/'), key('?')
    ..Default::default()
});
```

Both cases panic at the same three places:

| Key      | Location                                         | Message                                                |
| -------- | ------------------------------------------------ | ------------------------------------------------------ |
| `G`      | `src/pager.rs:350` (`Pager::jump_to_end`)        | `index out of bounds: the len is 0 but the index is 0` |
| `g`      | `src/pager.rs:684` (`JumpDistance::from`)        | same                                                   |
| `/`, `?` | `src/pager.rs:491` (`Pager::start_search_input`) | same                                                   |

### Resize to 0 rows

```rust
events: vec![resize(10, 0), key('q')],
```

```
thread '...' panicked at src/pager.rs:31:21:
attempt to subtract with overflow
```

## Root Cause

`Viewport` handles an empty row list (see
`viewport.rs::resize_does_not_panic_when_rows_are_empty`), but `Pager` assumes at least one row:

- `src/pager.rs:350` — `let top_line_index = self.viewport.rows()[0].line_index();`
- `src/pager.rs:684` — `prev_top: rows[0].clone()` in `JumpDistance::from`
- `src/pager.rs:491` — `self.contiguous_rows()[0].line_index()`, which also indexes `rows[0]`
  inside `Pager::contiguous_top_row_index` (`src/pager.rs:236`)
- `src/pager/viewport.rs:127` — `DocPos::Before(&rows_after_line[0])` in `Viewport::jump_to`,
  reached only once the panics above are fixed

Nothing upstream guarantees the assumption:

- `src/lib.rs:163-165` states it but does not enforce it. `wait_until_exceeds_or_complete(&mut
  doc, 0)` blocks until the first line arrives *or the input ends*, so it only covers the
  streaming case; an already-complete 0-line source passes straight through.
- `ViewportSize::new` (`src/pager.rs:27-33`) computes `screen_height - 1` with no floor, so a
  1-row terminal yields height 0 and a 0-row terminal underflows.

## Plan

Enforce the invariant at the boundary rather than guarding each call site, and encode it in the
types so it does not have to be re-argued at every review.

> INVARIANT: `Viewport::rows()` is never empty.

It holds exactly when the document is non-empty **and** the viewport height is at least 1.

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
  next resize back to a valid size relayouts and repaints with `PageUpdate::Full`.

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
- `ScrollPhysics::configure` (`src/scroll.rs:48-52`) divides `REFERENCE_HEIGHT` by the terminal
  height and yields infinite friction/drag at 0; the same minimum covers it.

### 3. Remove the raw indexing

Add `Viewport::first_row(&self) -> &Row` and `last_row(&self) -> &Row` returning `&Row` rather
than `Option<&Row>`, and use them at the sites listed under Root Cause. A total accessor states
the invariant at every call site, so the "what if it's empty?" question stops coming up. Back it
with a module-level `INVARIANT` comment (same style as
`Renderer::current_highlight_lines`) and `debug_assert!`s in the `Viewport` constructors.

### Tests

- e2e: empty input exits without paging; a 1-row terminal fails to start; resizing to 0 or 1
  rows mid-session writes nothing and resizing back repaints the whole page.
- unit: `Document::into_non_empty` for empty/non-empty and streaming sources; `Viewport`
  invariant after `new`/`resize`/`jump_to`/`jump_to_end`.

This supersedes the separate `resize-to-zero-height-panics` draft, whose content is folded in
above.
