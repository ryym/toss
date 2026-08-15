---
type: bugfix
status: todo
opened_at: 2026-08-15T06:24:47Z
tags: [resize]
---

## Overview

Growing the terminal while viewing the end of a document, then scrolling up, corrupts the
display: a stale copy of the status line is left on screen on every scroll-up instead of being
replaced, so it visibly stacks up one row at a time.

- Expected: growing the terminal near the end of the document re-anchors the view to fill the
  newly available rows (pulling more of the document into view from above, the way `less`
  does), and scrolling afterward behaves normally, with the status line staying on a single
  row.
- Actual: the view stays anchored where it was before the resize, leaving blank space below the
  last line instead of filling it. This is not yet visibly broken by itself. The corruption
  appears on the next scroll-up: the status line is redrawn at a fixed row while the terminal
  content shifts underneath it, so the previous frame's status line is never cleared and a new
  one accumulates on top with every keypress.

## Reproduction

`src/tests/resize.rs::repro_grow_at_end_then_scroll_up` reproduces the stacking status lines
with a `MockScreen` script: jump to the end of a 10-line document on a 10x4 screen, resize to
10x8, then press `k` three times. Each `k` leaves one extra `{rev}...{/rev}` status line in the
snapshot instead of replacing the previous one.

Two more cases in the same file encode the expected behavior via `#[should_panic]`, since the
underlying bug affects them as well:

- `resize_at_end_fills_screen_from_above` — growing the screen while at the document's end
  should re-anchor the top row upward to fill the new rows, keeping the last line pinned at the
  bottom.
- `resize_screen_larger_than_document_shows_whole_document` — growing the screen past the
  document's total size should show the whole document with blank padding below it, and further
  scrolling in either direction should then be a no-op.

With `--header`, the same shortfall panics instead of only corrupting the display. Jumping to
the end of a 10-line document on a 10x4 screen with `header: 5`, then resizing to 10x15, leaves
3 viewport rows while the rebuilt header claims 5:

```
thread '...' panicked at src/pager.rs:205:43:
range start index 5 out of range for slice of length 3
```

`PageSnapshot::content` slices `&self.viewport.rows()[self.total_header_height()..]`
(`src/pager.rs:205`), and `Pager::content_height` (`src/pager.rs:219`) would underflow the same
way. Restoring the invariant below fixes this too, since a re-anchored viewport always holds at
least as many rows as the header does.

## Root Cause

`Viewport::resize` (`src/pager/viewport.rs:51-58`) keeps the current top row and fills downward
with `rows::list_forward`:

```rust
pub fn resize(&mut self, doc: &mut Document, size: ViewportSize) {
    let top = self
        .rows
        .first()
        .map_or((0, 0), |row| (row.line_index(), row.wrap_index()));
    self.rows = rows::list_forward(doc, size.width(), top, size.height());
    self.size = size;
}
```

When the top row is close to the end of the document, `list_forward` cannot produce
`size.height()` rows — there simply aren't that many lines left. `self.rows.len()` ends up
smaller than `size.height()`, and nothing ever pulls the top row back up to compensate.

`PageSnapshot::viewport_height()` (`src/pager.rs:86-88`) derives the status line's row from
`content.len()`, i.e. the _actual_ row count, not `size.height()`:

```rust
pub fn viewport_height(&self) -> usize {
    self.total_header_height() + self.content.len()
}
```

So right after a resize, the status line sits directly under the short content and the rest of
the screen is blank — this is the "actual" state described above.

The corruption appears on the next scroll. `Viewport::scroll_up` (`src/pager/viewport.rs:61-83`)
assumes `self.rows.len()` already equals `size.height()`:

```rust
let len = self.rows.len();
self.rows.truncate(len - new_rows.len());
self.rows.splice(0..0, new_rows);
```

It removes exactly as many rows from the tail as it prepends, so `self.rows.len()` (and
therefore `viewport_height()`, the status row's index) never grows back to `size.height()`. But
`Renderer::render_partial` (`src/renderer.rs`) always scrolls the whole terminal by one row via
`scroll_terminal` for a scroll update. The terminal's actual content shifts every row down by
one, while the status row is redrawn at the same fixed (too-small) index every time. The
previous frame's status line, one row up from where it's redrawn, is never cleared. Each
scroll-up leaves one more behind, so the status line visibly stacks up.

## Plan

### The invariant to aim for

Both plans below are different answers to the same question: what is `Viewport::rows().len()`
allowed to be?

The intended invariant is:

> `rows.len() < size.height` only when the whole document is already shown — i.e. the top row
> is the document's first row and the bottom row is its last.

Under that invariant, a short viewport can only occur where scrolling is a no-op in both
directions, so `PageSnapshot::viewport_height()` (the status row's index) can never drift out
of sync with what `Renderer::render_partial` scrolls. Both already behave correctly; restoring
the invariant fixes the rendering without touching the renderer.

Today the invariant is violated in exactly one place: `Viewport::resize` near the document's
end. The streaming path leaves `rows.len() < size.height` too, but there the top is still row 0
and everything known so far is shown, so it satisfies the invariant — verified by probing an
`App` whose first screen is not yet full: `j`, `k`, `f`, and `b` are all no-ops.

### Plan A: Re-anchor upward in `Viewport::resize`

When `list_forward` from the current top yields fewer than `size.height()` rows, pull the
shortfall in from above with `rows::list_backward` and prepend it. The top row moves toward the
document's start only when there is no content left below to fill with, mirroring `less`.

`Pager::relayout_page` must then re-resolve the heading for the new top line, since the top can
now change during a resize (it never could before). `Pager::resize` already returns
`PageUpdate::Full`, so no renderer change is needed.

Backward filling can only run out at row 0, so after this change a short viewport always has
the document's first row at the top — exactly the invariant above. Nothing else needs to
change.

This alone satisfies both `#[should_panic]` cases:

- 10x4 → 10x8 at the end: top is line 8 (index 7), forward yields 3 rows, backward pulls in
  lines 4-7 → lines 4-10 fill the 7-row viewport, the last line stays pinned at the bottom.
- 10x4 → 10x15: backward can only reach line 1, so `rows.len()` stays at 10 (the document's
  total). `scroll_up`/`scroll_down` then find no new rows and return 0, i.e. genuine no-ops.

- Good: fixes the root cause; matches `less`; restores the invariant globally; both expectation
  tests pass with one localized change; no renderer or snapshot changes.
- Bad: `Viewport::resize` is shared with the streaming fill via `Pager::relayout_page`, so
  re-anchoring would also apply there — a partially filled first screen would pull content
  upward as lines arrive, which is not wanted. The two callers need to be split (e.g. `resize`
  vs. a `fill_down` used by `pump_input`), which widens the diff slightly.

### Plan B: Pin the status line to the screen bottom

Change `PageSnapshot::viewport_height()` to return `size.height` instead of
`total_header_height() + content.len()`, so the status row never moves and stale copies cannot
accumulate.

- Good: smallest diff; kills the corruption regardless of why the viewport is short.
- Bad: treats the symptom, not the cause — the view still fails to refill after a resize, so
  `resize_at_end_fills_screen_from_above` stays broken. It also changes an existing deliberate
  rule: for a document shorter than the screen, the status line currently sits directly under
  the content (see `width_change_reflows_wrapped_lines`), and this would push it to the bottom
  instead. That is a user-visible spec change beyond the scope of this bug.

### Recommendation

Plan A: split `Viewport::resize` from the streaming fill, then add the upward re-anchoring.

Plan B is not worth taking: it changes a deliberate display rule and leaves the actual
re-anchoring bug in place.

### Rejected: making `scroll_up`/`scroll_down` self-healing

An earlier draft proposed dropping the `rows.len() == size.height` assumption in
`scroll_up`/`scroll_down` (truncating only the excess beyond `size.height` instead of exactly
as many rows as were prepended), partly to close a suspected `len - new_rows.len()` underflow.

Both motivations turned out to be unfounded:

- The underflow is unreachable. `App::apply_scroll` (`src/app.rs:215`) clamps every scroll by
  `Pager::content_height()`, which is derived from `rows.len()`, so `n_rows <= rows.len()`
  always holds. Probed with `b` and `u` after a grow-at-end resize: no panic.
- Plan A already establishes the invariant everywhere, so there is no state left for the
  scroll methods to heal.

It would also not fix the reported symptom on its own: `Renderer::render_partial` scrolls the
terminal by `scroll.num_rows` while the row count would simultaneously grow, so the rows that
actually move and the rows the page now holds would diverge, and
`compute_scroll_redraw_ranges` would need re-deriving to match.

### Test changes

`repro_grow_at_end_then_scroll_up` and `resize_at_end_fills_screen_from_above` drive the exact
same event sequence and differ only in expected output. Once fixed, the repro case is
redundant: delete it and drop `#[should_panic]` from the other two.
