---
type: maintenance
tags: [heading]
---

## Overview

**`Heading::find_heading` only bounds its search by `min_line_index` when the caller folds
that bound into the range itself; it does not enforce it internally.**

`min_line_index` is the lowest document line that may become a heading — lines inside the
global header must never be picked, since the header is always shown anyway. The field is
documented on `HeadingConfig` as an unconditional invariant of the search:

```rust
struct HeadingConfig {
    /// The minimum line index that can be a heading.
    /// Lines below this index are never treated as headings, regardless of pattern matching.
    min_line_index: usize,
```

But `find_heading` (`src/pager/heading.rs`) scans `range` exactly as given and never reads
`min_line_index`. Of its two callers:

- `resolve` builds its range as `self.config.min_line_index..(line_index + 1)`, so it happens
  to respect the bound.
- `resolve_if_found` takes a caller-supplied `Range<usize>` and passes it straight through, so
  nothing stops that range from starting below `min_line_index`.

As of commit `82d6b0b`, `resolve_if_found` has exactly one call site, `Pager::scroll_down`, and
the range it passes always turns out to already satisfy the bound (the header's rows are built
from exactly its `num_lines`, so the first row past the header is always at line index
`>= min_line_index`, and scrolling only increases it; a capped header additionally forces
`max_heading_height == 0`, at which point `find_heading` returns `None` before looking at the
range at all). So there is no known reachable case at that commit where a header line gets
picked as a heading through this path.

The gap is nonetheless real as a structural one: the invariant only holds because of how the
current caller happens to construct its range, not because `find_heading` enforces it. A future
caller of `resolve_if_found` (or a change to how `scroll_down` computes its range) could pass a
range starting before `min_line_index`, and `find_heading` would silently honor it, picking a
header line as a heading.

## Outcome

- The `min_line_index` doc comment is true as written: no caller of `find_heading` can pick a
  line before it, regardless of what range it passes in.
- `resolve_if_found` (and any future caller of `find_heading`) gets the bound for free instead
  of needing to reconstruct it correctly.

## Plan

**Move the bound into `find_heading`, where every search passes through it:**

```rust
fn find_heading(&self, doc: &mut Document, range: Range<usize>) -> Option<HeadingState> {
    let range = range.start.max(self.config.min_line_index)..range.end;
    ...
}
```

`resolve` then no longer needs to fold the bound into its own range:

```rust
pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
    self.current = self.find_heading(doc, 0..(line_index + 1));
}
```

`resolve_if_found` needs no change; it inherits the bound automatically once `find_heading`
enforces it.

Also reword the doc comment on `min_line_index`: in a pager, "lines below this index" reads as
"further down the page", the opposite of what it means. "Lines before this index" is
unambiguous.

This is a small, behavior-preserving change: both current callers already produce ranges that
satisfy the bound, so no existing test result changes.

### Tests

Add a unit test that calls `resolve_if_found` with a range starting before `min_line_index` and
asserts a header line within that range is not picked as a heading — the case `find_heading`
does not currently guard against.
