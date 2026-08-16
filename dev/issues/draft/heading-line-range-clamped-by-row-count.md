---
type: maintenance
tags: [heading]
---

## Overview

**`Heading::find_heading` clamps a *line* count with a *row* count.**

`src/pager/heading.rs:152-153`:

```rust
let height = options.num_lines.min(self.config.max_heading_height);
let line_range = nearest..(nearest + height);
```

- `options.num_lines` — how many document **lines** form a heading (`--heading-lines`).
- `max_heading_height` — how many screen **rows** are left for the heading.

The same unit confusion as `dev/issues/draft/heading-min-line-index-uses-header-row-count.md`,
in the opposite direction: there a row count is read as a line count, here a line count is cut
down by a row count.

**No user-visible symptom is known.** `rows::from_lines` (`src/pager/rows.rs:7-23`) truncates to
`max_rows` unconditionally, so the rendered rows are already correct with or without the clamp.
The clamp only shrinks `line_range`, which is used solely to rebuild the rows. Filed as cleanup,
not as a bug.

Both directions still misdescribe the heading once lines wrap:

- **Clamp too tight** (no wrapping, `num_lines > max_heading_height`): `line_range` records
  fewer lines than `--heading-lines` asks for. Harmless while the rows are rebuilt for the same
  size; the stale case where they are not is
  `dev/issues/draft/heading-state-not-recomputed-on-resize.md`.
- **Clamp too loose** (heading lines wrap): e.g. `num_lines = 3`, each line taking 5 rows,
  `max_heading_height = 10`. The clamp leaves `height = 3`, but `from_lines` fits only lines 0-1.
  `line_range` then claims a line that no row represents.

## Outcome

- `line_range` means one thing — the lines the heading occupies — and agrees with `rows`.
- One less place where a row count stands in for a line count, removing the pattern that
  `heading-min-line-index-uses-header-row-count` fixes elsewhere.

## Plan

**Drop the clamp and derive `line_range` from the rows that were actually built.**

```rust
let line_range = nearest..(nearest + options.num_lines);
let rows = rows::from_lines(doc, self.config.width, line_range, self.config.max_heading_height);
let line_range = nearest..(rows.last().map_or(nearest, |r| r.line_index() + 1));
```

- `from_lines` stays the single place that decides how much fits.
- An empty `rows` (the line range is past the end of the document) yields an empty `line_range`,
  matching what is displayed.

Check whether `HeadingState::line_range` is worth keeping at all once it is derived from `rows`:
its only readers are `Heading::start_line_index` and the rebuild in `Heading::resize`, and both
could read `rows` instead. Decide when addressing this; if it goes away, so does the mismatch.

### Tests

Unit tests in `src/pager/heading.rs`:

- A heading whose lines wrap past `max_heading_height` reports a `line_range` covering only the
  lines that have rows.
- `--heading-lines` larger than `max_heading_height` without wrapping keeps the rendered rows
  unchanged from today.

### Order

Independent of `heading-min-line-index-uses-header-row-count`, but touches the same function as
`heading-state-not-recomputed-on-resize`, whose plan replaces `Heading::resize` with
`Heading::relayout`. Do this one after that to avoid reworking the same lines twice.
