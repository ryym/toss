---
type: bugfix
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

Not yet decided. `Viewport::resize` needs to re-anchor upward — shifting the top row toward the
document's start — whenever `list_forward` from the current top returns fewer rows than
`size.height()` and there are lines above the top to pull in. `Viewport::scroll_up` and
`scroll_down` also need to stop assuming `self.rows.len() == self.size.height`, since that
invariant does not hold right after such a resize.
