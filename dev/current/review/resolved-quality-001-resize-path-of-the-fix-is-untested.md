# Resize Path of the Fix Is Untested

Review target: 7f909cc..306811d (`src/pager.rs`, `src/pager/heading.rs`, `src/tests/heading.rs`)

## Summary

**The fix touches two call sites, but only one of them is pinned by a test.**

`Heading` gained the new `global_header_num_lines` argument in both `new` and `resize`,
and `Pager` feeds it at both places:

`src/pager.rs` (L172, L313-320):

```rust
let mut heading = Heading::new(options.heading, &size, header.height(), header.num_lines());
```

```rust
fn relayout_page(&mut self, size: ViewportSize) {
    self.header.resize(&mut self.doc, &size);
    self.heading.resize(
        &mut self.doc,
        &size,
        self.header.height(),
        self.header.num_lines(),
    );
```

Only the `new` call site is covered. Verified by mutation — reintroducing the original bug
at each site and running `scripts/test`:

| Mutation | Result |
| --- | --- |
| `Heading::new(..., header.height(), header.height())` | `tests::heading::heading_just_below_wrapped_header_line` FAILED (296 passed, 1 failed) |
| `heading.resize(..., header.height(), header.height())` | **297 passed, 0 failed** |

### Why nothing catches it

- The `resize` unit test passes `0` for both units, so the two are indistinguishable there.

  `src/pager/heading.rs`:

  ```rust
  h.resize(&mut doc, &size(5, 5), 0, 0);
  ```

- `src/tests/heading.rs::heading_just_below_wrapped_header_line` starts at width 8, where the
  header line already wraps. It never resizes, so it only exercises `Heading::new`.
- `src/tests/heading_resize.rs` has no case where a width change makes a header line wrap
  while a heading sits below it. `resize_when_document_fits_entirely_in_header` uses
  `header: 3` with a document entirely inside the header, so no heading exists at any size.

### Why it matters more than usual here

The two arguments are adjacent bare `usize`s (see
`dev/issues/draft/heading-swappable-usize-header-metrics.md`), so the compiler cannot catch a
transposition either. On the `resize` path there is currently no signal at all — neither the
type system nor the suite.

### Suggested coverage

An integration test in `src/tests/heading_resize.rs`: start wide enough that the header line
fits on one row, shrink the width so it wraps into two, and assert the heading on the first
line below the header is still sticky. That is the `heading_just_below_wrapped_header_line`
shape reached through a resize instead of through startup.

## Assessment

- Newly introduced issue? Yes
- Does it block the overall goal? No, but it undermines confidence in the fix itself

Confirmed by reading the code (not just re-running the reviewer's claimed mutation):

- `src/pager/heading.rs` `resize_rebuilds_rows_at_new_width` calls
  `h.resize(&mut doc, &size(5, 5), 0, 0)` — both metrics are `0`, so a transposition of the two
  arguments is invisible to this test.
- `src/tests/heading.rs::heading_just_below_wrapped_header_line` starts at `screen_width: 8`,
  where `HEADERLINE!` already wraps into `HEADERLI>` / `NE!` from the very first frame. It never
  resizes, so it exercises only `Heading::new`, not `Heading::resize`.
- `src/tests/heading_resize.rs` has several resize tests, but `resize_when_document_fits_entirely_in_header`
  has no heading below the header at all, and `WRAP_CONTENT`-based tests wrap a *content* line,
  not the header line, and are `#[should_panic]` for an unrelated pre-existing bug
  (`dev/issues/draft/heading-state-not-recomputed-on-resize.md`).

So the reviewer's core claim holds: the fix changed both `Heading::new` and `Heading::resize`
call sites, but only the `new` path has a test that would fail if `header.height()` and
`header.num_lines()` were swapped or one were dropped at that call site. Since both arguments
are bare `usize`s with no newtype distinction (tracked separately in
`dev/issues/draft/heading-swappable-usize-header-metrics.md`), the type system gives no backstop
either. This is a real coverage gap in the same change being reviewed, not a pre-existing issue,
so it's worth closing now rather than deferring.

## Plans

### Plan 1: Add the suggested resize-path integration test (recommended)

Add a test to `src/tests/heading_resize.rs` shaped like `heading_just_below_wrapped_header_line`
but reached via `resize` instead of `new`:

```rust
/// Shrinking the width so the header line wraps should still keep the heading right
/// below it sticky. This exercises `Heading::resize`'s `global_header_num_lines` argument,
/// which `heading_just_below_wrapped_header_line` only exercises via `Heading::new`.
#[test]
fn resize_wraps_header_line_heading_stays_below_it() {
    let content = "\
HEADERLINE!
# A
b1
b2
b3
b4
b5
b6
b7
b8
";
    let screen = run_test_screen(TestCase {
        screen_width: 20, // fits "HEADERLINE!" on one row at first
        screen_height: 7,
        content,
        options: Options {
            header: 1,
            heading: heading_opts("^# "),
            ..Default::default()
        },
        events: vec![key('G'), resize(8, 7), key('q')],
        ..Default::default()
    });
    // After resizing to width 8, "HEADERLINE!" wraps into 2 rows and the header grows
    // from 1 to 2 num_lines; "# A" must still be recognized as the heading right below it.
    let want = "...";
    assert_eq!(screen.out(), want);
}
```

The exact `want` block needs to be produced by running the test once (or reasoning through
`rows::from_lines` wrapping) rather than hand-derived, since off-by-one row math is easy to get
wrong by inspection. The important assertion is that `# A` is shown pinned as the heading in the
post-resize frame, matching the shape `heading_just_below_wrapped_header_line` already checks for
the `new` path.

This directly targets the untested branch (`Heading::resize`'s two trailing arguments) with a
single new test, mirrors an existing test's shape so it is easy to review, and would fail today
if the two arguments at `src/pager.rs:317-320` were transposed or if `header.num_lines()` were
replaced by `header.height()`.

### Plan 2: Do not add a dedicated test now; rely on the swappable-usize issue

Defer to `dev/issues/draft/heading-swappable-usize-header-metrics.md` alone: once that issue
introduces distinct newtypes for `global_header_height` and `global_header_num_lines`, a
transposition becomes a compile error, making a dedicated regression test redundant for that
specific failure mode.

This is weaker than Plan 1 on its own: the newtype issue is still in draft (unscheduled), and
even with newtypes in place, a *test* still adds value beyond compile-time safety — it pins the
actual rendered behavior (that a heading below a newly-wrapped header line is still detected),
which a type-level fix alone does not guarantee if the underlying logic in `HeadingConfig::new`
or `is_heading_start` has some other bug.

## Recommendation

Plan 1. It is a small, self-contained addition (one test, modeled directly on an existing one),
it directly closes the coverage gap the reviewer identified in the changes under review, and it
does not depend on unscheduled work. The newtype issue in Plan 2 is still worth pursuing
separately as a structural safeguard, but it is not a substitute for pinning the actual resize
behavior with a test.
