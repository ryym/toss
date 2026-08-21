# The capped-header direction of the bug is unreachable, so its new test guards nothing

Review target: `7f909cc..82d6b0b`

## Summary

**Only one of the two divergence directions the fix targets can actually happen: header lines
wrapping. The capped-header direction is unreachable, because a capped header forces
`max_heading_height` to `0` and `find_heading` then bails before any line is considered.**

The fix itself is correct. What is wrong is the reachability claim it is built on, and that
leaks into the new unit test and into `dev/current/review/quality-002-...`.

## Why capping implies `max_heading_height == 0`

**`Header` caps at `viewport height - 1` rows, and `HeadingConfig` subtracts exactly that.**

`src/pager/header.rs` (L46-50):

```rust
fn build_rows(doc: &mut Document, size: &ViewportSize, num_lines: usize) -> Vec<Row> {
    // Reserve at least one non-header row so the header does not cover the entire viewport.
    let max_height = size.height().saturating_sub(1);
    rows::from_lines(doc, size.width(), 0..num_lines, max_height)
}
```

`rows::from_lines` ends with `rows.truncate(max_rows)`, so whenever the cap bites,
`height() == size.height() - 1` exactly.

`src/pager/heading.rs` (L211-215):

```rust
        // Reserve at least one non-heading row so the heading does not cover the entire viewport.
        let max_heading_height = size
            .height()
            .saturating_sub(global_header_height)
            .saturating_sub(1);
```

- `height() == size.height() - 1` gives `max_heading_height == 0`.
- `find_heading` (`src/pager/heading.rs:143-145`) returns `None` on `max_heading_height == 0`.
- So while the header is capped, **no heading exists at all** — a capped-away header line
  cannot be picked as one, with either the old or the new `min_line_index`.

## The only other way `height() < num_lines`

**A document shorter than `num_lines`** — `rows::from_lines` skips missing lines, so
`height() < num_lines` without the cap.

- Those lines do not exist, so they cannot match the heading pattern.
- Streaming does not create a lasting gap: `Pager::pump_input` calls `relayout_page` (and thus
  `Header::resize`) on every pump while the viewport is under-filled.
- No observable difference between `height()` and `num_lines()` here either.

**Net: `min_line_index` only ever changed behaviour in the wrapping direction**, which is what
`tests::heading::heading_just_below_wrapped_header_line` covers.

## Consequence 1: the new cap test states an impossible configuration

`src/pager/heading.rs` (L378-386):

```rust
    #[test]
    fn min_line_index_covers_header_lines_dropped_by_the_height_cap() {
        let mut doc = Document::from_string("# A\n# B\n# C\nx\n".into());
        // The header covers 3 lines but only 2 rows of it fit, so line 2 is still not a heading.
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 3);

        h.resolve(&mut doc, 3);
        assert!(h.start_line_index().is_none());
    }
```

- `(height = 2, num_lines = 3)` requires `size.height() == 3`, not `10`.
- With a real `size(10, 3)`, `max_heading_height` is `0` and the assertion holds for a reason
  that has nothing to do with `min_line_index`.
- The comment — "only 2 rows of it fit" — describes a `Header` this `size` cannot produce.

Either drop the test, or keep it and say plainly that it pins `HeadingConfig` arithmetic against
a synthetic input rather than a reachable layout.

## Consequence 2: the repro in `quality-002` does not reproduce

That file argues `resolve_if_found` can pick a header line, with this case:

> - `--header-lines 3`, viewport height 3
> - `build_rows` caps the header at `height - 1 = 2` rows, so `height() == 2`, `num_lines == 3`
> - `viewport.rows()[2]` is line 2, still a header line, so `prev_top_line == 2`

`max_heading_height` is `0` there, so `find_heading` returns `None` before the range is scanned.
The gap it describes is real as a **latent** one, but it has no reachable trigger today:

- **Capped header** — no heading at all, as shown above.
- **Uncapped header** — at the document top, `viewport.rows()[header.height()]` is the first row
  of line `num_lines`, so `prev_top_line >= min_line_index`. Scrolling only raises it.

Worth correcting there so the gap is not fixed under a false urgency.

## Open question

**Should the open issue's second symptom be retracted?**

`dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md` lists it as a live
divergence:

> - **Header is capped** (short terminal): `build_rows` caps the header at `viewport height - 1`
>   rows, so `height() < num_lines`, and a line that _is_ part of the header can be picked as a
>   heading.

`dev/issues/draft/heading-state-not-recomputed-on-resize.md` already records the opposite in its
`top_line` discussion:

> Note that `max_heading_height` is `0` whenever the global header is capped
> (`header.height() == viewport height - 1`), and `find_heading` returns `None` on a
> `max_heading_height` of 0.

The two documents contradict each other. The draft is the correct one.

## Assessment

Verified directly against the source:

- `global_header_height` passed into `HeadingConfig::new` is always `self.header.height()`
  (`src/pager.rs:172`, `src/pager.rs:318`), i.e. the capped row count, not the line count.
- Whenever that cap bites, `height() == size.height() - 1`, so `max_heading_height` is
  `0`, and `find_heading` (`src/pager/heading.rs:143-145`) short-circuits to `None` before
  scanning anything.
- The new test `min_line_index_covers_header_lines_dropped_by_the_height_cap` constructs
  `Heading::new(.., &size(10, 10), 2, 3)` — `global_header_height` (2) and `size.height()` (10)
  are unrelated in this call, so `max_heading_height` is `10 - 2 - 1 = 7`, not `0`. The test
  never exercises the capped branch at all; it passes only because line 3 (`x`) does not match
  the heading pattern, same mechanism as `min_line_index_excludes_global_header_area` just above
  it. Its comment describes a `(height=2, num_lines=3)` pairing that only a real
  `size.height() == 3` viewport could produce, which this test does not construct.

The reachability claim in the review is correct: the fix's row-vs-line distinction only ever
changes behavior in the wrapping direction. This is a documentation/test-accuracy issue, not a
code defect — the fix at 82d6b0b is correct as shipped.

One refinement to the review's framing: the test is not *worthless* — it does pin that
`min_line_index` is bounded by `global_header_num_lines` (3) rather than `global_header_height`
(2), which is exactly what the fix changed. What's wrong is only the comment/name claiming this
demonstrates the height-cap scenario specifically; structurally it duplicates
`min_line_index_excludes_global_header_area` with a different line/height split.

## Plans

### Plan 1: Rename and re-comment the test; correct the two docs (recommended)

- In `src/pager/heading.rs`, rename `min_line_index_covers_header_lines_dropped_by_the_height_cap`
  to something like `min_line_index_uses_header_line_count_even_when_it_exceeds_row_count`, and
  replace its comment to state plainly that `global_header_height` and `global_header_num_lines`
  are passed independently here to pin the arithmetic, not to model a reachable capped-header
  layout (capping in practice drives `max_heading_height` to `0`, which `find_heading` already
  short-circuits on, covered elsewhere).
- In `dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md`: replace the
  "Header is capped" bullet in the Overview's divergence list with a short investigation note —
  not just a deletion — recording that this direction does not currently occur: whenever the
  header is capped, `build_rows` forces `height() == size.height() - 1`, which drives
  `max_heading_height` to `0`, and `find_heading` returns `None` before any line is considered, so
  no header line can ever be picked as a heading through this path today. Also drop the
  corresponding "capped header" bullet from the Plan's Tests section, since it would test a
  code path that cannot execute.
- `quality-002-min-line-index-not-enforced-in-find-heading.md` is out of scope here — it is being
  handled in a separate session. Worth a passing mention there (its `scroll_down` repro's
  capped-header half doesn't reproduce for the same `max_heading_height == 0` reason), but no edit
  to that file as part of this plan.

This keeps the open issue and the shipped test internally consistent and prevents future work
from chasing an unreachable branch, at the cost of touching two files with no production-code
change.

### Plan 2: Leave as is

Treat this as internal review-trail noise: the shipped fix is correct, and the test still passes
and still pins real (if narrowly-scoped) behavior. Do not edit `dev/issues/open/...` since it is
a historical review artifact, not a living spec.

This avoids any edit cost but leaves a contradiction on record between the open issue and the
draft issue, and leaves the test's comment asserting something false about reachability, which
could mislead whoever next touches `heading.rs`.

## Recommendation

**Plan 1.** The fix itself needs no changes, but the false reachability claim is inconsistent
between the merged commit's test and the open issue, and the latter (`dev/issues/open/...`) is
still marked `status: doing` — i.e. still treated as live guidance for follow-up work, not
archived history. Left uncorrected, it risks someone later
adding "capped header" test coverage or a repro for a branch that cannot execute. The edits are
small, purely textual, and carry no maintenance burden going forward.
