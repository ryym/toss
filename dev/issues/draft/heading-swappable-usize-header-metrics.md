---
type: maintenance
tags: [heading]
---

## Overview

**`Heading::new` and `Heading::resize` take two adjacent `usize` parameters that differ only
in unit, and nothing stops them from being swapped.**

`src/pager/heading.rs`:

```rust
pub fn new(
    options: Option<HeadingOptions>,
    size: &ViewportSize,
    global_header_height: usize,
    global_header_num_lines: usize,
) -> Self {
```

- `global_header_height` — screen rows, feeds `max_heading_height`.
- `global_header_num_lines` — document lines, feeds `min_line_index`.
- The compiler cannot tell the two apart. Swapping the arguments at a call site compiles and
  silently reintroduces the same class of bug as
  `dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md`
  (a header row count used where a header line count was needed).
- Both real call sites (`src/pager.rs`, `Pager::new` and `Pager::relayout_page`) already hold
  the `Header` these two values come from, so passing them separately buys nothing.
- Unit tests for `Heading` compound the risk: they call `Heading::new(..., height, num_lines)`
  with bare adjacent integer literals (e.g. `Heading::new(opts, &size, 2, 1)`), some of which
  describe a `(height, num_lines)` pair no real `Header` could ever produce for that
  `ViewportSize` — the shape is fabricated, not derived from an actual header.

## Outcome

- No pair of adjacent `usize` parameters can be transposed without the compiler (or a test
  fabricating an unreachable state) catching it.

## Related Concern

Nearby, and worth keeping in view while choosing a direction: **"this line is inside the global
header" is currently expressed in two modules under two names.**

`src/pager/header.rs`:

```rust
    pub fn contains(&self, line_index: usize) -> bool {
        line_index < self.num_lines
    }
```

`src/pager/heading.rs` holds the same boundary as a threshold fed by `global_header_num_lines`:

```rust
struct HeadingConfig {
    /// The minimum line index that can be a heading.
    /// Lines below this index are never treated as headings, regardless of pattern matching.
    min_line_index: usize,
```

and re-derives the predicate from it as the lower bound of the heading search:

```rust
    pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
        self.current = self.find_heading(doc, self.config.min_line_index..(line_index + 1));
    }
```

"excluded from the search" is therefore exactly `line_index < num_lines` — `Header::contains`
spelled differently. Nothing links the two, so a change to what counts as "inside the header"
has to be remembered in both places, with no compiler or test signal.

A possible direction, not a requirement: let `Header` name that boundary once (e.g. an accessor
for the first document line outside the header) and have the heading search read it, so the
`&Header` option above removes the duplication rather than preserving it. `Header::contains`
answers a boolean question and is not directly reusable as a range start, so sharing the named
value is likely a better fit than sharing the predicate.

## Plan

Not decided. The first four directions are alternatives to each other, roughly in order of
how much they change. The last is on a different axis and combines with any of them:

- **Pass `&Header` instead of the two `usize`s.** `Heading` and `Header` are both private
  submodules of `pager` already, so there is no encapsulation boundary being crossed.
  `HeadingConfig::new` would read `header.height()` / `header.num_lines()` itself. Removes the
  transposable pair entirely and forces tests to go through a real `Header`, which also
  surfaces (and may force resolving) cases where a capped header leaves no room for a heading.
- **Introduce a small named carrier struct** (e.g. `HeaderMetrics { height, num_lines }`).
  Keeps `Heading` decoupled from `Header`'s type; the fields are named at the call site, but a
  `Header` can still fill the struct wrongly — the mistake just becomes local to one place.
- **Newtype the units** (e.g. `Rows(usize)` / `Lines(usize)`). Strongest guarantee, and
  generalizes: `dev/issues/draft/heading-line-range-clamped-by-row-count.md` records the same
  row/line mix-up elsewhere in this file. Largest change of the first three, and touches call sites
  beyond `Heading::new`/`resize`.
- **Stop holding the bound at all; let `Pager` pass a search range.** The two values are not
  symmetric: `max_heading_height` is screen space, which `Heading` needs to lay itself out, while
  `min_line_index` only narrows where the search may start — something the caller can express
  directly. `Heading` then keeps only the size-derived values, and the header boundary stays in
  the module that owns the `Header`:

  ```rust
  // src/pager/heading.rs
  pub fn new(options: Option<HeadingOptions>, size: &ViewportSize, global_header_height: usize) -> Self

  /// Find and set the heading nearest to `range.end`, searching within `range`.
  pub fn resolve(&mut self, doc: &mut Document, range: Range<usize>) {
      self.current = self.find_heading(doc, range);
  }
  ```

  ```rust
  // src/pager.rs
  fn resolve_heading(&mut self, up_to_line: usize) {
      let range = self.header.num_lines()..(up_to_line + 1);
      self.heading.resolve(&mut self.doc, range);
  }
  ```

  Removes the transposable pair by removing one of the two values rather than renaming or
  wrapping it, leaving a single `usize`. `resolve_if_found` already takes a range, so the two
  entry points converge on one shape. It also answers the "Related Concern" above: the header
  boundary is read where the `Header` is in scope, so `Header::contains` and the search bound
  stop being the same rule written twice. As a side effect the six `heading.resolve` call sites
  in `src/pager.rs` collapse into one helper, which is the audit
  `dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md` currently has to do.

  **Weigh against it:** `Heading::is_heading_start` is a second path to the same decision, and a
  search range does not bound it — `Pager::push_up_heading_if_needed` calls it directly on rows
  under the overlay. `Pager` would have to apply `header.contains(..)` there as well, so the
  bound goes from "enforced once, inside the decision" to "applied at each `Pager` call site".
  That is the opposite of `dev/issues/draft/heading-min-line-index-not-enforced.md`, whose plan
  consolidates both paths onto one guarded method inside `Heading`. Choosing this direction means
  re-planning that issue around a `Pager`-side bound instead of adopting its current plan.
  Note also that `resolve` changes from an inclusive reference line to an exclusive range end.
- **Stop passing the bound on resize.** `Header::num_lines` is written once in `Header::new` and
  never again — `Header::resize` only rebuilds `rows` — so `Heading::resize`'s
  `global_header_num_lines` argument is always the value `Heading::new` already received. Hold it
  as a plain field on `Heading`, set in `new`, and leave `HeadingConfig` to the size-derived
  values (`width`, `max_heading_height`):

  ```rust
  pub(super) struct Heading {
      /// The minimum line index that can be a heading. Fixed for the life of the `Pager`:
      /// the global header always covers the same document lines, however they are rendered.
      min_line_index: usize,
      config: HeadingConfig,
      // ...
  }

  pub fn resize(&mut self, doc: &mut Document, size: &ViewportSize, global_header_height: usize) {
      self.config = HeadingConfig::new(size, global_header_height);
      // ...
  }
  ```

  On its own this cuts the transposable pair from two call sites (`Pager::new` and
  `Pager::relayout_page`) to one, which lowers the stakes of whichever direction above is chosen.
  It also makes the distinction visible in the type: a resize changes how many rows the header
  renders as, never which document lines it covers. Moot under the previous direction, where the
  field and the argument disappear together.

Whichever direction is chosen should also update (or replace) the `Heading` unit tests so they
can no longer construct a `(height, num_lines)` pair that no real `Header` could produce.
