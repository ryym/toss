# `is_heading_start` is a second, unbounded way to declare a line a heading

Review target: `7f909cc..316c110` (`src/pager/heading.rs`, `src/pager.rs`)

## Summary

**`min_line_index` guards only the search inside `find_heading`. `Pager::push_up_heading_if_needed`
decides "this line starts a heading" through `Heading::is_heading_start`, which never consults the
bound, so the invariant the fix strengthens is enforced on one of the two paths.**

- The fix corrects *which* number bounds the search (lines, not rows).
- It does not change *how many* code paths respect that bound: still one of two.

## The unguarded path

`src/pager.rs:465-474`, inside `push_up_heading_if_needed`:

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

`Heading::is_heading_start` (`src/pager/heading.rs:58-63`) forwards straight to the free function
and reads no config:

```rust
    pub fn is_heading_start(&self, doc: &mut Document, line_index: usize) -> bool {
        match &self.options {
            Some(options) => is_heading_start(doc, line_index, options),
            None => false,
        }
    }
```

A header line answering `true` here shifts the current heading's `offset`, i.e. hides part of it,
on account of a line that is never allowed to be a heading.

## Why it is not reachable today

**Only because a capped header forces `max_heading_height` to `0`.**

The scan starts at viewport row `header.height()`, so it reaches a header line only when
`height() < num_lines()` with the viewport at the top of the document:

- **Header capped** — `max_heading_height == 0`, so `find_heading` never returns a heading,
  `start_line_index()` is `None`, and `push_up_heading_if_needed` returns before the scan.
- **Document shorter than `num_lines`** — no viewport row past `header.height()` exists yet, and
  there is no heading to push up either.

This is the same cap-dependent containment already recorded in
`dev/issues/draft/capped-header-lines-become-unreachable.md`. That issue lists three consumers
leaning on the cap (`jump_to`, the reserved-row constants, the streaming fill gate); this is a
fourth it does not mention.

## Relation to the existing draft

**`dev/issues/draft/heading-min-line-index-not-enforced.md` does not close this path.**

Its plan clamps the range inside `find_heading`:

```rust
    let range = range.start.max(self.config.min_line_index)..range.end;
```

`is_heading_start` does not go through `find_heading`, so after that fix the doc comment on
`min_line_index` ("Lines below this index are never treated as headings, regardless of pattern
matching") would still be false for this caller. Either the bound belongs on the shared
`is_heading_start` predicate, or the doc comment should say it bounds the *search* only.

## Assessment

- Newly introduced issue? No
- Does it block the overall goal? No

**The observation is factually correct, and its most useful part is not the gap itself but what
it says about the existing draft's plan.**

Verified against the code:

- `Heading::is_heading_start` (`src/pager/heading.rs:58-63`) reads only `self.options`, never
  `self.config`, so it is a second entry point into the shared predicate that skips
  `min_line_index` entirely. Its only production caller is
  `Pager::push_up_heading_if_needed` (`src/pager.rs:471`).
- The containment argument holds. `HeadingConfig::new` computes
  `max_heading_height = height - global_header_height - 1`, and a capped header is exactly
  `height() == size.height() - 1` (`Header::build_rows`), which forces `max_heading_height == 0`
  and makes `find_heading` bail before anything resolves. With no current heading,
  `push_up_heading_if_needed` returns at its first `match`. The short-document case is likewise
  inert: `resolve` searches `min_line_index..top_line + 1`, which is empty while the document is
  still shorter than the configured header.

So there is no reachable misbehavior today, and nothing here was introduced by
`7f909cc..316c110` — `is_heading_start` ignored the bound before the change as well. The change
only altered *which* value `min_line_index` holds.

What is worth acting on is the interaction with `dev/issues/draft/heading-min-line-index-not-enforced.md`:
its plan clamps the range inside `find_heading`, which does not touch the `is_heading_start`
path. If that issue is implemented as drafted, its stated Outcome — "the `min_line_index` doc
comment is true as written" — would still be false. That is a defect in the draft's plan, not a
new issue, so it belongs in that draft rather than in a separate one.

## Plans

### Plan 1: Extend the existing draft to guard the shared predicate, not the search range

Amend `dev/issues/draft/heading-min-line-index-not-enforced.md` so its plan enforces the bound at
the one place both paths already share — the `Heading::is_heading_start` method:

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

and route the search through it, so the free function has exactly one guarded caller:

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

This subsumes the draft's `range.start.max(self.config.min_line_index)` clamp (that clamp may be
kept purely to skip a pointless scan, but is no longer what makes the invariant hold), and keeps
the rest of the draft — the `resolve` simplification to `0..(line_index + 1)`, the "lines below"
wording fix, the `resolve_if_found` test — as written.

Also add to the draft's test list a `Pager`-level or unit case covering the
`push_up_heading_if_needed` path: with the header uncapped and a header line matching the
heading pattern under the overlay, the heading's `offset` must not shift.

Additionally, add this to the consumer list in
`dev/issues/draft/capped-header-lines-become-unreachable.md` § *The gap widens if...*, as a
fourth thing the cap coincidentally contains, alongside `jump_to`, the reserved-row constants,
and the streaming fill gate.

### Plan 2: Narrow the doc comment instead of the behavior

Leave both code paths as they are and rewrite the `min_line_index` comment so it describes only
what is actually enforced:

```rust
struct HeadingConfig {
    /// The lowest line index the heading *search* considers.
    /// Note this does not constrain `Heading::is_heading_start`, which answers for any line.
    min_line_index: usize,
```

Cheapest option and it removes the false statement, but it leaves the bound as something each
caller has to remember, which is exactly the structural complaint the existing draft was filed
about. Only worth taking if the decision is that `is_heading_start` should stay a pure predicate.

### Plan 3: Fix it now, in this branch

Apply the `is_heading_start` guard from Plan 1 directly, ahead of the draft issue. Behavior-
preserving today (no reachable case), a handful of lines, and it makes the doc comment true
immediately. Rejected as the recommendation because it drags a pre-existing, unreachable
structural gap into a branch whose goal was the `height` vs `num_lines` bound, and it would land
half of the draft issue's plan while leaving the other half (the `find_heading` clamp, the
`resolve` simplification, the wording fix) unfiled work.

## Recommendation

**Plan 1.**

The gap predates the reviewed change and is unreachable, so it should not be fixed here. But the
finding is not merely "file another issue": it shows the existing draft's plan would not deliver
its own stated outcome. Folding the correction into
`dev/issues/draft/heading-min-line-index-not-enforced.md` keeps one issue for one invariant,
costs only a documentation edit now, and produces the simpler end state — a single guarded
predicate that both the search and `push_up_heading_if_needed` go through — instead of two places
that each have to remember the bound.

## Filed as Issue

- `dev/issues/draft/heading-min-line-index-not-enforced.md` — rewritten so the invariant is
  enforced on the shared `is_heading_start` predicate rather than on the search range.
- `dev/issues/draft/capped-header-lines-become-unreachable.md` — records the push-up scan as a
  fourth consumer contained only by the header cap.
