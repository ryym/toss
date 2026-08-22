# Header Height Is Spelled Two Ways in `Pager`

Review target: 7f909cc..306811d (`src/pager.rs`) — pre-existing code, surfaced by this change

## Summary

**`Pager::contiguous_top_row_index` reaches for `header.rows().len()` where every other
call site uses `header.height()`.**

Same value, two spellings, in a file where confusing header metrics has just caused a bug.

`src/pager.rs` (L234-246):

```rust
    fn contiguous_top_row_index(&self) -> usize {
        let rows = self.viewport.rows();
        if let Some(row) = self.header.rows().first()
            && row == &rows[0]
        {
            return 0;
        }
        if let Some(row) = self.heading.rows().first()
            && row.line_index() == rows[self.header.rows().len()].line_index()
        {
            return self.header.rows().len();
        }
        self.total_header_height()
    }
```

### Why it is worth fixing now

- **`height()` is now a documented concept**, not just a length.
  - The diff gave it a doc comment spelling out how it differs from `num_lines()`.
  - `rows().len()` bypasses that name, so a reader has to re-derive which unit is in play.
- **Every other row-space use in the file goes through `height()`** — L214, L337, L425,
  L435, L437, L456, L464. This function is the lone exception.
- **`Header::rows()` is only genuinely needed for the rows themselves** — L203 (snapshot)
  and the `.first()` call above. Using it as a length keeps a wider accessor in play than
  the code needs.

Replacing both occurrences with `self.header.height()` is behavior-preserving.

## Assessment

- Newly introduced issue? No
- Does it block the overall goal? No

The feedback is valid. `Header::height()` is defined as `self.rows.len()`
(`src/pager/header.rs`), so it and `Header::rows().len()` always return the same
value — there is no semantic difference, only a style inconsistency. `height()`
now carries a doc comment explaining how it differs from `num_lines()` (rendered
vs. configured extent), so it is the name a reader is meant to reach for when
thinking in row-space; `rows().len()` bypasses that documented concept for no
benefit. The fix is a pure rename with no behavior change, and it is confined to
a single pre-existing function, so it is safe to make now rather than deferring.

## Plans

### Plan 1: Use `header.height()` in both spots (Recommended)

```rust
fn contiguous_top_row_index(&self) -> usize {
    let rows = self.viewport.rows();
    if let Some(row) = self.header.rows().first()
        && row == &rows[0]
    {
        return 0;
    }
    if let Some(row) = self.heading.rows().first()
        && row.line_index() == rows[self.header.height()].line_index()
    {
        return self.header.height();
    }
    self.total_header_height()
}
```

Only the two `self.header.rows().len()` occurrences change to
`self.header.height()`; the `self.header.rows().first()` call stays as-is since
it genuinely needs the rows, not their length. Behavior-preserving, one-line-ish
diff.

## Recommendation

Plan 1. It is the only reasonable fix — a trivial, behavior-preserving rename
that removes the sole exception to how the rest of the file spells header row
counts.
