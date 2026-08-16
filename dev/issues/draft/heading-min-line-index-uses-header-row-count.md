---
type: bugfix
tags: [heading, header]
---

## Overview

**`Heading` uses the global header's *row* count as a *line* index.**

- **What it should hold** — `min_line_index` is the lowest document line that may become a
  heading. Lines inside the global header must not, since the header is always shown anyway.
  A **line** count.
- **What it is fed** — `Header::height()`, which is `Header::rows().len()`. A **row** count.

The two agree only while every header line occupies exactly one row. They diverge in both
directions:

- **Header lines wrap** (narrow terminal, long header line): `height() > num_lines`, so lines
  just below the header are wrongly excluded, and a heading there is never found.
- **Header is capped** (short terminal): `build_rows` caps the header at `viewport height - 1`
  rows, so `height() < num_lines`, and a line that *is* part of the header can be picked as a
  heading.

**No reproduction is recorded.** It needs:

- `--header` and `--heading` together.
- A width narrow enough to wrap a header line, or a height short enough to cap the header.

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

Unit tests in `src/pager/heading.rs`, mirroring `min_line_index_excludes_global_header_area`:

- A wrapped header line does not push `min_line_index` past the header, so a heading on the
  first line below the header is still found.
- A capped header does not pull `min_line_index` into the header, so a header line is never
  picked as a heading.
