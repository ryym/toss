# `submit_search` matches on `self.mode` twice to work around `mem::take`

Review target: the validity guard added to `submit_search`

- src/pager.rs:478-492 (`submit_search`)

## Summary

The guard was added as a separate leading `let ... else` on `&self.mode`, so the function now
destructures `PagerMode::SearchInput` twice: once immutably to read `is_query_valid`, then
again after `mem::take` to consume the draft. The shape only exists because `mem::take`
unconditionally leaves the pager in `View` mode, and the new early return must happen *before*
that side effect — but that reasoning is not written down anywhere, so the duplicated match
reads as accidental redundancy. A reader deleting the "redundant" first match would silently
introduce a bug where an invalid input exits search mode.

Reading the state first and only then applying the mode transition expresses the intent
directly, e.g.:

```rust
pub fn submit_search(&mut self) -> PageUpdate {
    let PagerMode::SearchInput(mode) = &mut self.mode else {
        return PageUpdate::StatusOnly;
    };
    if !mode.is_query_valid {
        // Keep the search input mode active so the user can keep editing.
        return PageUpdate::StatusOnly;
    }
    if let Some(draft) = mode.draft.take() {
        log::debug!("Submit search: query={:?}", draft.query.as_str());
        self.search = Some(draft);
    }
    self.mode = PagerMode::View;
    PageUpdate::StatusOnly
}
```

This also makes the two exits distinguishable at a glance: "not in search input" and "input is
not submittable" currently return the same value from two structurally identical-looking
statements, and only the second one is a deliberate no-op the user can recover from.

If quality-001's single-enum draft state is adopted, this collapses further into one `match`
on the draft.

## Assessment

Valid. The double match is real: `submit_search` reads `&self.mode` once to check
`is_query_valid`, then unconditionally calls `mem::take(&mut self.mode)` and matches again to
consume `draft`. The only reason for this shape is that `mem::take` always resets `self.mode` to
`PagerMode::View` as a side effect, so the validity check must run first to avoid dropping the
user back into `View` mode on an invalid regex. That constraint isn't documented, so the
duplicated match looks like accidental redundancy rather than a deliberate ordering requirement.

The sibling function `update_search_query` (src/pager.rs:505-508) already uses the
`let PagerMode::SearchInput(mode) = &mut self.mode else { return ... }` shape for the same kind
of early-return-if-not-in-search-mode guard, so switching `submit_search` to match once via
`&mut self.mode` and assigning `self.mode = PagerMode::View` explicitly at the end is consistent
with the existing convention in this file, not a new pattern.

## Plans

### Plan 1: Match once via `&mut self.mode`, assign `PagerMode::View` explicitly (recommended)

Adopt the reviewer's suggested rewrite: destructure `&mut self.mode` a single time, early-return
on both "not in search input" and "input not valid", then `mode.draft.take()` and set
`self.mode = PagerMode::View` explicitly at the end instead of relying on `mem::take`'s side
effect to do it implicitly. This removes the duplicated match, makes the two early-return cases
distinguishable, and matches the existing `let ... else` + `&mut self.mode` idiom already used in
`update_search_query`. Low cost, self-contained to `submit_search`, no test changes needed since
behavior is unchanged.

### Plan 2: Leave as-is, add a comment explaining the ordering

If a rewrite is deemed not worth touching right now, just add a short comment above the first
match explaining that the validity check must precede `mem::take` because `mem::take` always
resets `self.mode` to `View`. This addresses the "reads as accidental redundancy" concern cheaply
without a structural change, but leaves the actual duplication in place.

## Recommendation

Plan 1. It's a small, low-risk, purely local rewrite that removes real duplication and brings
`submit_search` in line with the pattern already established by `update_search_query` in the same
file — cheaper in the long run than documenting around the duplication with Plan 2.
