---
type: maintenance
status: todo
opened_at: 2026-08-01T07:21:37Z
tags: [renderer]
---

## Overview

`Renderer` (`src/renderer.rs`) tracks on-screen highlights with two `HashSet<usize>` fields:
`current_highlight_lines`, rebuilt from scratch each render by `draw_rows`/`refresh_rows`, and
`last_highlight_lines`, which `store_page_state` swaps it into afterward. This relies on an
unenforced invariant: every render must fully repopulate `current_highlight_lines` for every
highlighted row.

Commit 20eef62 already had to patch around a row-skipping path in `render_partial` by adding a
manual loop that replays entries from `last_highlight_lines` for rows it chose not to redraw.
That fix is correct, but it's a symptom fix — any future row-skipping path must remember the
same replay, and nothing in the types signals that obligation. There's also no test covering
this bookkeeping, so a forgotten replay would fail silently.

Not a live bug: every row-skipping path today already replays correctly. This is about removing
future fragility, so it's safe to defer.

## Outcome

Replace the two-set swap scheme with one persistent `highlighted_lines: HashSet<usize>`,
updated incrementally: `insert` when a line is drawn with a highlight, `remove` when drawn
without one. Rows that aren't drawn are simply left untouched — already correct, no
carry-forward needed. This deletes the swap, the second set, and the carry-forward loop,
removing the whole bug class.

Note: `draw_rows` currently only ever `insert`s (it never needed to remove, since its set
always started empty). With a persistent set it also needs a `remove` branch, mirroring the
match `refresh_rows` already has.

## Plan

In `src/renderer.rs`:

1. Merge `last_highlight_lines`/`current_highlight_lines` into one persistent
   `highlighted_lines: HashSet<usize>`.
2. `draw_rows`: add the `remove(&line_idx)` branch for the no-highlight case (mirror
   `refresh_rows`).
3. `refresh_rows`: swap its insert/contains calls to the single set.
4. Delete `render_partial`'s carry-forward loop (from commit 20eef62) and `store_page_state`'s
   `mem::take` swap — `store_page_state` keeps only its `last_search` update and its
   `StatusOnly` early return (that early return is unrelated to this refactor — it's what
   makes the invalid-regex search freeze work, see `update_search_query` in `src/pager.rs`).
5. Add a unit test: a row stays highlighted across a partial render where it isn't redrawn
   (regression test for the bug commit 20eef62 fixed), and a cleared highlight is no longer
   tracked after being redrawn without one.
