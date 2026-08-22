# The new streaming heading test does not cover the fix, and its stated premise is unobservable

Review target: `7f909cc..134a6c4` (`src/pager.rs`, `heading_never_picks_a_line_inside_the_header_even_before_it_has_arrived`)

## Summary

**The test added at `317a29d` passes unchanged both with the bug reintroduced and with its
streaming premise removed, so it pins neither the fix nor the case its name describes.**

- It asserts a redirect performed by `Header::contains` in `Pager::jump_to`, not by
  `min_line_index`.
- Its "before it has arrived" wording describes an ordering that no assertion observes.

## What the test claims

`src/pager.rs` (L962-965):

```rust
    /// A line inside the configured header range never becomes a sticky heading, whether or
    /// not it has arrived yet at the moment `Heading` is asked about it.
    #[test]
    fn heading_never_picks_a_line_inside_the_header_even_before_it_has_arrived() {
```

## Verification

Both checks were run against the tree at `134a6c4`; the tree was restored afterwards.

**1. It does not exercise the fix.**

Reverting `HeadingConfig::new` to the pre-fix line (`min_line_index: global_header_height`)
leaves this test green. Only two tests fail:

```
test pager::heading::tests::min_line_index_ignores_wrap_rows_of_the_global_header ... FAILED
test tests::heading::heading_just_below_wrapped_header_line ... FAILED
```

The header in the test is 3 unwrapped lines, so `height() == num_lines() == 3`; the change under
review cannot alter its result.

**2. The streaming ordering is never observed.**

Moving every `tx.send` before `Pager::new` — so the whole document has arrived before `Heading`
is ever asked anything — also leaves it green. Nothing is asserted between `Pager::new` (where
line 1 is still missing) and `pump_input()` (where the document is complete), so the "before it
has arrived" branch has no witness.

## What it actually asserts

`src/pager.rs`, `jump_to` (L328-335):

```rust
    pub fn jump_to(&mut self, mut line_index: usize) -> PageUpdate {
        if self.header.contains(line_index) {
            line_index = 0;
        }
```

- `jump_to(1)` — `Header::contains(1)` is `true`, so `line_index` becomes `0`,
  `resolve` searches the empty range `3..1`, and no heading is set.
- `jump_to(3)` — line 3 is outside the header, so it resolves normally.

Both outcomes are produced by `Header::contains`, which this work did not touch.

## Why it matters

The purpose of `317a29d` was to pin the "document shorter than the configured header" shape
recorded in `dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md`. A test
whose result is invariant to both the fix and the shape it names records that argument in prose
only — it will keep passing if the argument stops holding.

Options, roughly in order of how much they change:

- **Assert during the gap.** Take a snapshot between `Pager::new` and `pump_input()`, while
  line 1 is genuinely absent, so the ordering has a witness.
- **Bypass the `jump_to` redirect.** Reach `Heading::resolve` through a path that does not
  consult `Header::contains` (e.g. `resolve_if_found` via scrolling), so `min_line_index` is
  what decides the outcome.
- **Rename it to what it checks.** If the intent really is to pin `jump_to`'s redirect under
  streaming, the name and doc comment should say so, and it belongs next to the other
  `jump_to` tests.

## Assessment

- Newly introduced issue? Yes
- Does it block the overall goal? No

**The two factual claims hold; the framing "does not cover the fix" is the wrong yardstick.**

### Both verifications reproduce

Re-ran both against the tree at `134a6c4`, restoring it afterwards.

1. Reverting `HeadingConfig::new` to `min_line_index: global_header_height` leaves this test
   green. Exactly the two tests the reviewer names fail:

   ```
   pager::heading::tests::min_line_index_ignores_wrap_rows_of_the_global_header
   tests::heading::heading_just_below_wrapped_header_line
   ```

2. Moving every `tx.send` before `Pager::new` leaves the whole suite green (`297 passed`).
   No assertion sits between `Pager::new` and `pump_input()`, so the streaming ordering has
   no witness.

### But "cover the fix" was never this test's job

The fix *is* covered — by the two tests that fail on revert above. The shape `317a29d` set out
to record is the *other* direction, and
`dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md` argues that direction
produces **no divergence at all**:

> every `line_index` that reaches `Heading::resolve`/`resolve_if_found` comes from a row the
> document already has, so while it's shorter than `num_lines`, `line_index < doc.line_count()
> <= num_lines` always holds. The search range [...] is therefore always empty

A test of that shape *cannot* be made to fail on reverting the fix without contradicting the
argument it exists to illustrate. Demanding one is asking for a witness that the design says
does not exist.

### One correction to the attribution

The review says both outcomes "are produced by `Header::contains`". They are **over-determined**,
not produced by it. Stubbing the redirect out:

```rust
if false && self.header.contains(line_index) {
    line_index = 0;
}
```

still leaves the whole suite green (`297 passed`). Without the redirect, `resolve(doc, 1)`
searches `min_line_index..2` == `3..2`, also empty. So `min_line_index` *is* consulted here —
it just holds the same value (`3`) before and after the fix, because `pump_input()` →
`relayout_page()` rebuilds `HeadingConfig` once the full document has arrived and
`height() == num_lines() == 3`.

### What is actually wrong

The name and doc comment overclaim. `even before it has arrived` and "whether or not it has
arrived yet at the moment `Heading` is asked about it" describe an ordering nothing observes.
That is a real defect — a reader trusts the name, and the streaming scaffolding can be deleted
without any test noticing.

## Plans

### Plan 1: Give the gap a witness, and rename to what is pinned

Assert between `Pager::new` and `pump_input()`, where the document genuinely is shorter than
`--header-lines`, then rename accordingly:

```rust
/// While a streamed document is still shorter than `--header-lines`, `Header::height()`
/// falls below `Header::num_lines()`. No line the header is configured to cover becomes a
/// sticky heading, before or after it arrives; the first line past the header does.
#[test]
fn heading_stays_outside_the_configured_header_while_the_document_is_still_shorter() {
    // ...unchanged setup: only line 0 ("# A") has arrived...
    let mut pager = Pager::new(doc, opts, ScreenSize::new(20, 10));

    // The header is configured for 3 lines but only line 0 exists, so it occupies 1 row,
    // and no heading is set while the rest is still missing.
    let (snap, _) = pager.snapshot();
    assert_eq!(line_indices(snap.header), vec![0]);
    assert!(line_indices(snap.heading).is_empty());

    // ...unchanged: the rest arrives, then the jump_to(1) / jump_to(3) assertions...
}
```

The `vec![0]` header assertion is the witness: moving the `tx.send` calls before `Pager::new`
now fails it, so the ordering the name describes is load-bearing.

Cost: ~4 lines. Keeps the documented shape covered end-to-end and drops the false claim.

### Plan 2: Delete the test

Its half of `317a29d` is not reachable as a behavioral guard, and what remains is covered:

- `header_height_stays_below_num_lines_while_document_is_still_shorter` (the sibling added in
  the same commit) already pins the `height() < num_lines()` rendering shape.
- `jump_to_within_global_header_jumps_to_top` already pins the redirect.
- `min_line_index_excludes_global_header_area` / `min_line_index_ignores_wrap_rows_of_the_global_header`
  pin `min_line_index` at the unit level.

Precedent on this very branch: `e7916cb` ("Drop test that couldn't distinguish its own
branches"). The no-divergence argument already lives in the issue doc, where prose belongs.

### Plan 3: Rename only

Keep the body, retitle to `jump_to_into_the_configured_header_sets_no_heading`, drop the
"before it has arrived" wording, and move it next to `jump_to_within_global_header_jumps_to_top`.
Cheapest honest option, but it throws away the streaming setup's only reason for existing
instead of making it count.

## Recommendation

**Plan 1.**

The shape `317a29d` records is real and worth having an end-to-end test for — a short or
still-streaming document driving `height() < num_lines()` is exactly the case the issue doc
reasons about, and reasoning is what regressions creep past. Plan 1 keeps that coverage and
costs four lines, while turning the name from an unbacked claim into one an assertion enforces.

Plan 2 is the honest fallback if the gap assertion is judged too weak to earn its keep, but it
trades a slightly thin test for none at all in a shape the codebase explicitly reasons about.

What none of the plans should do is contort the test into failing on the reverted fix. In this
direction there is nothing to fail on, by design.
