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
  `Heading::relayout(doc, size, global_header_height, top_line)` = config update + `resolve`.
  - Pairing them at the call site would work too, but only as a convention to remember. With a
    single entry point the inconsistent state is not reachable.
  - A standalone `resize` then no longer exists, so clamping `offset` defensively is
    unnecessary; a `debug_assert!(offset <= rows.len())` in `Heading::rows` is enough.

### Call order in `Pager::relayout_page`

`resolve` takes the new top line as input, and that is only known once the viewport has been
rebuilt, so the current heading-before-viewport order has to be swapped:

```rust
fn relayout_page(&mut self, size: ViewportSize) {
    self.header.resize(&mut self.doc, &size);   // determines min_line_index
    self.viewport.resize(&mut self.doc, size);  // determines the new top line
    let top_line = /* new top row's line index */;
    self.heading
        .relayout(&mut self.doc, &size, self.header.height(), top_line);
    self.push_up_heading_if_needed();
}
```

`Viewport` is unaware of header and heading rows by design (see the `Pager` doc comment), so the
swap creates no new dependency.

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
- Coordinate with `dev/issues/open/20260815-resize-does-not-refill-viewport-near-document-end.md`.
  - It plans to make the viewport's top row move during a resize, which makes re-resolving the
    heading mandatory.
- Cost: `resolve` scans backward from the top line to `min_line_index` and stops at the first
  match, so a document with no heading above the current position means a full scan on every
  resize. The existing `jump_to` path already carries the same cost, so this adds no new class
  of work.
- Drop the `#[should_panic]` attributes from `src/tests/heading_resize.rs`; the cases already
  hold the expected output.
