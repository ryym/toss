---
type: bugfix
tags: [resize, heading]
---

## Overview

**A resize re-wraps the sticky heading but never recomputes the rest of its state.**

Three symptoms follow, all requiring `--heading`:

- **Panic** — shrinking the terminal while the heading is pushed up panics with
  `range start index N out of range for slice of length M`.
- **Stale push-up** — the heading stays pushed up by the amount computed for the old size, so it
  is rendered from the wrong row and content rows leak in below it.
- **Stale line range** — a heading resolved while the terminal was small stays truncated after
  the terminal grows, showing fewer lines than `--heading-lines`.

## Reproduction

`src/tests/heading_resize.rs` drives each symptom with a `MockScreen` script. Every case
encodes the *expected* output and is marked `#[should_panic]`, since the bug makes it panic or
diff today; the fix drops the attributes.

- **`shrink_after_push_up_rebuilds_heading`** — the panic.
  - `--heading-lines 4` on 20x8, scrolled until `# B` pushes the heading up by 3, then shrunk to
    20x3 where only 1 heading row fits.
  - ```
    thread '...' panicked at src/pager/heading.rs:72:31:
    range start index 3 out of range for slice of length 1
    ```
- **`shrink_recomputes_push_up_offset`** — stale push-up on a height change.
  - `--heading-lines 4` on 20x8, scrolled until the heading is pushed up by 1, then shrunk to
    20x5 where the heading is capped to 3 rows and no longer reaches `# B`.
  - Shows `a1` / `a2` / `body a2` / `# B` instead of `# A` / `a1` / `a2` / `# B`.
  - The test carries a comment explaining why the expected output starts at `# A`.
- **`width_change_recomputes_push_up_offset`** — stale push-up with the heading size unchanged.
  - `--heading-lines 2` on 12x7 narrowed to 8x7. The heading stays 2 rows, but the body line
    above `# B` rewraps, moving `# B` out of the overlay area.
  - Shows `sub a` / `y 1` / `# B` instead of `# A` / `sub a` / `# B`: the heading stays pushed up
    and the tail of the rewrapped line leaks in.
- **`grow_restores_full_heading_height`** — stale line range.
  - `--heading-lines 3` on 20x4 (which caps the heading at 2 rows) grown to 20x10.
  - The heading stays `# A` / `a1` although there is now room for `a2`.

`resize_when_document_fits_entirely_in_header` in the same file is not a symptom but a guard: it
passes today and locks the `rows().len() == header.height()` boundary that the fix has to keep
safe (see "Why `top_line` is an `Option`").

## Root Cause

**`Heading::resize` rebuilds only `h.rows`; `h.line_range` and `h.offset` are left stale.**

`Pager::relayout_page` (`src/pager.rs:313-318`) only calls `Heading::resize`:

```rust
fn relayout_page(&mut self, size: ViewportSize) {
    self.header.resize(&mut self.doc, &size);
    self.heading
        .resize(&mut self.doc, &size, self.header.height());
    self.viewport.resize(&mut self.doc, size);
}
```

`Heading::resize` (`src/pager/heading.rs:96-106`) rebuilds `h.rows` for the new width and
`max_heading_height`, but keeps the other fields untouched:

- **`h.offset`** — set by `Heading::push_up` against the *previous* row count.
  - A smaller `max_heading_height` produces fewer rows.
  - `Heading::rows()` (`src/pager/heading.rs:72`) then evaluates `&h.rows[h.offset..]` with
    `offset > rows.len()` and panics.
- **`h.line_range`** — clamped to the old `max_heading_height` in `Heading::find_heading`
  (`src/pager/heading.rs:152-153`, `num_lines.min(max_heading_height)`).
  - Growing the terminal raises `max_heading_height`, but the range is never widened again.

`Pager::resize` also never calls `heading.resolve` or `push_up_heading_if_needed`, so the
push-up amount is not re-derived for the new layout either.

## Plan

**Fold the re-resolve into a single `Heading::relayout` entry point, so a half-updated heading
cannot be produced.**

- `Heading::resize` does two jobs today: update `config`, and rebuild `h.rows`.
  - `resolve` depends on the first one (`find_heading` reads `config`).
  - `resolve` makes the second one dead work: `find_heading` rebuilds `line_range`, `rows` and
    `offset` from scratch.
- Replace `Heading::resize` with
  `Heading::relayout(doc, size, global_header_height, top_line: Option<usize>)`
  = config update + `resolve`.
  - Pairing them at the call site would work too, but only as a convention to remember. With a
    single entry point the inconsistent state is not reachable.

```rust
/// `top_line` is the first viewport line below the global header.
/// `None` means there is no such line, so no heading can be resolved.
pub fn relayout(
    &mut self,
    doc: &mut Document,
    size: &ViewportSize,
    global_header_height: usize,
    top_line: Option<usize>,
) {
    self.config = HeadingConfig::new(size, global_header_height);
    self.current = top_line
        .and_then(|line| self.find_heading(doc, self.config.min_line_index..(line + 1)));
}
```

### Call order in `Pager::relayout_page`

`resolve` takes the new top line as input, and that is only known once the viewport has been
rebuilt, so the current heading-before-viewport order has to be swapped:

```rust
fn relayout_page(&mut self, size: ViewportSize) {
    self.header.resize(&mut self.doc, &size);   // determines min_line_index
    self.viewport.resize(&mut self.doc, size);  // determines the new top line
    let top_line = self
        .viewport
        .rows()
        .get(self.header.height())
        .map(|row| row.line_index());
    self.heading
        .relayout(&mut self.doc, &size, self.header.height(), top_line);
    self.push_up_heading_if_needed();
}
```

`Viewport` is unaware of header and heading rows by design (see the `Pager` doc comment), so the
swap creates no new dependency.

### Why `top_line` is an `Option`

The heading sticks to the first viewport line *below* the global header, so the reference row is
`viewport.rows()[header.height()]` — not `rows()[0]`, which the global header covers. Resolving
against `rows()[0]` would pick a heading from an earlier section whenever a new one starts within
the covered rows. (`rows()[total_header_height()]` cannot be used: the heading's own height is
what is being computed.) `Pager::scroll_up` / `scroll_down` already use `rows()[header.height()]`;
`Pager::pump_input` uses `rows().first()`, which this change also corrects.

That index can be out of bounds, so it must not be indexed raw:

- `rows().len() == header.height()` — the viewport holds nothing but header rows, e.g. a document
  short enough to fit entirely in the header, or a streaming document whose arrived lines are all
  header lines. Covered by `tests::heading_resize::resize_when_document_fits_entirely_in_header`.
- `rows().len() < header.height()` — reachable only while the viewport is under-filled near the
  document end, which panics in `Pager::snapshot` before reaching here. That is
  `dev/issues/open/20260815-resize-does-not-refill-viewport-near-document-end.md`, not this issue.

`None` is the honest answer in those cases: with no content row below the header there is nothing
for a heading to stick to, so `current` is unset. Falling back to line 0 would give the same
result — the search range `min_line_index..1` is empty whenever the fallback applies — but only
by coincidence, so the `Option` states it instead.

`relayout` must still be called even when `top_line` is `None`, for `config`: `width`,
`max_heading_height` and `min_line_index` all change with the new size, and skipping the call
would leave exactly the half-updated state this issue is about.

Note that `max_heading_height` is `0` whenever the global header is capped
(`header.height() == viewport height - 1`), and `find_heading` returns `None` on a
`max_heading_height` of 0. So no heading exists while the header is capped, at any top line.

### `push_up_heading_if_needed` is still required

| Applied                          | Panic  | `line_range` | `offset`             |
| -------------------------------- | ------ | ------------ | -------------------- |
| today (`resize` only)            | occurs | stale        | stale                |
| `+ resolve`                      | gone   | correct      | reset to 0           |
| `+ push_up_heading_if_needed`    | gone   | correct      | re-derived correctly |

- `resolve` builds a fresh `HeadingState` with `offset: 0`, which makes `offset > rows.len()`
  unrepresentable — that alone removes the panic.
- 0 is not always the correct offset. When the next section's heading still overlaps the overlay
  area after the resize, the push-up has to be re-derived; otherwise the two headings visibly
  collapse onto each other.

### Other

- Remove the duplicate resolve in the streaming path: `Pager::pump_input`
  (`src/pager.rs:278-286`) goes through `relayout_page` as well.
- `relayout_page` gains `push_up_heading_if_needed`, so the streaming path runs it too. It is a
  no-op there: that branch only runs while the viewport is under-filled, which means the whole
  known document fits and the heading starts at the top row. Adding the call is safe even where
  it is not a no-op — `Heading::push_up` assigns the offset rather than accumulating it, and the
  amount is derived from the current viewport rows on every call.
- Coordinate with `dev/issues/open/20260815-resize-does-not-refill-viewport-near-document-end.md`.
  - It plans to make the viewport's top row move during a resize, which makes re-resolving the
    heading mandatory.
- The expected outputs assume a resize keeps anchoring the viewport's top row. Anchoring the
  first *visible* row instead was considered and cancelled in
  `dev/issues/closed/20260816-preserve-visible-frame-on-resize.md`; re-deriving the push-up is
  required either way.
- Cost: `resolve` scans backward from the top line to `min_line_index` and stops at the first
  match, so a document with no heading above the current position means a full scan on every
  resize. The existing `jump_to` path already carries the same cost, so this adds no new class
  of work.
- **Fix `dev/issues/draft/heading-min-line-index-uses-header-row-count.md` first.** It is a
  prerequisite, not just a signature collision: `min_line_index` is `Header::height()`, a row
  count, so narrowing the terminal until a header line wraps raises it past a heading that is
  not in the header at all, and `relayout` then drops that heading. Today's `resize` never
  drops a heading, so the defect is invisible until this change lands. With `min_line_index`
  fed `Header::num_lines` it is constant across resizes and the case cannot arise.
- Drop the `#[should_panic]` attributes from `src/tests/heading_resize.rs`; the cases already
  hold the expected output.
- Update `heading.rs::resize_rebuilds_rows_at_new_width`: it calls `Heading::resize` directly,
  which no longer exists. Add unit cases for `relayout` with `top_line: None` and for a
  `top_line` whose heading is narrower than the previous one.

## Appendix: `resolve` never drops a heading that should stay

Why replacing `resize` with `relayout` cannot make a heading disappear when it should not.
Nothing here calls for extra work; it is recorded because the change turns "the heading always
survives a resize" into "the heading is re-derived", which is the kind of step a reviewer will
want checked rather than asserted.

`resize` keeps the current heading unconditionally; `relayout` re-runs `find_heading` and unsets
it when nothing is found. That can only unset a heading the new layout should not show.

Let `S` be the current heading's start line. `is_heading_start` reads only the document and
`options` (`src/pager/heading.rs:169-187`), neither of which a resize changes, so `S` is still a
heading start afterwards. `find_heading` therefore returns `S` — or a nearer one, which is the
point of re-resolving — whenever `S` falls in `min_line_index..(top_line + 1)`. Only two things
can put `S` outside that range:

- `max_heading_height == 0`, where `find_heading` returns early. This holds exactly while the
  global header is capped, i.e. when no row is left for a heading, so unsetting is correct.
- `min_line_index > S`. With `min_line_index` fed `Header::num_lines` it never changes on a
  resize, so this cannot happen. It can with today's row count — see the prerequisite above.

`top_line` cannot exclude `S`: a sticky heading sits at or above the viewport's top line.
