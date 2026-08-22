---
type: maintenance
tags: [heading]
---

## Overview

**`min_line_index` is documented as an unconditional rule about which lines may become
headings, but no code enforces it. Both paths that decide "this line is a heading" ignore it,
and the rule holds today only because of how their callers happen to be written.**

`min_line_index` is the lowest document line that may become a heading — lines inside the
global header must never be picked, since the header is always shown anyway. The field is
documented on `HeadingConfig` as an invariant of the decision itself, not of any one caller:

```rust
struct HeadingConfig {
    /// The minimum line index that can be a heading.
    /// Lines below this index are never treated as headings, regardless of pattern matching.
    min_line_index: usize,
```

### Path 1: the heading search

`Heading::find_heading` (`src/pager/heading.rs`) scans the `range` it is given and never reads
`min_line_index`. Of its two callers:

- `resolve` builds its range as `self.config.min_line_index..(line_index + 1)`, so it happens
  to respect the bound.
- `resolve_if_found` takes a caller-supplied `Range<usize>` and passes it straight through, so
  nothing stops that range from starting before `min_line_index`.

### Path 2: the public predicate

`Heading::is_heading_start` reaches the same decision without going through `find_heading` at
all, and reads only `self.options`:

```rust
    pub fn is_heading_start(&self, doc: &mut Document, line_index: usize) -> bool {
        match &self.options {
            Some(options) => is_heading_start(doc, line_index, options),
            None => false,
        }
    }
```

Its production caller is `Pager::push_up_heading_if_needed`, which scans the rows under the
heading overlay looking for the next section's start:

```rust
        for (i, row) in rows_under_heading {
            if row.wrap_index() != 0 || row.line_index() == current_start_line {
                continue;
            }
            if self
                .heading
                .is_heading_start(&mut self.doc, row.line_index())
            {
                other_section_start = i;
                break;
            }
        }
```

A header line answering `true` here shifts the current heading's `offset`, hiding part of it on
account of a line that is never allowed to be a heading.

### Why neither path misbehaves today

For path 1, the range `scroll_down` passes to `resolve_if_found` always already satisfies the
bound: the header's rows are built from exactly its `num_lines`, so the first row past the
header is at line index `>= min_line_index`, and scrolling only increases it.

For path 2, the scan starts at viewport row `header.height()`, so it reaches a header line only
when `height() < num_lines()` with the viewport at the top of the document. Both ways that can
happen are inert:

- **Header capped** — a capped header is exactly `height() == viewport height - 1`, which forces
  `max_heading_height` to `0`. `find_heading` then returns `None` before searching,
  `start_line_index()` is `None`, and `push_up_heading_if_needed` returns before its scan.
- **Document shorter than `num_lines`** — `resolve` searches `min_line_index..(top_line + 1)`,
  an empty range while the document is still shorter than the configured header, so there is no
  heading to push up.

The gap is nonetheless real as a structural one: the invariant holds only because of how the
current callers happen to be written, not because anything enforces it. The capped-header
containment in particular is a coincidence of two independent `saturating_sub(1)` reservations,
tracked separately in `capped-header-lines-become-unreachable.md`.

## Outcome

- The `min_line_index` doc comment is true as written: no caller can treat a line before it as a
  heading, through either the search or the predicate.
- Callers get the bound for free instead of each needing to reconstruct it correctly.

## Plan

**Enforce the bound in `Heading::is_heading_start`, the one method both paths already share:**

```rust
pub fn is_heading_start(&self, doc: &mut Document, line_index: usize) -> bool {
    if line_index < self.config.min_line_index {
        return false;
    }
    match &self.options {
        Some(options) => is_heading_start(doc, line_index, options),
        None => false,
    }
}
```

**Route the search through it**, so the unguarded free function has exactly one caller:

```rust
fn find_heading(&self, doc: &mut Document, range: Range<usize>) -> Option<HeadingState> {
    let options = self.options.as_ref()?;
    ...
    for i in (range.start..range.end).rev() {
        if self.is_heading_start(doc, i) {
            nearest = Some(i);
            break;
        }
    }
```

`resolve` then no longer needs to fold the bound into its own range:

```rust
pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
    self.current = self.find_heading(doc, 0..(line_index + 1));
}
```

`resolve_if_found` needs no change; it inherits the bound automatically.

Clamping the range start in `find_heading` (`range.start.max(self.config.min_line_index)`) is
optional on top of this — it only skips a scan that would reject every line anyway, and it must
not be what the invariant rests on.

Also reword the doc comment on `min_line_index`: in a pager, "lines below this index" reads as
"further down the page", the opposite of what it means. "Lines before this index" is unambiguous.

This is behavior-preserving: no current caller can reach a line before the bound, so no existing
test result changes.

### Tests

- Call `resolve_if_found` with a range starting before `min_line_index` and assert a header line
  within that range is not picked as a heading.
- Cover the `push_up_heading_if_needed` path: with the header uncapped and a header line matching
  the heading pattern sitting under the overlay, assert the heading's visible height is not
  reduced.
