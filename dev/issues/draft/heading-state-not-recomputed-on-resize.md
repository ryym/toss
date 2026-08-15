---
type: bugfix
tags: [resize, heading]
---

## Overview

**A resize re-wraps the sticky heading but never recomputes the rest of its state.**

Two symptoms follow, both requiring `--heading`:

- **Panic** — shrinking the terminal while the heading is pushed up panics with
  `range start index N out of range for slice of length M`.
- **Stale heading** — a heading resolved while the terminal was small stays truncated after the
  terminal grows, showing fewer lines than `--heading-lines`.

## Reproduction

Both cases use this document with `--heading '^# '`:

```
# A
a1
a2
a3
body a1
body a2
# B
b1
b2
b3
body b1
body b2
tail
```

### Panic

Setup:

- `--heading-lines 4`, 20x8 screen
- scroll down 5 rows — `# B` reaches the second overlay row, so the heading is pushed up by 3
- shrink to 20x3

```rust
run_test_screen(TestCase {
    screen_width: 20,
    screen_height: 8,
    content: CONTENT,
    options: Options {
        heading: Some(heading_opts("^# ", 4)),
        ..Default::default()
    },
    events: vec![key('j'), key('j'), key('j'), key('j'), key('j'), resize(20, 3), key('q')],
    ..Default::default()
});
```

```
thread '...' panicked at src/pager/heading.rs:72:31:
range start index 3 out of range for slice of length 1
```

### Stale heading

Setup:

- `--heading-lines 3`, 20x4 screen — viewport height 3, so `max_heading_height` is 2
- scroll down 2 rows
- grow to 20x10

```rust
events: vec![key('j'), key('j'), resize(20, 10), key('q')],
```

Result: the heading stays `# A` / `a1`, although a 20x10 screen has room for the configured 3
lines (`# A` / `a1` / `a2`).

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
- Add e2e cases in `src/tests/heading.rs` for both symptoms.
