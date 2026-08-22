# `min_line_index_excludes_global_header_area` does not exercise the exclusion

Review target: 7f909cc..134a6c4 (`src/pager/heading.rs`)

## Summary

**The unit test named after the `min_line_index` invariant passes even when `min_line_index`
is hardcoded to `0`, so it does not verify the invariant it is named for.**

This matters because `min_line_index` is exactly what commit `82d6b0b` changes, and this is
the only unit test in `heading.rs` whose name claims to cover the exclusion. The diff also
reworded its comment (`75a2f33`/`82d6b0b`) to assert a property the assertions never reach.

`src/pager/heading.rs` (L360-368):

```rust
#[test]
fn min_line_index_excludes_global_header_area() {
    let mut doc = Document::from_string("# A\n# B\n# C\nx\ny\n".into());
    // The header covers 2 lines, so lines 0 and 1 never become headings.
    let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 2);

    h.resolve(&mut doc, 4);
    assert_eq!(h.start_line_index(), Some(2));
}
```

- `resolve(4)` searches backward from line 4.
- Line 2 (`# C`) matches first, before the search can ever reach lines 0-1.
- So the result is `Some(2)` whether `min_line_index` is `2` or `0`. The bound is never
  consulted.

## Verification

Replacing `min_line_index: global_header_num_lines` with `min_line_index: 0` in
`HeadingConfig::new` and running `scripts/test`:

```
test pager::tests::heading_never_picks_a_line_inside_the_header_even_before_it_has_arrived ... FAILED
test tests::heading::heading_overlaps_fixed_header ... FAILED
test result: FAILED. 295 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

`min_line_index_excludes_global_header_area` is among the 295 that still pass.

(For contrast, the mutation this diff actually fixes — `min_line_index: global_header_height`
— is caught, by `min_line_index_ignores_wrap_rows_of_the_global_header` and
`tests::heading::heading_just_below_wrapped_header_line`. The gap is specific to the
*exclusion* direction.)

## Suggested fix

Make the nearest match fall inside the header, so the bound is what decides the outcome.

```rust
#[test]
fn min_line_index_excludes_global_header_area() {
    let mut doc = Document::from_string("# A\n# B\nx\ny\n".into());
    // The header covers 2 lines, so lines 0 and 1 never become headings.
    let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 2);

    h.resolve(&mut doc, 3);
    // Without the bound, `# B` on line 1 would be picked.
    assert!(h.start_line_index().is_none());
}
```

## Assessment

- Newly introduced issue? No (pre-existing; the reviewed range only touched its arguments and comment)
- Does it block the overall goal? No

The factual claim is correct, and I reproduced it. With `min_line_index: 0` forced in
`HeadingConfig::new`, `scripts/test` gives `295 passed; 2 failed`, and
`pager::heading::tests::min_line_index_excludes_global_header_area ... ok` is among the
passing ones. The reason is exactly as written: `find_heading` scans
`(range.start..range.end).rev()`, so `resolve(&mut doc, 4)` hits `# C` on line 2 before the
`range.start` bound could ever matter.

### But it is a test-quality issue, not a coverage hole

The invariant itself is not untested. Two higher-level tests do fail under the mutation:

- `src/pager.rs:965` `heading_never_picks_a_line_inside_the_header_even_before_it_has_arrived`
- `src/tests/heading.rs` `heading_overlaps_fixed_header`

So there is no real risk of a silent regression. What is wrong is narrower: a unit test whose
name and comment promise to guard the exclusion actually asserts nothing about it. That is
still worth fixing — a future reader trusts the name, and the reviewed range (`75a2f33`,
`82d6b0b`) just reworded its comment to restate a property the assertions never reach.

### The suggested fix is the only real shape

Any test that discriminates on `min_line_index` must have its *nearest* match inside the
header — otherwise the reverse scan returns before the bound is consulted. So the reviewer's
document shape is essentially forced, not one option among many.

### Verification of the suggested fix

I applied the suggested test body and confirmed both directions:

- with `min_line_index: 0`: `min_line_index_excludes_global_header_area ... FAILED`
  (`294 passed; 3 failed`)
- with `min_line_index: global_header_num_lines`: `297 passed; 0 failed`

The working tree has been restored; no code change is committed by this plan.

## Plans

### Plan 1 (recommended): Adopt the suggested test body as-is

Replace the test in `src/pager/heading.rs` (L360-368):

```rust
#[test]
fn min_line_index_excludes_global_header_area() {
    let mut doc = Document::from_string("# A\n# B\nx\ny\n".into());
    // The header covers 2 lines, so lines 0 and 1 never become headings.
    let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 2);

    h.resolve(&mut doc, 3);
    // Without min_line_index, `# B` on line 1 would be picked.
    assert!(h.start_line_index().is_none());
}
```

This names `min_line_index` where the reviewer's snippet said "the bound" — inside the test
body that word has no referent, and the whole point of the test is which knob decides the
outcome.

Verified above to fail under the `min_line_index: 0` mutation and pass without it. Sits
symmetrically next to `min_line_index_ignores_wrap_rows_of_the_global_header`, which uses the
same 4-line document and the same `resolve(&mut doc, 3)` — the two then differ only in
`global_header_num_lines` (`2` vs `1`) and in the expected outcome, which makes the bound's
role readable at a glance.

### Plan 2: Same document, but also assert the first line outside the header still wins

If a `None` assertion feels too weak on its own, extend the same document by one line so the
test pins both directions of the bound:

```rust
#[test]
fn min_line_index_excludes_global_header_area() {
    let mut doc = Document::from_string("# A\n# B\n# C\nx\n".into());
    // The header covers 2 lines, so lines 0 and 1 never become headings.
    let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 2);

    // Line 1 (`# B`) matches but is inside the header, so nothing is picked.
    h.resolve(&mut doc, 1);
    assert!(h.start_line_index().is_none());

    // Line 2 (`# C`) is the first line outside the header, so it is eligible.
    h.resolve(&mut doc, 3);
    assert_eq!(h.start_line_index(), Some(2));
}
```

Note the first `resolve` must target a line inside the header (`1`), not `3` — with `x` on
line 3 the reverse scan would reach line 2 and mask the bound again, reproducing the original
defect.

### Plan 3: Leave it, file an issue instead

Defensible on the letter of the process: the vacuousness predates the reviewed range, and the
invariant is genuinely covered by the two higher-level tests, so nothing is at risk. Rejected
because the fix is a four-line edit to a test the reviewed range already touched, which is
cheaper than the issue it would file.

## Recommendation

**Plan 1.** It is the minimum change that makes the test earn its name, it is verified in both
directions, and it leaves the unit test suite reading as two symmetric cases over the same
document. Plan 2 is a reasonable upgrade if the reviewer prefers a test that pins both
directions, but the "first line outside the header is eligible" half is already covered by
`min_line_index_ignores_wrap_rows_of_the_global_header` and by
`heading_never_picks_a_line_inside_the_header_even_before_it_has_arrived`, so it mostly buys
duplication.
