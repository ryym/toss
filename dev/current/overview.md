# Goal

Unify the row position type into a single `RowPos` struct defined next to `Row` in `src/line.rs`,
and let a `Row` produce its own position via `Row::pos()`.

# Context

A row position in the document is the pair `(line_index, wrap_index)`.
It is currently expressed as `type RowPos = (usize, usize)`, defined separately in three places:

- `src/pager/layout.rs` (anchors, also imported by `src/pager.rs`)
- `src/pager/rows.rs` (the start of `list_forward`)
- `src/renderer.rs` (the identity of a painted row, used as a `HashMap` key in `plan_shift`)

Besides the duplicated definitions, the pair is assembled by hand as
`(r.line_index(), r.wrap_index())` wherever a `Row` needs to become a position.

Why `src/line.rs`:

- A `RowPos` is the position of a `Row`, so it belongs next to it.
- Both the pager and the renderer use it. Keeping it in `pager::layout` would make the
  renderer depend on pager internals.

Why a struct rather than a shared type alias:

- Two bare `usize`s are easy to swap by mistake; named fields prevent that.
- Field access reads better than `.0` / `.1` or tuple destructuring.
- Common positions get a name, e.g. the first row of a line instead of `(line_index, 0)`.

`Row` itself implements no equality or hashing traits, so identity by position is the
responsibility of `RowPos`. Ordering is not derived until something needs it.

The main cost is verbosity in tests, which often pass positions as tuple literals such as
`(2, 0)`. A short constructor keeps them readable. Test helpers that collect rows into
`Vec<(usize, usize)>` for assertions can stay as they are.

## Out of scope

These are left for separate changes after this one:

- `rows::list_backward` takes its start as `DocPos::Before(&Row)` although it only needs
  the position, which forces `layout::anchor_backward` to rebuild a `Row` from a `RowPos`.
  It should take a `RowPos` instead.
- `Row` keeps storing `line_index` and `wrap_index` as separate fields. Holding a `RowPos`
  instead would express the structure more directly.
