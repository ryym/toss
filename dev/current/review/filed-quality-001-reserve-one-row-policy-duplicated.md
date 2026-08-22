# The reserve-one-row policy is duplicated, and the fix silently depends on the two copies matching

Review target: `7f909cc..a4a1ae3`

## Summary

**`min_line_index = header.num_lines()` is only safe in the capped-header case because
`Header` and `HeadingConfig` happen to reserve the same one row. Nothing in the code states
or enforces that link, and the new test explicitly declines to pin it.**

- The fix is right for the wrapping case, which is the reported bug.
- For the capped case it leans on a cross-module coincidence documented only in `dev/issues/`.

## The two copies

Both modules implement "reserve at least one row", with near-identical comments.

`src/pager/header.rs` (L45-50):

```rust
fn build_rows(doc: &mut Document, size: &ViewportSize, num_lines: usize) -> Vec<Row> {
    // Reserve at least one non-header row so the header does not cover the entire viewport.
    let max_height = size.height().saturating_sub(1);
    rows::from_lines(doc, size.width(), 0..num_lines, max_height)
}
```

`src/pager/heading.rs` (L211-215):

```rust
        // Reserve at least one non-heading row so the heading does not cover the entire viewport.
        let max_heading_height = size
            .height()
            .saturating_sub(global_header_height)
            .saturating_sub(1);
```

## Why the fix depends on them agreeing

- **Capped header** — `rows::from_lines` breaks and truncates at `max_rows`, so
  `header.height() == size.height() - 1` exactly whenever the cap bites.
- **That forces `max_heading_height` to `0`** — `size.height() - (size.height() - 1) - 1`.
- **`find_heading` bails on `0`** (`src/pager/heading.rs:142-145`), so no heading exists while
  the header is capped.
- **Therefore `min_line_index` never matters there**, whether it is `height()` or `num_lines()`.

Change either reserve — e.g. `Header` reserves 2 rows — and `max_heading_height` is no longer
`0` under a cap. The capped case becomes reachable, and `min_line_index = num_lines` then
excludes header lines that are configured but never rendered. Whether that is desired is a
behaviour decision nobody has made.

## The new test opts out of pinning it

`src/pager/heading.rs` (L378-386):

```rust
    #[test]
    fn unreachable_capped_header_still_uses_header_line_count() {
        let mut doc = Document::from_string("# A\n# B\n# C\nx\n".into());
        // height (2) < num_lines (3) only to pin the arithmetic, not to model a capped header:
        // that would force max_heading_height to 0, which find_heading short-circuits on first.
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 3);

        h.resolve(&mut doc, 3);
        assert!(h.start_line_index().is_none());
    }
```

- The comment records the coupling but the assertion does not test it.
- `size(10, 10)` with `height = 2` is a shape no real `Header` can produce.
- Break `build_rows`' reserve and this test still passes.

## Open question

**Should `Heading` own a "reserve one row" rule at all?**

- `Heading` needs two numbers: the first line it may pick, and how many rows it may use.
- It currently re-derives both from header metrics, re-implementing a layout policy that
  `Header` (and `Pager`, via `total_header_height`) also knows about.
- An alternative is for `Pager` to compute the row budget once and hand `Heading` the result,
  leaving `Heading` unaware of the global header entirely.
- Note this pulls the opposite way from the leading candidate in
  `dev/issues/draft/heading-swappable-usize-header-metrics.md` ("pass `&Header`"), which
  tightens the `Heading` -> `Header` coupling instead of removing it. Worth deciding both
  together.

## Assessment

- Newly introduced issue? Yes, in effect.
- Does it block the overall goal? No.

The duplicated "reserve one row" comment/logic itself predates this diff (both copies existed
before `7f909cc`). What's new is that `min_line_index` switched from `global_header_height`
(rows) to `global_header_num_lines` (lines). That's the correct fix for the reported wrapping
bug, and I verified the capped-header reasoning holds: `Header::build_rows` caps rows at
`size.height() - 1`, so whenever the cap actually bites, `header.height() == size.height() - 1`
exactly, which forces `max_heading_height` to `0` and `find_heading` bails before
`min_line_index` is ever consulted. So today's behavior is correct — this is a latent coupling,
not a live bug.

I also worked through whether the two reserved-row constants need to match at all. They don't
serve the same purpose: `Header`'s reserve keeps a header-only viewport from covering every row;
`Heading`'s reserve keeps `header + heading` from covering every row, and that second guarantee
only depends on `Heading`'s own constant, not on `Header`'s. So diverging the two constants would
not break the general "leave one row for content" invariant. What it *would* do is make the
capped-header case reachable (`max_heading_height` becomes nonzero while the header is still
truncating configured lines), a combination nobody has designed or tested — matching what the
review calls "a behaviour decision nobody has made."

Given the fix is correct today and the real question is architectural (should `Heading`
re-derive header-derived numbers at all), this fits the "file an issue instead of fixing now"
case: it doesn't block the goal, and a code fix now would either be a documentation-only patch
(doesn't remove the coupling) or a `Heading`/`Header` API change that overlaps with the
already-open `heading-swappable-usize-header-metrics.md` decision — better decided once, not
piecemeal.

## Plans

### Plan 1: Tighten the existing test to pin the coupling, and cross-reference the two reserves in comments (recommended)

Small, local, no architecture decision required.

- Rewrite `unreachable_capped_header_still_uses_header_line_count` to construct a *real* capped
  header via `Header::new` (or drive `Heading` through `Pager`-level setup) instead of a
  fabricated `(height, num_lines)` pair, so the test actually exercises `Header` and `Heading`
  agreeing, rather than asserting on a shape no real `Header` produces.
- If constructing a real capped header from `heading.rs`'s test module is awkward (e.g. it would
  need to reach into `Header`, a sibling private module), move the test to a shared/integration
  test that has both, or at minimum add a `debug_assert!`/comment in `HeadingConfig::new` that
  explicitly says its `- 1` must match `Header::build_rows`'s `- 1` for the capped case to stay
  unreachable, with a pointer to this file's reasoning.
- This doesn't remove the coupling, but it stops the next change to either reserve from silently
  making the capped case reachable without anyone noticing.

### Plan 2: Fold the "should Heading own this policy" question into the existing draft issue, fix nothing now

- Append this review's "Open question" section (and the reachability analysis above) to
  `dev/issues/draft/heading-swappable-usize-header-metrics.md`, since its "pass `&Header`"
  candidate already touches the same surface and the two should be decided together rather than
  separately.
- No code change in this diff.

### Plan 3: Have `Pager` compute the shared row budget once and hand `Heading` a single "rows available" number, removing `Heading`'s own `- 1`

- Larger change: `Pager` would compute `max_heading_height` itself (it already knows both
  `Header`'s row height and line count via `total_header_height`), and `Heading::new`/`resize`
  would take that number directly instead of re-deriving it from `global_header_height`.
- Directly resolves the review's "Open question" by removing the duplicate policy, but changes
  the `Heading`/`Pager` API boundary and overlaps with (and partly conflicts with) the
  "pass `&Header`" candidate in the draft issue — should not be done independently of that
  decision.

## Recommendation

**Plan 1**, and defer the architectural question via **Plan 2**.

The fix in this diff is correct and nothing here blocks it. Plan 3 is the "real" fix for the
underlying duplication, but it's an architecture decision that already has an open, related issue
tracking it (`heading-swappable-usize-header-metrics.md`) — deciding it here in isolation risks
picking a shape that gets reworked again once that issue is resolved. Plan 1 is cheap, has no
design trade-off, and closes the immediate gap (a test that claims to pin an invariant but
doesn't).

## Filed as Issue

Follow-up investigation with the user found that the reserve-one-row coincidence is one
manifestation of a broader, already-live gap: `Header::contains()`/`num_lines()` is used
elsewhere (`jump_to`) as if it meant "rendered as header", which is false whenever the header
is capped — independent of whether the two reserves match. That existing bug and this review's
divergence risk share the same root cause, so they were filed together as a single issue rather
than following Plan 1/Plan 2 separately:

`dev/issues/draft/capped-header-lines-become-unreachable.md`
