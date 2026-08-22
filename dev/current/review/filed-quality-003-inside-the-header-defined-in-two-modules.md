# "Inside the global header" is now defined in two modules under two names

Review target: `7f909cc..134a6c4` (`src/pager/header.rs`, `src/pager/heading.rs`)

## Summary

**After the fix, `Header::contains` and `HeadingConfig::min_line_index` are the same predicate
over the same value, expressed twice.**

- `Header` owns the notion of "this line belongs to the header".
- `Heading` now copies the number behind it and re-derives that notion itself.

## The two copies

`src/pager/header.rs` (L42-44):

```rust
    pub fn contains(&self, line_index: usize) -> bool {
        line_index < self.num_lines
    }
```

`src/pager/heading.rs`, `HeadingConfig::new` (L206-222):

```rust
        Self {
            // Header lines are never heading candidates, regardless of how many rows they
            // render as.
            min_line_index: global_header_num_lines,
```

`min_line_index` is used as `self.config.min_line_index..(line_index + 1)` in `resolve`, so
"excluded from the search" means exactly `line_index < num_lines` — `Header::contains` spelled
differently.

## Why it is worth naming

Before this change the duplication was hidden by the unit mismatch: `Heading` held a row count,
so it was visibly *not* the same thing as `Header::contains`. The fix makes the two agree, which
is correct, and in doing so turns an accidental difference into an unmarked copy.

- **The bug being fixed was exactly this.** `Heading` re-derived "inside the header" from a value
  that did not mean that. The fix corrects the value but keeps the re-derivation.
- **Nothing links the two.** A future change to what `Header::contains` means (the capped-header
  work in `dev/issues/draft/capped-header-lines-become-unreachable.md` proposes changing it to
  reflect rendered rows) would have to remember to change `HeadingConfig` too, with no compiler
  or test signal.

## Relation to existing issues

`dev/issues/draft/heading-swappable-usize-header-metrics.md` covers the *transposable argument
pair*, and its "pass `&Header`" option would remove the swap hazard while keeping this
duplication — `HeadingConfig::new` would still read `header.num_lines()` and rebuild the
predicate.

The point here is narrower and separate: whichever direction that issue takes, consider having
`Heading` ask `Header` the question (or take the header's line range as one value) instead of
holding a threshold it interprets on its own.

## Assessment

- Newly introduced issue? No (sharpened by the change, not created by it)
- Does it block the overall goal? No

The observation is accurate: after the fix, `min_line_index` and `Header::contains` encode the
same boundary, and nothing in the code says so. But the shape it describes is not new — `Heading`
has always held a threshold handed to it as a bare `usize` by `Pager`. The fix changed *which*
number is handed over; it did not introduce the hand-over.

Two details narrow the practical risk and the shape of a good fix:

- **The two are not literally the same predicate.** `Header::contains` answers a boolean question
  (used once, in `Pager::jump_to`, `src/pager.rs:329`). `Heading` needs a *range start*:

  ```rust
  self.current = self.find_heading(doc, self.config.min_line_index..(line_index + 1));
  ```

  So "have `Heading` ask `Header` the question" does not fit as-is — `contains` is the wrong
  shape. What both actually depend on is one named boundary: *the first document line that is
  not part of the header*.

- **The real fix is already the subject of an open draft.**
  `dev/issues/draft/heading-swappable-usize-header-metrics.md` is deciding between passing
  `&Header`, a named carrier struct, and unit newtypes. Its `&Header` option does remove the
  duplication if `Header` exposes the boundary as one named value; the review is right that
  reading `header.num_lines()` and rebuilding the threshold would not.

So the feedback is valid as a *requirement to record*, not as a defect to patch mid-branch: fixing
it properly means choosing the same API direction that draft is already weighing.

## Plans

### Plan 1: Record the requirement in the existing draft issue, change nothing now

Extend `dev/issues/draft/heading-swappable-usize-header-metrics.md` so its "Outcome" section also
demands a single owner for the header/body boundary, e.g.:

```markdown
## Outcome

- No pair of adjacent `usize` parameters can be transposed without the compiler catching it.
- The "first document line outside the header" boundary is defined once, in `Header`.
  `Heading` must not re-derive it from a raw line count; `Header::contains` and the heading
  search's lower bound must both read the same named value.
```

Also note there that the `&Header` option only satisfies this if `Header` gains that named
accessor — `header.num_lines()` at the `HeadingConfig::new` call site leaves the copy in place.

No source change on this branch.

### Plan 2: Name the boundary on `Header` now, keep the `usize` parameter

A ~10-line change that removes the unmarked copy without touching the API shape the draft issue
is still deciding on.

`src/pager/header.rs`:

```rust
    /// The first document line that is not part of the header.
    /// Header lines are the range `0..body_start_line_index()`, regardless of how many
    /// screen rows they render as.
    pub fn body_start_line_index(&self) -> usize {
        self.num_lines
    }

    pub fn contains(&self, line_index: usize) -> bool {
        line_index < self.body_start_line_index()
    }
```

`src/pager.rs` passes `header.body_start_line_index()` where it now passes `header.num_lines()`,
and `HeadingConfig`'s field is renamed to match:

```rust
struct HeadingConfig {
    /// The first line index that can be a heading; supplied by `Header` as the start of
    /// the non-header region. Lines below it are never treated as headings.
    body_start_line_index: usize,
```

`Header::num_lines()` stays if other callers need it; otherwise it is replaced outright.

The transposable-`usize` hazard is untouched — that remains the draft issue's job.

### Plan 3: Pass `&Header` into `Heading` now

Do the draft issue's first option on this branch: `Heading::new` / `Heading::resize` take
`&Header` instead of the two `usize`s, and `HeadingConfig::new` reads
`header.height()` and `header.body_start_line_index()` (Plan 2's accessor) itself.

This resolves both this review and the draft issue at once, but it forces every `Heading` unit
test to construct a real `Header` (and a `Document` to build it from), which the draft issue
itself flags as work that may surface further cases — capped headers leaving no room for a
heading. That is a larger change than this branch's goal warrants.

## Recommendation

**Plan 1.**

The code is correct as it stands; what is missing is a link between two modules, and the decision
about how to express that link is exactly what the existing draft issue is holding open. Landing
Plan 2 now would introduce an accessor name and a config field name that the issue's chosen
direction (`&Header`, a carrier struct, or unit newtypes) may immediately rework — a second edit
to the same lines for no interim safety gain, since no bug is reachable today.

Plan 2 is the right fallback if the draft issue is expected to sit unresolved for a while: it is
cheap, purely additive, and compatible with all three directions the issue lists. Plan 3 is the
correct end state but belongs to that issue, not to this branch.

## Filed as Issue

`dev/issues/draft/heading-swappable-usize-header-metrics.md` (recorded as a related concern
alongside the transposable `usize` pair, since both are decided by the same API direction).
