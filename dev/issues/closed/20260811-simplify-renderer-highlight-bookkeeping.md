---
type: maintenance
status: cancelled
opened_at: 2026-08-01T07:21:37Z
tags: [renderer]
---

## Reason for Cancel

The plan below does not actually simplify the renderer, and it lengthens the lifetime of the
state it touches. Cancelled in favour of keeping the two-set scheme.

- **The implicit invariant is not removed, only relocated.** The plan's own wording is that
  "every path that draws a line without a highlight must remove it explicitly". That is the
  same class of unenforced obligation as the carry-forward it deletes, and it is carried by
  more code: today only a row-*skipping* path owes anything (one site, in `render_partial`),
  whereas afterwards every row-*drawing* path does (`draw_rows` and `refresh_rows`, steps 2
  and 3).
- **It is a net addition of code and concepts.** Removed: a `mem::take` and a five-line loop.
  Added: a removal branch in `draw_rows`, a rewrite of `refresh_rows`, and a whole pruning
  mechanism — a threshold constant, a widened `store_page_state` signature, and a visible-line
  set. Memory growth is a concern that does not exist under the current scheme; the plan
  introduces it and then spends code containing it.
- **The state gets weaker semantics and a longer life.** `current_highlight_lines` today means
  "highlights drawn this frame" and nothing else, so its correctness is checkable by reading
  `draw_rows`/`refresh_rows`. A persistent set's value at any moment depends on the whole
  history of renders, and its intended meaning ("lines currently highlighted on screen") only
  holds if pruning runs. Trading a frame-scoped fact for a history-dependent one is the wrong
  direction, even where the two-set version costs a few more lines.
- **The problem being solved is hypothetical.** The Overview states it is not a live bug; it
  guards future row-skipping paths only. That is not worth the above.

What was salvaged instead:

- The scroll-route regression test from the plan was added as
  `search_incremental::highlights_clear_on_rows_untouched_by_a_previous_scroll`, closing the
  coverage gap the Overview identified. It is verified to fail if `render_partial`'s
  carry-forward loop is removed.
- The invariant is now documented on the field declarations in `src/renderer.rs`, since it
  cannot be expressed in the types.

## Overview

`Renderer` (`src/renderer.rs`) tracks on-screen highlights with two `HashSet<usize>` fields:
`current_highlight_lines`, rebuilt from scratch each render by `draw_rows`/`refresh_rows`, and
`last_highlight_lines`, which `store_page_state` swaps it into afterward. This relies on an
unenforced invariant: every render must fully repopulate `current_highlight_lines` for every
highlighted row.

Commit 20eef62 already had to patch around a row-skipping path in `render_partial` by adding a
manual loop that replays entries from `last_highlight_lines` for rows it chose not to redraw.
That fix is correct, but it's a symptom fix — any future row-skipping path must remember the
same replay, and nothing in the types signals that obligation.

Test coverage is partial. Reverting the replay loop makes exactly one existing test fail,
`search_regex::invalid_regex_after_valid_preserves_last_match` (added in d52be38), which covers
the route the fix was written for: an invalid regex freezes the preview via a `StatusOnly`
render, and recovering to a query equal to the last valid one hits the skip path. The other
route into the same skip path — an ordinary scroll while the search state is unchanged — has no
test, even though it was equally broken before 20eef62 (verified by replaying the scenario
below against 95072b9, the pre-regex parent commit).

Not a live bug: every row-skipping path today already replays correctly. This is about removing
future fragility, so it's safe to defer.

## Outcome

- On-screen highlight tracking no longer depends on the implicit invariant that every render
  fully repopulates the set of highlighted rows. Adding a new row-skipping render path can no
  longer introduce the "forgot the carry-forward" failure mode, because that mode ceases to
  exist.
- Both routes into the skip path are covered by tests that assert on screen output: the
  invalid-regex route by the existing test named above, and the scroll route by a new one.
- The redraw-skipping that suppresses flicker keeps working under the same conditions as today.
- The renderer's highlight state stays bounded by a constant rather than growing with the
  document, and the per-render cost of keeping it bounded does not scale with the viewport.

## Plan

Replace the two-set swap scheme with one persistent `highlighted_lines: HashSet<usize>`,
updated incrementally: `insert` when a line is drawn with a highlight, `remove` when drawn
without one. Rows that aren't drawn are simply left untouched — already correct, so no
carry-forward is needed. This deletes the swap, the second set, and the carry-forward loop.

The two-set scheme removes entries implicitly: `current_highlight_lines` starts empty every
frame, so "did not insert" means "not highlighted" next frame. A persistent set has no such
reset, so **every path that draws a line without a highlight must remove it explicitly**. Miss
one and that entry stays set forever, making the line pay a wasted redraw on every later
refresh. Both such paths are called out below (steps 2 and 3).

In `src/renderer.rs`:

1. Merge `last_highlight_lines`/`current_highlight_lines` into one persistent
   `highlighted_lines: HashSet<usize>`.
2. `draw_rows`: add the `remove(&line_idx)` branch for the no-highlight case. `draw_rows`
   currently only ever `insert`s, since its set always started empty.
3. `refresh_rows`: point its insert at the single set, and make its `Cow::Borrowed` branch
   remove instead of only testing. `HashSet::remove` returns whether the entry was present,
   so the test and the removal collapse into one call:

   ```rust
   Cow::Borrowed(text) => {
       // Skip redraw only if the line has no highlights both before and this time.
       if !self.highlighted_lines.remove(&line_idx) {
           continue;
       }
       self.clear_row_range(screen_y, start..i)?;
       self.screen.write_at(start + screen_y, text)?;
   }
   ```

4. Delete `render_partial`'s carry-forward loop (from commit 20eef62) and `store_page_state`'s
   `mem::take` swap — `store_page_state` keeps only its `last_search` update and its
   `StatusOnly` early return (that early return is unrelated to this refactor — it's what
   makes the invalid-regex search freeze work, see `update_search_query` in `src/pager.rs`).
5. Bound the set by dropping off-screen entries once it grows past a threshold (see below).
   This needs `store_page_state` to take the whole `PageSnapshot` instead of just `search`.

### Bounding the set

A line that scrolls off screen while highlighted is never drawn again, so nothing removes its
entry. Left unbounded the set grows with the number of distinct lines ever drawn with a
highlight, which for a long session over a large file approaches the document's line count.
That is not negligible next to what a document actually keeps resident: a file-backed
`Document` holds a `Vec<u64>` line index (8 bytes per line, `src/document/line_index.rs`) plus
a 1000-line LRU cache — it does not hold every line. At roughly 10-18 bytes per entry for a
`HashSet<usize>`, an unbounded set can exceed the line index itself.

Dropping off-screen entries is always safe: a line re-entering the viewport is redrawn by
`draw_rows`, which rewrites its entry before anything reads it. Entries for lines currently on
screen must be kept — dropping one would recreate a stale false. Deriving the survivors from
the snapshot satisfies that by construction, so unlike the carry-forward it replaces, there is
no obligation for future code to remember.

Pruning may run at any time, so gate it on a threshold rather than running it every render:

```rust
/// Must stay comfortably above any plausible viewport line count, so that
/// pruning cannot end up firing on every render.
const HIGHLIGHT_PRUNE_THRESHOLD: usize = 4096;

// in store_page_state, after the StatusOnly early return
if self.highlighted_lines.len() > HIGHLIGHT_PRUNE_THRESHOLD {
    let visible: HashSet<usize> = page.header.iter()
        .chain(page.heading)
        .chain(page.content)
        .map(|row| row.line_index())
        .collect();
    self.highlighted_lines.retain(|line_idx| visible.contains(line_idx));
}
```

The common path costs one integer comparison — no allocation, no `retain`. A prune shrinks the
set to the viewport, so the next one cannot happen until thousands more lines have been drawn
with a highlight, which amortizes to nothing. Memory is capped at a constant regardless of file
size, and the set keeps meaning exactly "lines currently highlighted on screen".

### Regression test for the scroll route

Add to `src/tests/search_incremental.rs`. The `j` scroll takes the "search unchanged, remaining
rows not redrawn" path, and the following `/z` triggers `refresh_rows`. With the tracking
broken, only row `foo 5` (redrawn by the scroll itself) loses its highlight and the other three
keep a stale one.

This test was written and verified during refinement: it passes on the current code and fails
if the carry-forward loop is removed without the rest of the refactor. Use it as-is; only the
expected output should need updating, and only if unrelated behavior changes.

```rust
// Highlights on rows that a scroll leaves untouched are still cleared once the
// search query changes.
#[test]
fn highlights_clear_on_rows_untouched_by_a_previous_scroll() {
    let content = "\
foo 1
foo 2
foo 3
foo 4
foo 5
foo 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),
            key('j'),
            key('/'),
            key('z'),
            enter(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
foo 1
foo 2
foo 3
foo 4
{rev}lines 1-4/6 66%{/rev}
-----
[EVENT]:char:/
foo 1
foo 2
foo 3
foo 4
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo 1
{rev}{line}{b}f{/rev}{/line}{/b}oo 2
{rev}{line}{b}f{/rev}{/line}{/b}oo 3
{rev}{line}{b}f{/rev}{/line}{/b}oo 4
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 2
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 4
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 4
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 4
{rev}lines 1-4/6 66%{/rev}
-----
[EVENT]:char:j
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 4
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 5
{rev}lines 2-5/6 83%{/rev}
-----
[EVENT]:char:/
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 4
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 5
/█
-----
[EVENT]:char:z
foo 2
foo 3
foo 4
foo 5
/z█
-----
[EVENT]:enter
foo 2
foo 3
foo 4
foo 5
{rev}lines 2-5/6 83%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
```

### Two kinds of staleness

The set answers "does line L currently show a highlight on screen?", and the two ways its
answer can disagree with the screen are not symmetric. The terms are used throughout below.

- **stale true** — the set contains L, but L shows no highlight. Read only by `refresh_rows`'s
  `Cow::Borrowed` branch, where it causes a redraw that rewrites identical text. Costs work,
  never wrong on screen. The `remove` in that branch (step 3) clears the entry as it reads it,
  so each stale entry costs at most one wasted redraw rather than one per refresh.
- **stale false** — the set lacks L, but L shows a highlight. `refresh_rows` then skips the
  redraw and the old highlight stays on screen. This is the bug 20eef62 fixed.

A persistent set cannot produce stale false: entries are dropped only when a line is drawn
without a highlight — the same moment the highlight leaves the screen — or by pruning, which
never touches on-screen lines. That asymmetry is what makes the trade-off below acceptable.

### Accepted trade-off

- **The `draw_rows` removal branch (step 2) is not covered by a test.** Forgetting it leaves a
  stale true entry, whose only effect is that `refresh_rows` redraws a row it could have
  skipped — the text written is identical, the write happens inside `begin_sync`/`end_sync`, and
  step 3's `remove` corrects the entry on that first read, so the cost does not recur. It is a
  pure optimization, not a correctness concern.

  Such a test is writable: a fake `Screen` counting `write_at`/`clear_row` calls detects whether
  a row was redrawn, and the `Row` values a `PageSnapshot` needs can be obtained either by
  opening `Row::new` behind `#[cfg(test)]` (as `MatchPosition::new` already is) or by having a
  real `Pager` build the snapshot. It is skipped because of what it would have to assert on: a
  redraw count is an implementation detail of the skip heuristics, so the test would break on
  any legitimate change to them, while what it guards has no observable effect when it breaks.
