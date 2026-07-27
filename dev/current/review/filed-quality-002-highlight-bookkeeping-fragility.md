# Highlight bookkeeping is rebuilt per frame, so every skipped draw path must remember to carry it forward

Review target: the renderer fix that makes invalid-regex freezing safe

- src/renderer.rs:150-159 (new carry-forward loop in `render_partial`)
- src/renderer.rs:85-95 (`store_page_state` swaps `current_highlight_lines` into `last_highlight_lines`)
- src/pager.rs:524-527 (`update_search_query` returns `StatusOnly` for invalid input)

## Summary

`Renderer` tracks which lines are highlighted on screen with two sets: `current_highlight_lines`
is built up from scratch during a render, and `store_page_state` then swaps it into
`last_highlight_lines`. The invariant this relies on is "every render fully repopulates
`current_highlight_lines` for all rows that are highlighted on screen". That invariant is not
enforced anywhere — it just happens to hold as long as every row is passed through `draw_rows`
or `refresh_rows`, both of which do the `insert`.

The new freeze path breaks it, and commit 20eef62 patches it by adding a third place that
writes to `current_highlight_lines`: a manual loop that copies entries out of
`last_highlight_lines` for rows `render_partial` decided not to redraw. The patch is correct
for the case it addresses, but it treats the symptom. The same class of bug reappears for any
future code path that skips drawing some rows, and each such path must independently remember
to replay the bookkeeping. Nothing about the field names or types signals that obligation.

A representation that cannot go stale would remove the class entirely: keep one persistent
`highlighted_lines: HashSet<usize>`, `insert` when a line is drawn *with* a highlight and
`remove` when a line is drawn *without* one. Rows that are not drawn are then simply not
touched, which is exactly the intended semantics ("what is on screen right now"), and the
carry-forward loop, the second set, and the `mem::take` swap all disappear. `refresh_rows`'s
`Cow::Borrowed` branch already reads as "was it highlighted before?", which is the same
question this single set answers directly.

Two further points worth deciding on:

- `store_page_state` skipping bookkeeping for `StatusOnly` is now load-bearing for the freeze
  behaviour: `update_search_query` returning `StatusOnly` on an invalid regex is what keeps
  `last_search` / `last_highlight_lines` pointing at the frozen frame. That is an implicit
  dependency between `Pager`'s choice of `PageUpdate` variant and a private renderer
  optimisation. It works, but it means "which `PageUpdate` do I return?" silently doubles as
  "should the renderer forget what is on screen?". Worth at least documenting on `PageUpdate`,
  since nothing at either end mentions the other.
- The carry-forward loop covers `ranges.remaining` only. That is complete today because
  `new_rows`, `header` and `heading` all go through `draw_rows` on this path — but that is a
  non-obvious argument a reader has to reconstruct, and it is a second reason to prefer the
  persistent-set model over enumerating the rows that need replaying.

## Assessment

Valid, and worth fixing now rather than deferring. Confirmed by reading `renderer.rs`:
`current_highlight_lines` is reset to empty every render via the `mem::take` in
`store_page_state` (renderer.rs:94), so the whole scheme depends on every code path that
skips a row remembering to replay its highlight state into the fresh set. The carry-forward
loop added in 20eef62 is exactly one more instance of that obligation, not a fix to the root
cause. There is also no unit test in `renderer.rs` covering this bookkeeping at all, so a
future path that forgets the replay would fail silently (stale highlights lingering, or
highlights vanishing) rather than failing a test — which raises the cost of leaving the
invariant implicit.

The suggested persistent-set model is sound and cheap: everything touched
(`current_highlight_lines`/`last_highlight_lines`, `store_page_state`, `draw_rows`,
`refresh_rows`, `render_partial`) is private to `Renderer`, so this is a self-contained,
mechanical refactor with no ripple into `pager.rs` or the public API. One gap in the reviewer's
sketch: `draw_rows` currently only ever `insert`s (it never had to remove, because it always
starts from an empty set). Under a persistent single set, `draw_rows` must also `remove` on
the "line has no highlight" branch — same shape as `refresh_rows` already does — otherwise a
line that was highlighted and gets redrawn without a highlight via `draw_rows` (e.g. a full
page render, or new rows scrolled into view) would leave a stale entry behind forever.

The second point (the `StatusOnly` / freeze implicit dependency) is a separate, smaller
concern. It's real — the freeze behavior for invalid regex relies on `store_page_state`
skipping the swap for `StatusOnly` — but it isn't made worse or better by the highlight
bookkeeping refactor, and is cheap to address with a doc comment.

## Plans

### Plan 1: Replace the two-set carry-forward scheme with one persistent `highlighted_lines` set (Recommended)

In `src/renderer.rs`:

- Replace `last_highlight_lines` and `current_highlight_lines` with a single
  `highlighted_lines: HashSet<usize>` field that persists across renders and is never reset
  wholesale.
- In `draw_rows`, mirror the two-armed match already used in `refresh_rows`: when
  `apply_highlight_if_matches` returns `Cow::Owned`, `insert(line_idx)`; when it returns
  `Cow::Borrowed`, `remove(&line_idx)`. (Today `draw_rows` only inserts; it needs to remove too
  once the set is no longer implicitly cleared each frame.)
- In `refresh_rows`, replace `self.current_highlight_lines.insert(...)` /
  `self.last_highlight_lines.contains(...)` with `insert`/`remove`/`contains` on the single
  `highlighted_lines` set. The "skip redraw" condition becomes "no highlight now and
  `!highlighted_lines.contains(&line_idx)`".
- Delete the carry-forward loop in `render_partial` (renderer.rs:150-159) entirely — rows that
  are not drawn are simply not touched, which is already correct.
- Delete the `mem::take` swap in `store_page_state` (renderer.rs:94); `store_page_state` keeps
  its existing `last_search` bookkeeping and the early return for `StatusOnly` (still needed —
  see Plan 2), but no longer needs to touch highlight state at all.
- Add or update a doc comment on the field explaining the invariant it now upholds directly:
  "reflects exactly the rows currently highlighted on screen; updated in lockstep with every
  draw, untouched rows are implicitly still correct."

This removes the two-set/swap/carry-forward machinery and the class of bug the review
describes, since there is no longer a "did every path remember to repopulate the fresh set"
question to get wrong.

### Plan 2: Leave the bookkeeping as-is, but document the implicit `PageUpdate` / freeze dependency

Independent of Plan 1 (or in addition to it): add a short doc comment on
`PageUpdate::StatusOnly` in `pager.rs` noting that returning it also suppresses the renderer's
highlight/search-state bookkeeping update, which is what makes the invalid-regex freeze work —
so any future caller returning `StatusOnly` for a new reason should be aware it has this
side effect on the renderer, not just "skip redrawing the page".

This doesn't address the fragility of the highlight bookkeeping itself, only the second,
smaller point in the review about the implicit cross-module dependency.

## Recommendation

Plan 1. It directly removes the bug class the review identifies rather than documenting around
it, the change is fully contained inside `renderer.rs`, and the resulting model is simpler than
what it replaces (one field instead of two, no swap, no carry-forward loop) — a rare case where
the more correct design is also the less code. Pair it with Plan 2's doc comment, since that
addresses a distinct, still-valid observation that Plan 1 does not resolve.
