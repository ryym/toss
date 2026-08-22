# Partial Jump Render Leaves Stale Rows When the Sticky Heading Disappears

Review target: 7f909cc..316c110 (found while reviewing the heading/header boundary; the defect
itself is pre-existing, not introduced by this range)

## Summary

**Pressing `g` (jump to top) while a sticky heading is showing corrupts the screen: rows that
were hidden under the heading overlay are scrolled into view and never redrawn.**

- `Pager::jump_to` returns `PageUpdate::Partial(Some(scroll))` measured purely as a viewport row
  shift, ignoring that the sticky overlay (`header + heading`) may shrink during the same jump.
- `Renderer::render_partial` then scrolls the whole terminal by that amount and redraws only
  `scroll` content rows, so the rows previously masked by the heading stay on screen as garbage.
- The status line is correct, so the pager state is fine — this is a render-only defect.

### Reproduction

Add to `src/tests/heading.rs` (screen 20x8, viewport 7 rows):

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

Frame after `g`:

```
HEADERLINE!
# A
# A            <- stale; should be "sub A"
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

A content row's **screen** position equals its **viewport row index**, because
`snapshot.content` is `viewport.rows()[overlay..]` and `render_partial` draws it at
`screen_y = overlay + i`. A terminal scroll of `N` is therefore correct for every retained row —
but only for rows that were *visible* before. The top `overlay` viewport rows are masked by the
header/heading, and the terminal scroll drags those masked rows down into the content region.

`src/pager.rs`, `JumpDistance::compute` measures the shift with no knowledge of the overlay:

```rust
match viewport.row_index(self.prev_top.line_index(), self.prev_top.wrap_index()) {
    Some(pos) => PageUpdate::Partial(Scroll::new(Direction::Up, pos)),
    None => PageUpdate::Full,
}
```

`src/renderer.rs:129-146`, `render_partial` redraws only that many rows:

```rust
let header_height = page.total_header_height();
let ranges = compute_scroll_redraw_ranges(page.content, scroll.as_ref());
if let Some(scroll) = &scroll {
    self.screen.scroll_terminal(scroll)?;
    let screen_y = header_height + ranges.new_rows.start;
    self.draw_rows(doc, &page.content[ranges.new_rows], page.search, screen_y)?;
}
```

For an upward scroll the number of content rows that actually need redrawing is
`scroll + prev_overlay - new_overlay`, not `scroll`. In the minimal repro:

| value          | before `g` | after `g` |
| -------------- | ---------- | --------- |
| overlay height | 2          | 1         |
| scroll rows    | —          | 1         |
| rows redrawn   | —          | 1         |
| rows needed    | —          | 2         |

### Why the scroll keys do not hit it

- `scroll_down` uses `Heading::resolve_if_found`, which never unsets the heading.
- `scroll_up` only calls `Heading::resolve` when `top_line < heading_start`, and at the boundary
  the heading is kept.
- `jump_to_end` returns `PageUpdate::Full`, so `G` repaints everything.

`Pager::jump_to` is the reachable path: `g`, `Pager::cancel_search_input`, and
`Pager::reveal_match`'s upward branch all route through it, and its
`heading.resolve(&mut self.doc, line_index)` can unset the heading. `jump_to_bottom` has the same
`resolve` + `JumpDistance` shape and is exposed to the same failure when the overlay shrinks.

## Candidate Fixes

- **Fall back to `PageUpdate::Full` when the overlay height changed across the jump.** Smallest
  change; `jump_to` already knows both values.
- **Widen the redraw range by the overlay delta** in `render_partial`, so a partial scroll stays
  possible. Needs the renderer to remember the previous overlay height.
- **Have `JumpDistance` capture the overlay height too**, so the "how much of the screen is
  reusable" decision lives in one place rather than being split between `Pager` and `Renderer`.

Whichever is chosen, add an e2e regression test in `src/tests/heading.rs` for `j` then `g` with a
sticky heading active, and one for the multi-line-heading variant above.

## Assessment

- Newly introduced issue? **No** — `git log 7f909cc..316c110 -- src/pager.rs src/renderer.rs` is empty;
  neither `JumpDistance` nor `render_partial` was touched by the reviewed range.
- Does it block the overall goal? **No** — the reviewed range is about the heading/header boundary,
  and this defect is independent of it.

### The report is accurate

I reproduced it verbatim with the reviewer's test (screen 20x8, `header: 1`, `^# `, `j` then `g`):

```
[EVENT]:char:g
HEADERLINE!
# A
# A            <- stale; "sub A" expected
b1
b2
b3
b4
{rev}lines 1-7/10 70%{/rev}
```

The status line (`lines 1-7/10`) is right, so pager state is fine — this is render-only, as reported.

### The precise invariant

A content row's screen y equals its **viewport row index** (`content = viewport.rows()[overlay..]`
drawn at `overlay + i`). For an upward scroll of `N`, the new content row at viewport index `i` was
previously at screen `i - N`, and was actually painted only if `i - N >= prev_overlay`. So the rows
that need repainting are viewport `[new_overlay, prev_overlay + N)`, while `render_partial` repaints
`[new_overlay, new_overlay + N)`.

**The partial render is correct exactly when `prev_overlay <= new_overlay`.** A *growing* overlay is
harmless (rows are over-redrawn, and rows dragged into the overlay band get overwritten by the
heading draw). Only a *shrinking* overlay corrupts the screen. The symmetric condition for a
downward scroll is `new_overlay + N >= prev_overlay`, i.e. it is direction-dependent — which argues
for treating "the overlay height changed at all" as the bail-out condition rather than encoding two
directional inequalities.

### Reachability

- `jump_to` (upward): confirmed reachable via `g`, `Pager::cancel_search_input`, and
  `Pager::reveal_match`'s upward branch, because `heading.resolve` can unset the heading.
- `jump_to_bottom` (downward): also at risk. It calls `push_up_heading_if_needed`, which can shrink
  `heading.full_height()` to a partial height, so `prev_overlay > new_overlay` is possible there too.
  I did not construct a concrete repro for it, but any fix should be direction-agnostic rather than
  special-casing the upward branch.

### Scope

The defect predates the reviewed range and does not block its goal, so it belongs in a separate
issue rather than in this branch. The fix candidates below are written to be handed to that issue.

## Plans

### Plan 1 (recommended): File an issue; do not fix in this branch

The defect is pre-existing and independent of the heading/header boundary work under review, so
folding it in would mix two concerns in one branch. File it as its own issue, carrying over the
reproduction, the invariant, and the two fix candidates below, so whoever picks it up does not have
to re-derive any of it.

The issue should record:

- The reproduction (the reviewer's test, confirmed to fail as reported).
- The invariant: a partial render is correct exactly when `prev_overlay <= new_overlay`; only a
  shrinking overlay corrupts the screen, and the condition is direction-dependent.
- The two implementation options (Plan 2 and Plan 3), with Plan 2 as the suggested one — it is
  already verified to work.
- The regression tests to add in `src/tests/heading.rs`: `j` then `g` with a sticky heading, and the
  `--heading-lines 2` variant with `j j j G g`, asserting the expected frames rather than `panic!`.

### Plan 2: Let `JumpDistance` carry the overlay height and bail out to `Full`

`JumpDistance` already exists precisely to answer "how much of the screen is reusable across this
jump". The overlay height is part of that question, so it belongs there rather than being split
between `Pager` and `Renderer`.

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
        // A content row's screen position equals its viewport row index, so a terminal scroll
        // is only valid while the overlay keeps the same height. When it resizes, the rows that
        // were masked by the overlay would be dragged into the content region unrepainted.
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

**Verified**: I prototyped exactly this. The repro frame becomes correct (`sub A` on row 3) and the
full suite stays green (297 passed, only the deliberate `panic!` repro failing).

Trade-off: a jump that both moves a little *and* changes the overlay now repaints the whole screen
instead of scrolling. That is the same cost `G` already pays (`jump_to_end` always returns `Full`),
and it only happens on the frame where the heading appears/disappears, so it is not a visible
regression.

### Plan 3: Widen the redraw range in `render_partial` by the overlay delta

Keep the partial scroll and repaint `scroll + prev_overlay - new_overlay` rows:

```rust
// Renderer gains state:
last_overlay_height: usize,
```

```rust
let header_height = page.total_header_height();
let extra = self.last_overlay_height.saturating_sub(header_height);
let ranges = compute_scroll_redraw_ranges(page.content, scroll.as_ref(), extra);
```

with `scroll_dirty_range`'s `Direction::Up` arm using `num_rows + extra` (and the `Down` arm needing
its own, different adjustment).

Downside: it puts a second source of truth for overlay height inside `Renderer`, which must be kept
in sync across every render path (`render_full_page`, resize, no-scroll partials) or it silently
goes stale — exactly the kind of ongoing maintenance cost worth avoiding for a rare frame.

## Recommendation

**Plan 1 — file an issue, and fix it there with Plan 2.**

The defect is real and confirmed, but it predates the reviewed range and is independent of its goal,
so it does not belong in this branch. Keeping the branch focused matters more than the small
convenience of fixing it here.

When the issue is picked up, **Plan 2** is the implementation to take: it is the smallest change,
keeps the "is this jump renderable as a scroll?" decision in the single place that already owns it,
covers `jump_to` and `jump_to_bottom` together, and is direction-agnostic so it does not need
revisiting if `push_up_heading_if_needed` starts shrinking the overlay on downward jumps. Plan 3's
extra renderer state is not worth the one frame of repaint it saves.

## Filed as Issue

`dev/issues/draft/partial-jump-render-leaves-stale-rows-when-overlay-shrinks.md`
