# The new jump-heading draft misses a third `rows()[0]` call site

Review target: 7f909cc..134a6c4
(`dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md`, commit `5ecad30`)

## Summary

**The draft states there are exactly two heading-resolve sites using `rows()[0]`, but
`Pager::pump_input` is a third — and a sibling draft already lists it, so the two documents
contradict each other.**

Following the draft's plan literally would fix `jump_to_end` and `jump_to_bottom` and leave
`pump_input` on the wrong reference row, with no note saying that is deliberate.

### The claim

`dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md`:

> `Pager::scroll_up`, `scroll_down` and `jump_to` already resolve the heading against
> `rows()[header.height()]`; `jump_to_end` and `jump_to_bottom` are the only two call sites
> still using `rows()[0]`.

### The third site

`src/pager.rs` (L277-285), in `Pager::pump_input`:

```rust
        if result.grew && self.viewport.rows().len() < self.viewport.size().height() {
            self.relayout_page(*self.viewport.size());
            let top_line = self
                .viewport
                .rows()
                .first()
                .map_or(0, |row| row.line_index());
            self.heading.resolve(&mut self.doc, top_line);
```

`rows().first()` is `rows()[0]`, the same header-covered row the draft is about.

- **Harmless today** — this branch runs only while the viewport is under-filled, so the top
  anchor is line 0 and the search range `min_line_index..1` is empty unless there is no
  header. As with the other two sites, that is a property of the caller, not of the code.
- **Already recorded elsewhere** — `dev/issues/draft/heading-state-not-recomputed-on-resize.md`
  says, in "Why `top_line` is an `Option`":

  > `Pager::pump_input` uses `rows().first()`, which this change also corrects.

### Secondary inaccuracy

The same sentence lists `jump_to` as resolving against `rows()[header.height()]`. It does not
— it resolves against the jump target (`src/pager.rs:334`):

```rust
        self.heading.resolve(&mut self.doc, line_index);
```

That is correct behaviour for a jump, but it is not the enumerated pattern.

## Suggested fix

- List `pump_input` in the draft's call-site inventory, and state whether it is in scope here
  or deferred to `heading-state-not-recomputed-on-resize.md` (whose plan removes it as part of
  folding the resolve into `relayout_page`).
- Reword the `jump_to` mention so it is not counted as a `rows()[header.height()]` site.

## Assessment

- Newly introduced issue? Yes
- Does it block the overall goal? No

Both points are factually correct.

`Pager::pump_input` (`src/pager.rs:280-285`) does resolve from `rows().first()`, which is
`rows()[0]`. So the draft's "the only two call sites still using `rows()[0]`" is wrong, and it
contradicts `dev/issues/draft/heading-state-not-recomputed-on-resize.md`, which explicitly claims
that site:

> `Pager::pump_input` uses `rows().first()`, which this change also corrects.

The `jump_to` point is correct too — `src/pager.rs:335` resolves against `line_index` (the jump
target), not `rows()[header.height()]`:

```rust
self.heading.resolve(&mut self.doc, line_index);
```

Impact is documentation-only. No shipped behaviour is wrong, and neither draft's plan changes if
the sentence is fixed: `pump_input` is already owned by the resize draft, whose plan folds the
resolve into `relayout_page`. The problem is that a reader of this draft alone would conclude the
`rows()[0]` inventory is exhausted after fixing the two jumps, and either leave `pump_input`
silently wrong or duplicate the fix and collide with the sibling draft.

## Plans

### Plan 1: Correct the inventory sentence and mark `pump_input` as out of scope

Rewrite the single sentence in the "Overview" section of
`dev/issues/draft/heading-resolved-from-header-covered-row-on-jump.md`:

```markdown
`Pager::scroll_up` and `scroll_down` already resolve the heading against
`rows()[header.height()]` (`jump_to` resolves against its jump target instead, which is what a
jump should do). Among the sites that resolve from the viewport's top row, `jump_to_end` and
`jump_to_bottom` still use `rows()[0]`. `Pager::pump_input` uses the equivalent `rows().first()`,
but it is out of scope here: `dev/issues/draft/heading-state-not-recomputed-on-resize.md` corrects
it as part of folding the heading resolve into `relayout_page`.
```

No other part of the draft changes — the "Root Cause" and "Plan" sections already talk only about
`jump_to_end` / `jump_to_bottom`.

### Plan 2: Widen the draft's scope to also fix `pump_input`

Add `pump_input` to this draft's plan as a third call site:

```rust
let top_line = self
    .viewport
    .rows()
    .get(self.header.height())
    .map_or(0, |row| row.line_index());
self.heading.resolve(&mut self.doc, top_line);
```

Rejected. The resize draft already owns this line and removes it entirely (the resolve moves into
`relayout_page`), so fixing it here means writing code that the sibling issue immediately deletes,
plus a `.get()` fallback whose `None` case that draft argues should be an `Option`, not a `0`. Two
drafts editing the same statement in incompatible ways is worse than the current inaccuracy.

### Plan 3: Leave the draft as is

Rejected. The sentence is a call-site inventory used to justify "these are the last two"; leaving
it wrong is exactly the kind of stale claim that survives into the implementation commit.

## Recommendation

**Plan 1.** The defect is purely in the draft's prose, and one rewritten sentence removes both the
missing call site and the `jump_to` mischaracterisation while keeping the ownership boundary
between the two drafts explicit. No code change, no scope creep.
