# Header Boundary API Lacks a Rule for Choosing Between Its Accessors

Review target: 7f909cc..306811d (`src/pager/header.rs`)

## Summary

**`Header` now offers three answers to "where does the header end?", and the diff documents
two of them without saying which one a caller should reach for.**

The bug being fixed _was_ picking the wrong one of these. The new docs explain how they
differ, but not how to choose — so the next caller is left making the same judgement call
from scratch.

`src/pager/header.rs` (L30-45):

```rust
    /// The number of screen rows the header covers. Unlike [`Self::num_lines`], this counts
    /// rendered rows: larger than `num_lines` when header lines wrap, smaller when the header
    /// is capped to fit the viewport or the document has fewer lines than configured.
    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// The number of leading document lines configured as the header.
    /// This is a configured extent, not the number of lines actually rendered.
    pub fn num_lines(&self) -> usize {
        self.num_lines
    }

    pub fn contains(&self, line_index: usize) -> bool {
        line_index < self.num_lines
    }
```

### Two gaps

- **`contains` is the only one left undocumented**, and it is the one with the surprising
  behavior: it returns `true` for lines the cap dropped, which never render as header rows.
  That surprise is the root of
  `dev/issues/draft/capped-header-lines-become-unreachable.md`. The diff documented the two
  accessors whose meaning is comparatively obvious and skipped the one that needs it.
- **No stated rule ties a unit to a question.** The rule the codebase actually follows is
  simple and worth writing down on the type:

  | Question                                         | Use                          |
  | ------------------------------------------------ | ---------------------------- |
  | How much screen space / which viewport row index | `height()`                   |
  | Is this document line part of the header         | `contains()` / `num_lines()` |

  `Pager` follows it consistently today — `viewport.rows()[self.header.height()]`,
  `.skip(self.header.height())`, `self.header.contains(line_index)` — but only by
  convention.

### Relation to existing issues

`dev/issues/draft/heading-swappable-usize-header-metrics.md` attacks the same bug class
structurally (`&Header`, a carrier struct, newtypes). This is the cheap complement, not a
substitute: whichever direction is chosen there, `Header` still exposes these three members
and still owes callers a rule.

## Assessment

- Newly introduced issue? Yes — the reviewed diff (commit `444d567`) is exactly what created
  the asymmetry: it added rustdoc to `height()` and `num_lines()` but left `contains()`, the
  one accessor with surprising cap behavior, untouched.
- Does it block the overall goal? No — it's a documentation gap, not a functional bug. But
  it's cheap to close and directly caused by this diff, so it belongs in this round rather
  than a filed issue (the "existed before the reviewed changes" exemption doesn't apply here;
  the missing-rule *shape* of the gap is new, even though `contains()` itself predates the diff).

The reviewer's structural point holds up against the code: `contains()` is the only member of
the trio without a doc comment, and its behavior (`true` for capped-out lines that never
render) is the one most likely to surprise a caller. `dev/issues/draft/heading-swappable-usize-header-metrics.md`
independently confirms callers already have to re-derive this same boundary in `heading.rs`,
which is more evidence the rule needs to live somewhere callers can find it, not just be
followed by convention in `pager.rs`.

## Plans

### Plan 1: Document `contains()`, deferring to `num_lines()` for detail (recommended)

Add a short doc comment to `contains()` that names which of the two questions it answers and
points at `num_lines()` for the caveat that's already spelled out there, instead of restating
it:

```rust
/// Whether `line_index` is a document line configured as part of the header.
/// Uses the configured extent, not the rendered one — see [`Self::num_lines`].
pub fn contains(&self, line_index: usize) -> bool {
    line_index < self.num_lines
}
```

Scope: one doc comment, no behavior change. Closes the specific gap the review flags
(`contains()` being the only undocumented member) without adding a struct-level rule or an
issue cross-reference — both `num_lines()`'s existing doc comment and
`dev/issues/draft/heading-swappable-usize-header-metrics.md`'s "Related Concern" section
already cover that ground.

### Plan 2: Do not address now, fold into the structural issue

Treat this purely as documentation debt and leave it to
`dev/issues/draft/heading-swappable-usize-header-metrics.md`, whose "Related Concern" section
already discusses `Header::contains`'s duplicated boundary and proposes directions that would
change or remove the method's shape entirely. Documenting a method whose signature may not
survive that redesign risks needing another pass shortly after.

Weigh against it: that issue is filed as `type: maintenance` with an undecided plan and no
urgency tied to it, so the doc gap could sit for a while with no compiler or test signal
pointing back at it — unlike a code bug, nothing forces someone to revisit it. The fix in
Plan 1 is small enough that it's not meaningfully wasted work even if the structural issue
later renames or removes `contains()`.

## Recommendation

Plan 1. It's a doc-only change, costs nothing to redo if the structural issue later reshapes
`Header`, and it closes the exact gap the review points at instead of leaving it to an
undecided, unscheduled maintenance issue.
