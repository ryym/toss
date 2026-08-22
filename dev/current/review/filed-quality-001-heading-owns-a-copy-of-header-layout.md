# `Heading` keeps a copy of the header's layout instead of letting `Pager` own it

Review target: `7f909cc..316c110` (`src/pager/heading.rs`, `src/pager.rs`)

## Summary

**The fix makes `Heading` hold two facts about the global header instead of one — but `Heading`
never needed to know about the header at all. `Pager` already owns the overlay, and both facts
are only used to narrow values `Pager` could just as well narrow itself.**

- `Heading` uses the header for exactly two things:
  - `min_line_index` — the lower bound of the search range in `resolve`.
  - `max_heading_height` — the row budget left below the header.
- Both are `Pager`'s domain, per `Pager`'s own doc comment (`src/pager.rs:155-157`):

  ```rust
  /// The role of [`Pager`] is to maintain this overlay correctly while applying the requested
  /// operations to update the page state.
  ```

## What the coupling costs today

**Every problem currently filed against this area is a symptom of it.**

- `dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md` — `Heading`
  received the wrong one of the two header numbers.
- `dev/issues/draft/heading-swappable-usize-header-metrics.md` — with the fix, it now receives
  both, adjacent and transposable.
- `dev/issues/draft/heading-min-line-index-not-enforced.md` — the bound is honoured by `resolve`
  only, because `Heading` holds it but does not apply it consistently.
- `dev/issues/draft/heading-line-range-clamped-by-row-count.md` — the row budget leaks into a
  line count inside `find_heading`.

None of the four exists if `Heading` never learns about the header.

## The alternative

**Give `Heading` a range and a row budget; keep the header knowledge in `Pager`.**

- `resolve` takes the range, exactly as `resolve_if_found` already does:

  ```rust
  // Heading
  pub fn resolve(&mut self, doc: &mut Document, range: Range<usize>) { ... }
  ```

- `HeadingConfig` keeps only what `ViewportSize` and the budget give it (`width`,
  `max_heading_height`); `min_line_index` disappears.
- `Pager` gets one place that builds both:

  ```rust
  // Pager
  fn resolve_heading(&mut self, up_to_line: usize) {
      let range = self.header.num_lines()..(up_to_line + 1);
      self.heading.resolve(&mut self.doc, range);
  }
  ```

### Why this is a better shape

- **One unit per parameter** — `Heading` no longer takes two `usize`s that differ only in unit,
  so nothing can be transposed.
- **One header boundary** — `header.num_lines()` is read where the `Header` is in scope; the
  `Header::contains` / `min_line_index` duplication noted in
  `dev/issues/draft/heading-swappable-usize-header-metrics.md` ("Related Concern") has one
  owner again.
- **One place per call site to get wrong** — `heading.resolve` currently appears at six sites
  (`src/pager.rs:173`, `285`, `335`, `355`, `379`, `427`), each recomputing its own reference
  row. A single `Pager::resolve_heading` also gives
  `dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md` one line to fix
  instead of an inventory of call sites to audit.
- **Testable without fabricating a header** — `Heading` unit tests pass a range and a budget,
  both of which are meaningful on their own, instead of a `(height, num_lines)` pair that no
  real `Header` could produce.

## Caveat

**This is a larger change than the bug being fixed, and it is not a prerequisite for it.**

The fix as committed is correct. The point is that the direction it takes — give `Heading` more
header state — is the opposite of the one that removes the class of bug, and each further fix in
this area (the four issues above) adds to the same coupling. Worth deciding before those are
worked on, not after.

## Assessment

- Newly introduced issue? No
- Does it block the overall goal? No

**The observation is valid; the specific alternative is only half right, and it is not free of
trade-offs either.**

The coupling is pre-existing, not introduced by `82d6b0b`. `HeadingConfig` already held
`min_line_index`; the fix only changed which header number feeds it:

```rust
// before 82d6b0b
fn new(size: &ViewportSize, global_header_height: usize) -> Self {
    ...
    min_line_index: global_header_height,
```

So this is not a defect report against the reviewed diff. It is a direction question: should
`Heading` keep learning header facts, given that several filed issues sit on that seam?

### What holds

- **`min_line_index` is `Header::contains` spelled in another module.** Already recorded as the
  "Related Concern" in `dev/issues/draft/heading-swappable-usize-header-metrics.md`. Nothing
  links the two, so what counts as "inside the header" has to be remembered twice.
- **A single `Pager::resolve_heading` is worth having on its own.** `heading.resolve` appears at
  six sites, each computing its own reference row, and
  `dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md` exists precisely because
  two of them picked `rows()[0]` instead of `rows()[header.height()]`. One helper turns that
  issue's "search for the current set and judge each hit" into a one-line fix.

### What does not

**Moving `max_heading_height` out of `Heading` is the weaker half of the proposal.**

- It contradicts the shape `Header` already has. `Header` computes its own cap from
  `ViewportSize` (`src/pager/header.rs:48-51`):

  ```rust
  fn build_rows(doc: &mut Document, size: &ViewportSize, num_lines: usize) -> Vec<Row> {
      // Reserve at least one non-header row so the header does not cover the entire viewport.
      let max_height = size.height().saturating_sub(1);
  ```

  "Each overlay derives its own row budget from `ViewportSize`" is the existing convention;
  `Heading` differs only in also subtracting the header above it, which is one number.
- It does not deliver the "one unit per parameter" benefit it claims. `Heading::new(options,
  width, max_heading_height)` is still two adjacent bare `usize`s — columns and rows. Keeping
  `size: &ViewportSize` and passing only `global_header_height` leaves exactly **one** `usize`,
  which is what actually removes the transposable pair.
- `dev/issues/draft/heading-line-range-clamped-by-row-count.md` is unaffected either way. The
  row/line mix-up there is `options.num_lines.min(max_heading_height)` inside `find_heading` —
  local to `Heading`, and present no matter who computes the budget.

### The cost the review does not price

**A search range does not reach the second path that decides "this line is a heading".**

`dev/issues/draft/heading-min-line-index-not-enforced.md` records two such paths:
`find_heading`'s scan, and the public `Heading::is_heading_start`, which
`Pager::push_up_heading_if_needed` calls directly on rows under the overlay. A range bounds the
first only. That issue's plan puts the bound in `is_heading_start` and routes the search through
it, so **one** method carries it for both paths:

```rust
pub fn is_heading_start(&self, doc: &mut Document, line_index: usize) -> bool {
    if line_index < self.config.min_line_index {
        return false;
    }
    ...
```

If `Heading` stops holding the bound, that consolidation is not available: `Pager` has to apply
`self.header.contains(..)` at the push-up scan as well as when building the range. The bound
moves from "enforced once, inside the decision" to "applied at each `Pager` call site". That is
defensible — `Pager` genuinely owns the header — but it is a real trade-off in the opposite
direction from the review's framing, and it has to be weighed before anything is closed.

### Scope

The reviewer's own caveat is right — this is larger than the bug it comments on and is not a
prerequisite for it. But the direction interacts with work already in flight: both
`heading-min-line-index-not-enforced` and `heading-swappable-usize-header-metrics` propose fixes
*inside* the shape this would remove. The latter's fourth direction already gestures at it:

> Moot if `Heading` stops holding the bound altogether — e.g. if `resolve` takes a search range
> that `Pager` builds from its own `Header` — since then the field and the argument disappear
> together.

That parenthetical is the whole of this review, and it is currently a clause inside a bullet
about something else. Making it a candidate direction in its own right is the smallest change
that puts it in front of whoever decides.

## Plans

### Plan 1: Add it as a candidate direction in `heading-swappable-usize-header-metrics`

Append a fifth bullet to that issue's `## Plan` list — alongside `&Header`, the carrier struct,
the newtypes, and "stop passing the bound on resize" — and drop the now-redundant "Moot if …"
clause from the fourth bullet, since the fifth states it fully:

- **Stop holding the bound at all; let `Pager` pass a search range.** `Heading` keeps only the
  size-derived values, and the header boundary stays in the module that owns the `Header`:

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

  Removes the transposable pair by removing one of the two values, not by renaming or wrapping
  it, and leaves a single `usize`. It also collapses the six `heading.resolve` call sites into
  one helper, which is what
  `dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md` has to audit today.

  **Weigh against it:** `Heading::is_heading_start` is a second path to the same decision and a
  range does not bound it, so `Pager` must apply `header.contains(..)` at the push-up scan too —
  the opposite of `dev/issues/draft/heading-min-line-index-not-enforced.md`, whose plan
  consolidates both paths onto one guarded method inside `Heading`. Choosing this direction means
  re-planning that issue around a `Pager`-side bound rather than adopting its current plan.
  `resolve` also changes from an inclusive reference line to an exclusive range end, so the
  `+ 1` moves into the helper.

  Test impact: `min_line_index_excludes_global_header_area` and
  `min_line_index_ignores_wrap_rows_of_the_global_header` no longer have a bound to test in
  `Heading`; they move to the `Pager` level, where a real `Header` produces the relationship
  instead of a fabricated `(height, num_lines)` pair.

Nothing is closed and nothing is decided. `heading-min-line-index-not-enforced` stays open with
its current plan; this review adds a cross-reference to it (from the new bullet) so that if this
direction is chosen, its plan is known to need reworking rather than silently invalidated.

### Plan 2: File it as its own issue

A separate `heading-search-range-owned-by-pager` issue, with
`heading-swappable-usize-header-metrics` and `heading-min-line-index-not-enforced` linking to it
as "decide this first".

Gives the ownership move a title that matches its scope, instead of hiding it in an issue about
parameter shape. The cost is a third open issue on the same seam, and a decision that has to be
made in one place while its consequences are written in two others — which is the situation the
review is already complaining about.

### Plan 3: The full refactor, now

Do the reviewer's change on this branch (or the next), with `max_heading_height` left where it
is.

Rejected. The trade-off above is unresolved, the reviewed fix is correct and complete, and this
touches every `heading.resolve` call site — a bounded bugfix should not carry an ownership-move
refactor.

## Recommendation

**Plan 1: record it as a fifth candidate direction in
`dev/issues/draft/heading-swappable-usize-header-metrics.md`, close nothing, decide later.**

- That issue's `## Plan` is explicitly `Not decided.` and already collects competing directions;
  this is the axis its existing three alternatives all miss — they debate *how* to pass the two
  values, not whether the second one should be passed at all.
- The direction is not proven better. It removes the transposable pair and the six-call-site
  spread, but it also gives up the single-method enforcement that
  `heading-min-line-index-not-enforced` is built on. That comparison needs to be made with both
  plans in view, which is exactly what listing it as a direction enables and what closing an
  issue on it would foreclose.
- Recording the counter-argument alongside the bullet matters as much as the bullet: without it
  the direction reads as a strict improvement, which is how it is framed in this review and is
  not what the code supports.

## Filed as Issue

`dev/issues/draft/heading-swappable-usize-header-metrics.md`

Added as a fourth candidate direction in that issue's `## Plan` section, alongside the counter-
argument that `Heading::is_heading_start` is a second path a search range cannot bound. Nothing
was closed: `heading-min-line-index-not-enforced` keeps its current plan, and the choice between
the directions is left open. No code change on this branch.
