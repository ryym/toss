# Heading resolved from a header-covered row on jump

Review target: `7f909cc..a4a1ae3` (`src/pager.rs`, `src/pager/heading.rs`)

## Summary

**`Pager::jump_to_end` and `Pager::jump_to_bottom` resolve the sticky heading from
`viewport.rows()[0]`, a row the global header covers, so `G` can pin a heading from an earlier
section.**

- The heading sticks to the first *visible content* line, which is
  `viewport.rows()[header.height()]`.
- `viewport.rows()[0..header.height()]` is hidden: `Pager::snapshot` builds content as
  `&self.viewport.rows()[self.total_header_height()..]`.
- `scroll_up` / `scroll_down` already use `rows()[header.height()]`; these two do not.

`src/pager.rs` (L353-355, `jump_to_end`; `jump_to_bottom` L376-378 is identical):

```rust
let top_line_index = self.viewport.rows()[0].line_index();
self.heading.resolve(&mut self.doc, top_line_index);
self.push_up_heading_if_needed();
```

## Reproduction

Confirmed on `a4a1ae3` with a temporary test in `src/tests/heading.rs`
(`screen_width: 20`, `screen_height: 7`, `header: 2`, `heading: ^# `, events `G`, `q`):

```
H1
H2
# A
a1
a2
# B
b1
b2
b3
b4
```

After `G`:

```
H1
H2
# A
b2
b3
b4
lines 5-10/10 100%
```

- **Shown** — `# A`.
- **Expected** — `# B`; the visible content `b2 b3 b4` belongs to section B.
- **Why** — the viewport top row is `a2` (line 4), hidden under the 2-row header, so
  `resolve` searches `..=4` and finds `# A` on line 2. `# B` on line 5 is also hidden, so the
  compensating scan in `push_up_heading_if_needed` (which starts at `header.height()`) never
  sees it and `push_up` stays `0`.

## Relation to existing issues

`dev/issues/draft/heading-state-not-recomputed-on-resize.md` already states the rule:

> the reference row is `viewport.rows()[header.height()]` — not `rows()[0]`, which the global
> header covers. Resolving against `rows()[0]` would pick a heading from an earlier section
> whenever a new one starts within the covered rows.

That draft applies the rule to `relayout_page` and `pump_input` only. The two jump call sites
are not mentioned there, and unlike those, they are wrong on the current code path today
(no resize needed).

Note the index cannot be taken raw: `rows().len()` can be `<= header.height()`
(see `dev/issues/draft/empty-viewport-panics-on-key-input.md`).

## Assessment

- Newly introduced issue? **No.** `git show main:src/pager.rs` (main's tip is `7f909cc`, the
  start of the reviewed range) already has `rows()[0]` at both call sites, byte-for-byte
  identical to `a4a1ae3`. The reviewed range `7f909cc..a4a1ae3` never touches these lines, so the
  reproduction in this file fails the same way on `main` today. This is a pre-existing bug, not
  something the reviewed changes introduced.
- Does it block the overall goal? No — the reviewed range is unrelated to it.

Confirmed by reading the code: `scroll_up`/`scroll_down`/`jump_to` all resolve the heading
against `self.viewport.rows()[self.header.height()].line_index()` — the first row *not* covered
by the global header. `jump_to_end`/`jump_to_bottom` instead use `rows()[0]`, which is under the
header whenever `header.height() > 0`, so `heading.resolve` searches too far back and can pick a
heading from an earlier section. The reproduction in this file is consistent with the code, and
is reproducible on `main` as-is.

Per "Focus on Original Goal" in the review-plan instructions, a pre-existing issue outside the
reviewed diff should be filed rather than fixed inline here. The fix itself is trivial, so the
plan below is kept concrete enough for whoever picks up the filed issue to apply directly.

`rows()[0]` panics on an empty viewport just as `rows()[header.height()]` would
(`dev/issues/draft/empty-viewport-panics-on-key-input.md` already tracks that separately for
both call patterns); fixing this issue doesn't change that exposure, so no extra guard is needed
here.

## Plans

### Plan 1: Use `rows()[header.height()]` in both jump methods

Make `jump_to_end` and `jump_to_bottom` resolve against the same reference row as
`scroll_up`/`scroll_down`/`jump_to`, instead of `rows()[0]`:

```rust
// src/pager.rs, Pager::jump_to_end
let top_line_index = self.viewport.rows()[self.header.height()].line_index();
self.heading.resolve(&mut self.doc, top_line_index);
self.push_up_heading_if_needed();
```

```rust
// src/pager.rs, Pager::jump_to_bottom
let top_line_index = self.viewport.rows()[self.header.height()].line_index();
self.heading.resolve(&mut self.doc, top_line_index);
self.push_up_heading_if_needed();
```

This is a one-line change per call site and makes all heading-resolution call sites use the same
rule, matching the invariant already documented in
`dev/issues/draft/heading-state-not-recomputed-on-resize.md`. Add a regression test based on the
reproduction in this file (e.g. adapt the temporary test used to confirm it) asserting `G` shows
`# B`, not `# A`.

## Recommendation

File an issue (Plan 1's fix as the recorded proposal) rather than fixing it inline in this
branch: the bug predates the reviewed range and is unrelated to its goal. Plan 1 itself is a
minimal, targeted fix — aligning `jump_to_end` and `jump_to_bottom` with the reference-row rule
every other heading-resolving call site already follows — so whoever picks up the issue can apply
it directly.

## Filed as Issue

`dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md`
