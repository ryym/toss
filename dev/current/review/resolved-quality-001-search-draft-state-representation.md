# Search draft state is spread over two fields that allow illegal combinations

Review target: `SearchInputMode` state added for invalid-regex handling

- src/pager.rs:98-108 (`SearchInputMode` with `draft` + `is_query_valid`)
- src/pager.rs:505-551 (`update_search_query` maintains both fields)

## Summary

The new `is_query_valid: bool` and the existing `draft: Option<SearchState>` together
encode three logical states of the search input:

| logical state | `is_query_valid` | `draft` |
| --- | --- | --- |
| input is empty | `true` | `None` |
| input is a valid regex | `true` | `Some(state)` matching the input |
| input is an invalid regex | `false` | frozen last valid state, or `None` |

Two independent fields can express 4 combinations, and one of them
(`is_query_valid == false` together with a `draft` that *does* correspond to the current
input) is meaningless. Nothing in the type prevents it; only the ordering of assignments
inside `update_search_query` does. Every future reader of `draft` has to also know
`is_query_valid` to understand whether `draft` reflects the current input or a frozen
older one — that relationship is invisible at the field declaration and only recoverable
from doc comments.

There is also a subtle naming mismatch: the field is named `is_query_valid` ("the query is
a valid regex") but its doc comment defines it as "submittable: either empty or a valid
regex". Those are different predicates, and the empty case is deliberately folded into the
`true` branch. A reader who trusts the name will get the empty case wrong.

Making the state one value would remove both problems, e.g.:

```rust
enum SearchDraft {
    /// The input is empty: nothing to preview, but Enter may still be pressed.
    Empty,
    /// The input compiled successfully; the state matches the current input.
    Valid(SearchState),
    /// The input does not compile; the preview is frozen at the last valid state.
    Invalid(Option<SearchState>),
}
```

With this, "is submittable" and "which state to preview" become single `match`es on one
value, the frozen-vs-live distinction is explicit in the type, and the invalid state
literally cannot be paired with a fresh draft.

Somewhat related open question: should compiling the input and remembering the last valid
compilation live in `Pager` at all? It is a self-contained concern (raw text in, "regex or
frozen previous regex" out) that reads naturally as a small type next to `LineEditor`,
leaving `Pager` to deal only with searching and scrolling.

## Assessment

Valid. I re-checked every read/write site of `draft` and `is_query_valid`
(`start_search_input`, `submit_search`, `update_search_query`, and the `draft.as_ref().or(...)`
read in `snapshot`) — both fields are private to `pager.rs` and touched only inside
`Pager`'s `impl` block, so the "illegal" combination really is prevented solely by the
order of two assignments in `update_search_query` (src/pager.rs:524-528), not by the type.
Today that discipline is easy to keep because the whole struct is ~50 lines, but it is the
kind of invariant that silently breaks on a future edit (e.g. an early return added between
the two assignments) with no compiler signal.

The naming-vs-doc mismatch is real but minor by itself: `is_query_valid` is documented as
"submittable" (empty OR valid regex), which is a different predicate than the name implies.
It's worth fixing alongside the field consolidation rather than on its own.

The enum collapses the state into one value and removes both problems for a small, fully
contained change — no other module touches these fields. This is a good case for "fix it
now" rather than "note it and move on."

## Plans

### Plan 1: Collapse into a `SearchDraft` enum (recommended)

Replace `draft: Option<SearchState>` + `is_query_valid: bool` on `SearchInputMode` with a
single field, close to the reviewer's sketch:

```rust
enum SearchDraft {
    /// Input is empty: nothing to preview, but Enter may still submit (clears the query).
    Empty,
    /// Input compiled; the state matches the current input.
    Valid(SearchState),
    /// Input does not compile; preview stays frozen at the last valid state, if any.
    Invalid(Option<SearchState>),
}

impl SearchDraft {
    fn is_submittable(&self) -> bool {
        !matches!(self, SearchDraft::Invalid(_))
    }

    fn preview(&self) -> Option<&SearchState> {
        match self {
            SearchDraft::Valid(s) => Some(s),
            SearchDraft::Invalid(Some(s)) => Some(s),
            _ => None,
        }
    }
}
```

Update the four call sites accordingly:
- `start_search_input` (src/pager.rs:465-471): initialize with `SearchDraft::Empty`.
- `update_search_query` (src/pager.rs:513-543): on empty input, set `Empty`; on regex
  compile failure, replace the current value with `Invalid(self.draft.preview().cloned())`
  (or restructure to reuse the existing `SearchState` without cloning — the current code
  already avoids a clone by just not touching `draft`, so a `mem::take`-based swap that
  extracts the previous preview without an extra allocation is fine too); on success, set
  `Valid(state)`.
- `submit_search` (src/pager.rs:478-492): replace `is_query_valid` check with
  `mode.draft.is_submittable()`, and match on `SearchDraft::Valid`/`Invalid(Some(_))` to
  extract the state to commit (both should be submittable per current behavior — check:
  does the current code allow submitting while frozen-invalid? Looking at line 482, no —
  `is_query_valid == false` blocks submit entirely, so only `Empty`/`Valid` are
  submittable; `is_submittable` above should return `false` for `Invalid` regardless of
  its payload).
- `snapshot` (src/pager.rs:171): `search.draft.preview().or(self.search.as_ref())`.

This is a self-contained rename-and-restructure; no public API changes, no test changes
expected beyond compiling (tests only observe behavior through `snapshot()`/`submit_search()`).

### Plan 2: Extract a standalone `SearchDraft` type near `LineEditor`

Same enum as Plan 1, but move it (plus the "compile raw input, remember last valid state"
logic currently inlined in `update_search_query`) into its own module/type, e.g.
`search_draft.rs`, with a single `update(&mut self, input: &str) -> ...` method that
`Pager::update_search_query` calls. This answers the reviewer's open question by giving
`Pager` a narrower job (searching/scrolling) and the draft type a narrower job (raw text →
submittable regex-or-frozen-state).

This is strictly more work than Plan 1 for the same correctness win, and only pays off if
similar "raw input → validated preview" logic is expected elsewhere (none currently exists
in the codebase). Worth doing only if that pattern is anticipated to recur; otherwise it's
speculative structure for a single call site.

## Recommendation

Plan 1. It removes the illegal state and the naming mismatch — the two concrete problems
raised — with a change fully contained in `pager.rs` and no behavior change. Plan 2's
extraction is reasonable but not justified yet: there's only one caller of this logic today,
so a separate type would be speculative generality until a second use case shows up.
