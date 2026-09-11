# Goal

Make `rows::list_backward` take its start position as a `RowPos` instead of a `&Row`,
by changing `DocPos::Before(&Row)` to `DocPos::Before(RowPos)`.

# Context

`rows::list_forward` and `rows::list_backward` take their start in different forms:

- `list_forward(.., start: RowPos, ..)` takes a position.
- `list_backward(.., start: DocPos, ..)` takes `DocPos::Before(&Row)`, a whole row.
  - It only reads `line_index()` and `wrap_index()` from the row, never its `raw_range`.
  - This is also the only reason `DocPos` carries a `'row` lifetime parameter.

The mismatch shows up in `layout::anchor_backward`. It holds a `RowPos`, so it has to
re-wrap the line to build a `Row` just to pass it to `list_backward`.
Its counterpart `anchor_forward` passes its `RowPos` straight to `list_forward`.
With a `RowPos` in `DocPos::Before`, both functions take the same shape.

## Out-of-range positions

Building the `Row` in `anchor_backward` also acts as a range check: a missing line or a
`wrap_index` past the line's last row makes it return `from` unchanged.
Once the position is passed directly, `list_backward` has to handle such positions itself.

- A `wrap_index` beyond the line's rows currently underflows and panics in `list_backward`.
  It is clamped instead, so the position is treated as just after the line's last row.
  This matches `list_forward`, which also tolerates such a `wrap_index` by moving on to
  the next line.
- A missing line yields no rows, and `anchor_backward` falls back to `from` as before.

Real anchors come from the current frame or from the start of an existing line, so they
are valid for the current width. The behavior change is therefore mostly theoretical.

## Unchanged

The start of `list_forward` stays inclusive and the start of `list_backward` stays exclusive.

- `layout::fill_from` wants "the rows just before the first visible row", which the
  exclusive start expresses directly.
- The `count + 1` in `anchor_forward` compensates for this difference and is correct as is.
