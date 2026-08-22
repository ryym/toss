---
type: bugfix
tags: [heading, header]
status: doing
opened_at: 2026-08-17T13:41:15Z
---

## Overview

**`Heading` uses the global header's _row_ count as a _line_ index.**

- **What it should hold** — `min_line_index` is the lowest document line that may become a
  heading. Lines inside the global header must not, since the header is always shown anyway.
  A **line** count.
- **What it is fed** — `Header::height()`, which is `Header::rows().len()`. A **row** count.

The two agree only while every header line occupies exactly one row. They diverge in one
direction:

- **Header lines wrap** (narrow terminal, long header line): `height() > num_lines`, so lines
  just below the header are wrongly excluded, and a heading there is never found.

They do not diverge the other way, for two independent reasons:

- **The header is capped** — `build_rows` caps the header at `viewport height - 1` rows when it
  is short, so `height() < num_lines` there too. But the same cap forces `max_heading_height` to
  `0` in `HeadingConfig`, and `find_heading` returns `None` before considering any line whenever
  `max_heading_height` is `0`. So while the header is capped, no heading exists at all — the
  row/line mismatch is unobservable in this direction.
- **The document has fewer lines than the header is configured for** — `build_rows` skips lines
  that do not exist rather than padding for them, so a file shorter than `--header-lines`, or a
  streamed document whose header lines have not all arrived yet, also produces
  `height() < num_lines` without any cap. This is harmless for a different reason: every
  `line_index` that reaches `Heading::resolve`/`resolve_if_found` comes from a row the document
  already has, so while it's shorter than `num_lines`, `line_index < doc.line_count() <=
  num_lines` always holds. The search range `min_line_index..(line_index + 1)` (`min_line_index ==
  num_lines`) is therefore always empty — no header line can ever be reached as a heading
  candidate through this path, regardless of when the rest of the document arrives.

## Reproduction

`heading_just_below_wrapped_header_line` in `src/tests/heading.rs` covers the wrapping
direction: a 1-line header wraps into 2 rows at width 8, so `# A` on line 1 is excluded and
never becomes the sticky heading after `G`. It encodes the _expected_ output and is marked
`#[should_panic]`.

Note that `min_line_index` only gates `Heading::resolve`, not `Heading::resolve_if_found`,
so plain downward scrolling still finds the heading. The reproduction uses `G` to go through
`resolve`.

## Root Cause

**`HeadingConfig::new` (`src/pager/heading.rs:198-210`) stores its single argument straight into
`min_line_index`:**

```rust
fn new(size: &ViewportSize, global_header_height: usize) -> Self {
    let max_heading_height = size.height().saturating_sub(global_header_height).saturating_sub(1);
    Self {
        min_line_index: global_header_height,
        max_heading_height,
        width: size.width(),
    }
}
```

Every caller passes `self.header.height()` (`src/pager.rs:172`, `src/pager.rs:316`), so that one
argument serves two different units:

- `max_heading_height` — screen space left below the header. A **row** count; correct as is.
- `min_line_index` — the first document line outside the header. A **line** count; wrong.

`Header` already keeps the right value and uses it correctly elsewhere: `Header::contains`
(`src/pager/header.rs:34-36`) compares against `num_lines`, not `height()`.

## Plan

**Pass the two units separately instead of deriving one from the other.**

- Expose `Header::num_lines()` and hand it to `Heading` alongside the height, e.g.
  `HeadingConfig::new(size, header_height, header_num_lines)`.
- Keep `max_heading_height` derived from the row count.

### Tests

Drop `#[should_panic]` from `heading_just_below_wrapped_header_line`; it should pass as is.

Unit tests in `src/pager/heading.rs`, mirroring `min_line_index_excludes_global_header_area`:

- A wrapped header line does not push `min_line_index` past the header, so a heading on the
  first line below the header is still found.
