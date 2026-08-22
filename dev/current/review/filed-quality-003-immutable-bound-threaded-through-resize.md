# `min_line_index` can never change, yet it is rebuilt on every resize

Review target: `7f909cc..316c110` (`src/pager/heading.rs`, `src/pager.rs`)

## Summary

**`Header::num_lines` is fixed for the whole life of the `Pager`, so `Heading::resize`'s new
`global_header_num_lines` argument can only ever be the value already passed to `Heading::new` —
a constant threaded through the one path where it looks like it varies.**

- `Header::num_lines` is set once in `Header::new` and never written again; `Header::resize`
  (`src/pager/header.rs:26-28`) only rebuilds `rows`.
- So `HeadingConfig` mixes two kinds of field with two different lifetimes:
  - `width`, `max_heading_height` — derived from `ViewportSize`, genuinely change on resize.
  - `min_line_index` — derived from the options, never changes.

## Why it matters

- **It doubles the resize signature for nothing.** `Heading::resize`
  (`src/pager/heading.rs:97-104`) takes four arguments where three would do:

  ```rust
      pub fn resize(
          &mut self,
          doc: &mut Document,
          size: &ViewportSize,
          global_header_height: usize,
          global_header_num_lines: usize,
      ) {
          self.config = HeadingConfig::new(size, global_header_height, global_header_num_lines);
  ```

- **It doubles the surface of the transposable-argument problem.** The pair flagged in
  `dev/issues/draft/heading-swappable-usize-header-metrics.md` appears at two call sites
  (`src/pager.rs:172` and `src/pager.rs:315-320`) instead of one. A value set once at
  construction can only be got wrong once.

- **It reads as if the bound could move on resize.** It cannot; a resize changes how many rows
  the header renders as, never which lines it covers. That is exactly the distinction the fix is
  about, so keeping the two in one struct that is rebuilt as a unit blurs it again.

## Possible direction

Hold the bound as a plain field on `Heading`, set in `new`, and leave `HeadingConfig` to the
size-derived values only.

Note this becomes moot if `dev/current/review/quality-001-heading-owns-a-copy-of-header-layout.md`
is taken — there `min_line_index` disappears from `Heading` entirely.

---

## Assessment

- Newly introduced issue? Yes
- Does it block the overall goal? No

**The observation is factually correct and the mismatch is real, but it is a code-shape concern,
not a defect.**

- `Header::num_lines` is written once in `Header::new` and never again; `Header::resize`
  (`src/pager/header.rs:26-28`) only rebuilds `rows`. So at `src/pager.rs:315-320`
  `self.header.num_lines()` is provably the same value that `Pager::new` already passed to
  `Heading::new` at `src/pager.rs:172`.
- Rebuilding `min_line_index` from it on every resize is therefore a no-op that costs a
  parameter, and the parameter is the second half of the transposable `usize` pair recorded in
  `dev/issues/draft/heading-swappable-usize-header-metrics.md`. Two call sites can get the pair
  wrong instead of one.
- Behaviour is unaffected either way, so nothing here needs fixing before the branch lands.

**The one point to weigh is churn.** `dev/current/review/quality-001-heading-owns-a-copy-of-header-layout.md`
proposes removing `min_line_index` from `Heading` altogether (`resolve` takes a range, `Pager`
builds it from `self.header.num_lines()`). If that direction is taken, any restructuring done
here is deleted rather than built on. The restructuring is small enough that this is not an
argument against doing it — only against doing it *first*.

## Plans

### Plan 1: Hold the bound as a `Heading` field, keep `HeadingConfig` size-derived only

Split the struct along the two lifetimes the review identifies.

```rust
pub(super) struct Heading {
    /// The minimum line index that can be a heading. Fixed for the life of the `Pager`:
    /// the global header always covers the same document lines, however they are rendered.
    min_line_index: usize,
    config: HeadingConfig,
    options: Option<HeadingOptions>,
    current: Option<HeadingState>,
}

impl Heading {
    pub fn new(
        options: Option<HeadingOptions>,
        size: &ViewportSize,
        global_header_height: usize,
        global_header_num_lines: usize,
    ) -> Self {
        Self {
            min_line_index: global_header_num_lines,
            config: HeadingConfig::new(size, global_header_height),
            options,
            current: None,
        }
    }

    pub fn resize(&mut self, doc: &mut Document, size: &ViewportSize, global_header_height: usize) {
        self.config = HeadingConfig::new(size, global_header_height);
        // ...unchanged...
    }

    pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
        self.current = self.find_heading(doc, self.min_line_index..(line_index + 1));
    }
}
```

`HeadingConfig` loses `min_line_index` and keeps `max_heading_height` / `width`.
`Pager::relayout_page` (`src/pager.rs:315-320`) drops its fourth argument:

```rust
        self.heading
            .resize(&mut self.doc, &size, self.header.height());
```

Test updates are mechanical: `resize_rebuilds_rows_at_new_width` drops its trailing `0`.
Cost: roughly a dozen lines. It removes the transposable pair from one of the two call sites and
makes the "a resize changes rows, never which lines the header covers" distinction visible in
the type.

### Plan 2: Fold this into the draft issue as a fourth candidate direction

Do not change code on this branch. Instead add Plan 1 to the **Plan** section of
`dev/issues/draft/heading-swappable-usize-header-metrics.md` as a candidate direction, alongside
the three already listed there (`&Header`, a named carrier struct, unit newtypes).

Something like:

```markdown
- **Drop the immutable bound from `resize`.** `Header::num_lines` never changes, so
  `Heading::resize`'s `global_header_num_lines` argument is always the value `Heading::new`
  already received. Hold it as a plain field on `Heading` and let `HeadingConfig` cover only the
  size-derived values. Independent of the three directions above and combinable with any of
  them: on its own it cuts the transposable pair from two call sites to one
  (`Heading::new`), which lowers the stakes of whichever direction is finally chosen.
```

Why this belongs in that issue's Plan rather than only as a recorded symptom:

- The three listed directions all answer "how should the two values be passed?". This one answers
  "should the second value be passed on resize at all?" — a different axis, currently missing.
- It is not exclusive with any of them, so it is a real option to combine, not a competitor.
- It also documents the interaction with
  `dev/current/review/quality-001-heading-owns-a-copy-of-header-layout.md`: if `Pager` ends up
  owning the search range, both the field and the argument disappear together and this candidate
  is moot.

Cost: an edit to one draft issue. The four-argument `resize` stays until that issue is worked on.

### Plan 3: Pass `&Header` to `new` and `resize`

```rust
    pub fn resize(&mut self, doc: &mut Document, size: &ViewportSize, header: &Header) {
        self.config = HeadingConfig::new(size, header);
        // ...
    }
```

This removes the transposable pair at both call sites at once, which is the stronger version of
what Plan 1 buys. It does *not* address this review's actual complaint: `min_line_index` would
still be recomputed on every resize, just from a different source. It also borrows `&self.header`
and `&mut self.doc` simultaneously in `relayout_page`, which is fine today but couples `Heading`
harder to `Header` — the opposite of quality-001's direction. Listed for completeness; not
recommended on its own.

## Recommendation

**Plan 2 — do not change code on this branch; add Plan 1 to
`dev/issues/draft/heading-swappable-usize-header-metrics.md` as a fourth candidate direction.**

- The issue is cosmetic: no behaviour differs, and the redundant argument cannot produce a wrong
  value on its own — it is the same constant every time.
- The real cost the review names, the transposable `usize` pair, is already owned by that draft
  issue. Fixing half of it here on a separate branch would leave the issue open anyway and split
  one decision across two changes.
- Three of the four open concerns on `Heading` touch exactly these fields, and quality-001 would
  delete them outright. Restructuring now risks doing it twice.
- Recording Plan 1 as a candidate rather than a symptom keeps it actionable: it is the only
  direction on that issue's list that reduces the number of call sites rather than changing how
  the values are spelled, and it composes with all three of the others.

Plan 1 is cheap and correct, so if the coupling decision stalls well past this branch, take it
then — it is independent of every direction under discussion and trivially undone by whichever
one wins.

## Filed as Issue

`dev/issues/draft/heading-swappable-usize-header-metrics.md`

Added as a fourth candidate direction in that issue's `## Plan` section, on a different axis from
the three already listed and combinable with any of them. No code change on this branch.
