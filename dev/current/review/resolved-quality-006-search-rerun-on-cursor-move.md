# `update_search_query` re-compiles and re-searches even for edits that do not change the text

Review target: `update_search_query`, in light of the new compile-per-keystroke path

- src/pager.rs:505-551 (`update_search_query`)
- src/app.rs:177-178 (`MoveCursorLeft` / `MoveCursorRight` routed through `update_search_query`)

## Summary

Every `LineEdit` goes through the same path: edit the buffer, compile the input, search the
whole document, jump. For `MoveCursorLeft` / `MoveCursorRight` the input text is unchanged, so
the compile and the document scan are guaranteed to reproduce the previous result — only the
status line's cursor position actually differs.

This is pre-existing, and its cost was low when the pattern was always a `regex::escape`d
literal. Two things make it more worth naming now:

- The compiled pattern can now be an arbitrary user regex, so the per-edit compile is no
  longer bounded by "escaped literal" cost, and the scan is no longer a substring search.
- The function's contract is now genuinely "recompute the draft from the raw input". Running
  that for an edit that provably cannot change the raw input is the kind of redundant dynamic
  work the code otherwise avoids carefully (see the flicker-avoidance in `refresh_rows`).

Open question rather than a concrete demand: the cheapest fix is for `LineEditor::edit` to
report whether the text changed (as opposed to only the cursor), letting `update_search_query`
return `StatusOnly` early when it did not. That also happens to be exactly the signal needed to
avoid recomputing `is_query_valid` for cursor moves. Whether it is worth the extra return value
depends on how large documents are expected to get; on a small file the current behaviour is
imperceptible.

## Assessment

Valid observation. `MoveCursorLeft`/`MoveCursorRight` can never change `mode.editor.input()`, so
recompiling the regex and re-scanning the whole document on every cursor move is provably
redundant work, and it's no longer bounded by "escaped literal" cost now that the query can be an
arbitrary user regex on a scan that's no longer a plain substring search.

That said, the reviewer's suggested mechanism (having `LineEditor::edit` report back whether the
text changed) is more machinery than the problem needs. `update_search_query` already receives
the `LineEdit` value itself, and `MoveCursorLeft`/`MoveCursorRight` are statically identifiable as
cursor-only edits at that call site — no need to thread a changed-flag out of `LineEditor` to
learn something the caller already knows from the variant it just passed in.

## Plans

### Plan 1: Short-circuit on cursor-only edits, with `LineEdit` itself owning the classification (recommended)

Whether an edit can change the raw input text is a property of `LineEdit`, not something the
call site should re-derive. Add a method on `LineEdit` so the `match` lives next to the enum
definition — that way, adding a future variant makes this `match` non-exhaustive and the compiler
forces a decision, instead of silently falling through an unrelated `matches!` at the call site:

```rust
// line_editor.rs
impl LineEdit {
    /// Whether this edit can change the raw input text (as opposed to only the cursor).
    pub fn changes_text(&self) -> bool {
        match self {
            LineEdit::AddChar(_) | LineEdit::DeleteCharBeforeCursor => true,
            LineEdit::MoveCursorLeft | LineEdit::MoveCursorRight => false,
        }
    }
}
```

Then in `update_search_query`:

```rust
pub fn update_search_query(&mut self, edit: LineEdit) -> PageUpdate {
    let PagerMode::SearchInput(mode) = &mut self.mode else {
        return PageUpdate::StatusOnly;
    };

    let changes_text = edit.changes_text();
    mode.editor.edit(edit);
    if !changes_text {
        return PageUpdate::StatusOnly;
    }

    let input = mode.editor.input();
    // ... unchanged from here
}
```

This still applies the cursor movement (needed so the status line renders the new cursor
position) but skips the regex compile, `is_query_valid` update, and `search::search_document`
call entirely, since none of them can change when the raw input is unchanged.

### Plan 2: Do not address

Leave as-is. The redundant compile/scan only triggers on `Left`/`Right` while the search prompt is
open, which is a narrow interaction window, and the review itself notes the cost is imperceptible
on small files. Revisit if profiling ever shows search-input latency as a real problem on large
documents.

## Recommendation

Plan 1. It's a small, local, easily-reviewed change that removes provably-redundant work, adds
only a single classification method to `LineEdit` (no new state, no change to `LineEditor`'s
edit-application logic or its existing tests), and keeps the "does this edit change the text"
knowledge next to the enum it belongs to rather than duplicated/hand-matched at each call site.
The one-time cost is trivial and there's no ongoing maintenance burden, so there's no strong
reason to defer it.
