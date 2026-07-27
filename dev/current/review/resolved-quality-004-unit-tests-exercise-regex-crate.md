# Three of the four new `search.rs` unit tests exercise the `regex` crate, not `search_document`

Review target: regex syntax coverage added to the search engine tests

- src/search.rs:361-408 (`character_class_matches`, `quantifier_matches`, `anchor_matches_line_start`)
- src/search.rs:410-422 (`zero_width_match_does_not_hang`)

## Summary

`search_document` already took a compiled `regex::Regex` before this work; the overview states
this explicitly ("the search engine internals ... already work in terms of `regex::Regex` ...
without changes"). So `[bc]+`, `a{3}` and `^a` are not new capabilities of `search.rs` — they
were always supported, and these three tests assert that the `regex` crate matches character
classes, quantifiers and anchors correctly. They exercise no toss logic that the existing
literal-pattern tests do not already cover, and they will never fail for a reason that is
toss's fault.

That is maintenance cost with no defect-detection value: three more tests to update on any
change to `make_doc` / `pos` / the `search_document` signature, and three more results to read
past when the suite fails for a real reason.

`zero_width_match_does_not_hang` is a different matter and worth keeping: `a*` is the first
pattern that can produce an empty match, and whether `search_document`'s own iteration
terminates is genuinely toss's responsibility rather than the crate's. Its comment states the
contract it depends on ("the regex crate advances by one byte on empty matches"), which is the
right level to test at.

One gap this leaves, if the intent was to lock down the newly reachable pattern space: nothing
here covers a pattern that spans behaviour `search.rs` actually owns, such as a match at the
very end of a line, or an anchor interacting with the row/wrap boundary logic. Those would be
more valuable than the three syntax tests. The equivalent question for the backward direction
(`?` search over a zero-width or anchored pattern) is also untested in both this file and
`src/tests/search_regex.rs`.

## Assessment

Confirmed against the code (`src/search.rs:361-404` and `src/tests/search_regex.rs`). Verified:

- `search_document` only takes an already-compiled `Regex`; nothing in `character_class_matches`,
  `quantifier_matches`, or `anchor_matches_line_start` differs mechanically from the existing
  literal-pattern tests in the same suite — they'd catch a `regex` crate bug, never a toss bug.
- `zero_width_match_does_not_hang` genuinely tests iteration logic `search_document` owns (the
  by-one-byte advance on empty matches), so it earns its keep.
- The gap is real: every regex-flavored test in both `src/search.rs` and
  `src/tests/search_regex.rs` only drives `SearchDirection::Forward`. There is no coverage of a
  zero-width or anchored pattern searched backward, and no test where a match sits at a line's
  last byte or crosses a wrap-row boundary.

Agree with the review: the three syntax tests are low-value churn, and the backward/boundary gap
is the more useful thing to spend test-writing effort on.

## Plans

### Plan 1: Delete the three regex-syntax tests, keep the rest as-is

Remove `character_class_matches`, `quantifier_matches`, and `anchor_matches_line_start`
(`src/search.rs:361-404`). Keep `zero_width_match_does_not_hang` untouched. Do not add new
tests — accept the coverage gap the review notes as a separate, unscheduled concern.

Cheapest option; removes the maintenance liability but leaves the boundary/backward gap open.

### Plan 2: Delete the three regex-syntax tests and add tests for the actual gap

Same deletion as Plan 1, plus two additions to `src/search.rs`'s test module:

- A backward-direction test mirroring `zero_width_match_does_not_hang`, e.g. search backward
  with `Regex::new("a*")` (or an anchored pattern like `^a`) from a position after the match, to
  confirm `SearchDirection::Backward` also terminates and lands on the right span.
- A boundary test where the match sits at the very end of a line (or spans a wrap-row boundary
  via `.wrap(n)`, following the pattern already used in
  `document_row_forward_skips_matches_before_row`), to confirm `search_document`'s own
  row/line-end bookkeeping — not just the regex engine — handles it.

This directly replaces low-value coverage with coverage of logic `search.rs` actually owns,
matching what the review flagged as missing.

## Recommendation

Plan 2. The deletion part is uncontroversial cleanup either way, and given the review already
did the work of identifying exactly which untested paths are toss's own responsibility
(backward direction, line/wrap boundary), writing those two tests now is a small, well-scoped
addition — not scope creep — while the reasoning is fresh. Plan 1 alone would just shrink the
suite without closing the gap that motivated writing these tests in the first place.
