---
type: bugfix
tags: [heading, renderer]
---

## Overview

**A jump that shrinks the sticky overlay (`header + heading`) is rendered as a partial terminal
scroll, so rows that were hidden under the overlay are dragged into the content region and never
repainted, leaving stale text on screen.**

`Pager::jump_to` returns `PageUpdate::Partial(Some(scroll))` measured purely as a viewport row
shift. `Renderer::render_partial` then scrolls the whole terminal by that amount and redraws only
`scroll` content rows. When the overlay shrank during the same jump, that is too few rows.

- **Reachable via** `g` (jump to top) while a sticky heading is showing. `Pager::cancel_search_input`
  and `Pager::reveal_match`'s upward branch route through `Pager::jump_to` too.
- **Scale of corruption** — one stale row per row of overlay shrinkage. With `--heading-lines 2`
  three rows can be wrong at once.
- **Render-only** — the status line stays correct, so pager state is fine.

`Pager::jump_to_bottom` has the same `resolve` + `JumpDistance` shape and calls
`push_up_heading_if_needed`, which can shrink `heading.full_height()` to a partial height. It is
therefore exposed to the same failure on a downward jump. No concrete reproduction was constructed
for that path, but the fix should cover it rather than special-casing the upward branch.

The scroll keys do not hit this:

- `Pager::scroll_down` uses `Heading::resolve_if_found`, which never unsets the heading.
- `Pager::scroll_up` only calls `Heading::resolve` when `top_line < heading_start`, and at the
  boundary the heading is kept.
- `Pager::jump_to_end` returns `PageUpdate::Full`, so `G` repaints everything.

## Reproduction

`src/tests/heading.rs`, screen 20x8 (viewport 7 rows), `--header 1 --heading '^# '`:

```rust
#[test]
fn jump_to_top_after_heading_became_sticky() {
    let content = "HEADERLINE!\n# A\nsub A\nb1\nb2\nb3\nb4\nb5\nb6\nb7\n";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 8,
        content,
        options: Options {
            header: 1,
            heading: heading_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('g'), key('q')],
        ..Default::default()
    });
    panic!("{}", screen.out());
}
```

After `j` the heading becomes sticky (overlay 2 rows). Pressing `g` jumps back to the top, which
unsets the heading (overlay 1 row) and scrolls by 1:

```
HEADERLINE!
# A
# A            <- stale; "sub A" expected
b1
b2
b3
b4
lines 1-7/10 70%
```

The same setup with `--heading-lines 2` and `j j j G g` loses three rows instead of one:

```
HEADERLINE!
# A
sub A
b1
# A            <- stale
sub A          <- stale
b4             <- stale
```

## Root Cause

A content row's **screen** position equals its **viewport row index**: `Pager::snapshot` builds
`content` as `&self.viewport.rows()[self.total_header_height()..]`, and `render_partial` draws it
at `screen_y = total_header_height() + i`. A terminal scroll of `N` is therefore correct for every
retained row — but only for rows that were actually *painted* before. The top `overlay` viewport
rows are masked by the header/heading, and the terminal scroll drags those masked rows into the
content region.

`src/pager.rs`, `JumpDistance::compute` measures the shift with no knowledge of the overlay:

```rust
match viewport.row_index(self.prev_top.line_index(), self.prev_top.wrap_index()) {
    Some(pos) => PageUpdate::Partial(Scroll::new(Direction::Up, pos)),
    None => PageUpdate::Full,
}
```

`src/renderer.rs`, `render_partial` redraws only that many rows:

```rust
let header_height = page.total_header_height();
let ranges = compute_scroll_redraw_ranges(page.content, scroll.as_ref());
if let Some(scroll) = &scroll {
    self.screen.scroll_terminal(scroll)?;
    let screen_y = header_height + ranges.new_rows.start;
    self.draw_rows(doc, &page.content[ranges.new_rows], page.search, screen_y)?;
}
```

### The exact invariant

For an upward scroll of `N`, the new content row at viewport index `i` was previously at screen
`i - N`, and was actually painted only if `i - N >= prev_overlay`. So the rows needing a repaint are
viewport `[new_overlay, prev_overlay + N)`, while `render_partial` repaints
`[new_overlay, new_overlay + N)`.

**A partial render is correct exactly when `prev_overlay <= new_overlay`.** A *growing* overlay is
harmless: rows are over-redrawn, and rows dragged into the overlay band get overwritten by the
heading draw. Only a *shrinking* overlay corrupts the screen.

The symmetric condition for a downward scroll is `new_overlay + N >= prev_overlay` — i.e. the
correctness condition is direction-dependent. That argues for bailing out whenever the overlay
height changed at all, rather than encoding two directional inequalities.

In the minimal reproduction:

| value          | before `g` | after `g` |
| -------------- | ---------- | --------- |
| overlay height | 2          | 1         |
| scroll rows    | —          | 1         |
| rows redrawn   | —          | 1         |
| rows needed    | —          | 2         |

## Plan

Give `JumpDistance` the overlay height and fall back to `PageUpdate::Full` when it changed.
`JumpDistance` already exists to answer "how much of the screen is reusable across this jump", so
the overlay height belongs there rather than being split between `Pager` and `Renderer`.

```rust
struct JumpDistance {
    prev_top: Row,
    prev_bottom: Row,
    prev_overlay_height: usize,
}

impl JumpDistance {
    fn capture(pager: &Pager) -> Self {
        // Remember the viewport edges before the jump so we can measure the overlap afterwards.
        let rows = pager.viewport.rows();
        Self {
            prev_top: rows[0].clone(),
            prev_bottom: rows[rows.len() - 1].clone(),
            prev_overlay_height: pager.total_header_height(),
        }
    }

    fn compute(self, pager: &Pager) -> PageUpdate {
        // A content row's screen position equals its viewport row index, so a terminal scroll is
        // only valid while the overlay keeps the same height. When it resizes, rows that were
        // masked by the overlay would be dragged into the content region unrepainted.
        if self.prev_overlay_height != pager.total_header_height() {
            return PageUpdate::Full;
        }

        let viewport = &pager.viewport;
        let rows = viewport.rows();
        // ...unchanged...
    }
}
```

Call sites change from `JumpDistance::from(&self.viewport)` / `compute(&self.viewport)` to
`JumpDistance::capture(self)` / `compute(self)` in both `jump_to` and `jump_to_bottom`, so both
paths are covered at once and no caller has to remember the rule.

This was prototyped and verified: the reproduction frame becomes correct (`sub A` on row 3) and the
full suite stays green.

**Trade-off**: a jump that both moves a little *and* changes the overlay now repaints the whole
screen instead of scrolling. That is the same cost `G` already pays (`jump_to_end` always returns
`Full`), and it only happens on the frame where the heading appears or disappears.

### Alternative considered

Keep the partial scroll and widen the redraw range in `render_partial` by the overlay delta
(`scroll + prev_overlay - new_overlay`), with `Renderer` remembering the previous overlay height.
Rejected: it puts a second source of truth for the overlay height inside `Renderer`, which must be
kept in sync across every render path (`render_full_page`, resize, no-scroll partials) or it
silently goes stale — ongoing maintenance cost for one frame of saved repaint.

### Regression tests

Add to `src/tests/heading.rs`, asserting the expected frames:

- `j` then `g` with a sticky heading active.
- The `--heading-lines 2` variant with `j j j G g`.
