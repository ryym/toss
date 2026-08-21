# min_line_index is enforced at one call site, not in find_heading

Review target: `7f909cc..82d6b0b`

## Summary

**`min_line_index` is documented as an invariant of `HeadingConfig`, but it is applied only by
`Heading::resolve`, so `Heading::resolve_if_found` can still pick a header line as the heading.**

The commit makes `min_line_index` mean the right thing. It does not make it apply everywhere,
so the fix is only half effective.

`src/pager/heading.rs` (L197-201):

```rust
struct HeadingConfig {
    /// The minimum line index that can be a heading.
    /// Lines below this index are never treated as headings, regardless of pattern matching.
    min_line_index: usize,
```

The bound lives in the caller instead of in the search:

`src/pager/heading.rs` (L126-138):

```rust
    pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
        self.current = self.find_heading(doc, self.config.min_line_index..(line_index + 1));
    }

    /// Same as [`Self::resolve`] but keep the current heading when no heading is found.
    pub fn resolve_if_found(&mut self, doc: &mut Document, line_index_range: Range<usize>) {
        if let Some(heading) = self.find_heading(doc, line_index_range) {
            self.current = Some(heading);
        }
    }
```

`find_heading` scans `range` as given. `resolve_if_found` passes the caller's range straight
through, so `min_line_index` never applies.

## Why it still matters after this commit

**A capped header can put a header line inside the range `scroll_down` passes.**

`src/pager.rs` (L435-442):

```rust
    fn scroll_down(&mut self, num_rows: usize) -> usize {
        let prev_top_line = self.viewport.rows()[self.header.height()].line_index();
        let rows_scrolled = self.viewport.scroll_down(&mut self.doc, num_rows);
        let top_line = self.viewport.rows()[self.header.height()].line_index();

        // If a new heading exists within the moved range, replace the current one with it.
        self.heading
            .resolve_if_found(&mut self.doc, prev_top_line..(top_line + 1));
```

`prev_top_line` is the line under the header's *rows*, which is not the line under the header's
*lines* — the very distinction this commit introduces.

Concrete case:

- `--header-lines 3`, viewport height 3
- `build_rows` caps the header at `height - 1 = 2` rows, so `height() == 2`, `num_lines == 3`
- `viewport.rows()[2]` is line 2, still a header line, so `prev_top_line == 2`
- `min_line_index == 3`, but the range starts at 2, so line 2 can match and become the heading

The header line is then pinned twice: once as header, once as heading.

## Suggestion

**Move the bound into `find_heading`, where every search passes.**

```rust
fn find_heading(&self, doc: &mut Document, range: Range<usize>) -> Option<HeadingState> {
    let range = range.start.max(self.config.min_line_index)..range.end;
    ...
}
```

- `resolve` then just passes `0..(line_index + 1)`, and the config field stops leaking into
  the caller.
- The doc comment on `min_line_index` becomes true as written.
- Also worth rewording that comment: in a pager, "lines below this index" reads as
  "further down the page", i.e. the opposite of what it means. "Lines before this index"
  is unambiguous.

## Note

The open issue mentions this gap only as reproduction guidance:

> Note that `min_line_index` only gates `Heading::resolve`, not `Heading::resolve_if_found`,
> so plain downward scrolling still finds the heading.

It is used there to explain why the repro needs `G`. It is not raised as a problem to fix,
and no draft issue covers it.

## Assessment

Valid as a code-level fact, but the "concrete case" this file gives for why it still matters is
wrong — `dev/current/review/correctness-001-capped-header-direction-is-unreachable.md` catches
that, and I confirmed it by reading the code independently.

**The gap is real:**

- `find_heading` (`src/pager/heading.rs:141`) scans `range` exactly as given; it never touches
  `self.config.min_line_index`.
- `resolve` (`src/pager/heading.rs:130`) is the only caller that applies the bound, by folding it
  into the range it builds.
- `resolve_if_found` (`src/pager/heading.rs:135`) passes the caller's range straight through, and
  has exactly one call site: `scroll_down` (`src/pager.rs:435-442`).

**But the concrete case above (`--header-lines 3`, viewport height 3) cannot trigger it:**

- `Header::build_rows` (`src/pager/header.rs:46-49`) caps header rows at
  `size.height().saturating_sub(1)`. `HeadingConfig::max_heading_height`
  (`src/pager/heading.rs:211-215`) is `size.height() - global_header_height - 1`. Whenever the
  header is actually capped, `global_header_height == size.height() - 1`, which forces
  `max_heading_height` to exactly `0`.
- `find_heading` returns `None` outright when `max_heading_height == 0`
  (`src/pager/heading.rs:143-145`), before it ever looks at `range`. So in the capped scenario,
  no heading is found at all — the `min_line_index` check would never have run either way.
- The other reachable case, an uncapped (possibly wrapped) header, doesn't trigger it either:
  `viewport.rows()[header.height()]` in that case is always the first row of document line
  `num_lines` (i.e. `min_line_index`), because `Header`'s rows are built from exactly lines
  `0..num_lines`. So `prev_top_line` passed to `resolve_if_found` is always `>= min_line_index`
  already, and scrolling further down only increases it.

So today, with `resolve_if_found`'s one call site, the missing bound is unreachable — not a live
bug. It is a structural gap: nothing stops a *future* caller of `resolve_if_found` (or a change
to how `scroll_down` computes its range) from passing a range that starts before
`min_line_index`, and `find_heading` would silently honor it. The doc comment on
`min_line_index` ("Lines below this index are never treated as headings, regardless of pattern
matching") is stated as an unconditional invariant of `find_heading`'s search, but today it only
holds because of how the current single caller happens to construct its range.

## Plans

### Plan 1: Move the bound into `find_heading`

Apply `min_line_index` inside `find_heading` itself, so every caller gets it for free and the
doc comment becomes true as written, regardless of what future callers pass:

```rust
fn find_heading(&self, doc: &mut Document, range: Range<usize>) -> Option<HeadingState> {
    let range = range.start.max(self.config.min_line_index)..range.end;
    ...
}
```

Then simplify `resolve` to no longer fold the bound into its own range:

```rust
pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
    self.current = self.find_heading(doc, 0..(line_index + 1));
}
```

`resolve_if_found` needs no change; it now inherits the bound automatically. Also reword the
doc comment to avoid the "below this index" ambiguity flagged in the original review: "Lines
before this index are never treated as headings, regardless of pattern matching."

This is a small, local, behavior-preserving change (both current callers already produce ranges
that satisfy the bound). It's not fixing a live bug, but hardening: the invariant moves to the
one place (`find_heading`) all callers funnel through, instead of relying on every caller to
construct its range correctly by accident.

### Plan 2: Leave it, but drop the invariant claim from the doc comment

Reword `min_line_index`'s doc comment to describe what it actually guarantees today (`resolve`
bounds its own range by it) rather than claiming it as a `find_heading`-wide invariant. No code
change.

This avoids touching working code for a gap that isn't reachable, at the cost of a weaker,
more caller-dependent invariant that a future change could silently violate.

## Recommendation

Plan 1. The fix is a one-line, behavior-preserving change that makes a doc comment already
written as an unconditional invariant actually be one, and removes a foot-gun for any future
caller of `resolve_if_found` or `find_heading`. There's no meaningful maintenance cost to weigh
against that — it centralizes a check in the function that owns the search, which is the more
correct shape regardless of current reachability. Not urgent (no live bug), but cheap enough
that there's no reason to defer it to Plan 2's weaker documentation-only fix.
