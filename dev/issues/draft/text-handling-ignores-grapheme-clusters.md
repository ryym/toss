---
type: bugfix
tags: [pager, search]
---

## Overview

**Characters that render as one glyph are treated as separate units, so the search input and the
status line both break them apart.**

A user-perceived character is a grapheme cluster and can span several `char`s: `á` in its
decomposed form is `a` + `U+0301`, and a family emoji is several emoji joined by `U+200D`. Both
the line editor and the status line work one `char` at a time, which shows up in four ways.

- **The cursor stops where no cell exists.** The cursor moves one character at a time, so it can
  stop between `a` and `U+0301`. The two render as a single cell, so that position has no column
  of its own and the cursor is drawn on a zero-width cell, which shows nothing.
- **Backspace deletes half a character.** Deleting before the cursor removes one `char`, so
  backspacing over `á` strips only the accent and leaves `a` behind. It looks like the deletion
  did nothing.
- **Highlighting the cursor splits joined emoji.** The cursor cell is built as "one character plus
  the zero-width characters that follow it", which only holds when every continuation of a cluster
  is zero-width. It is not: in `👨\u{200D}👩\u{200D}👧` the widths are `2, 0, 2, 0, 2`, so the cell
  becomes `👨\u{200D}` and the reverse-video sequence is emitted between the joiner and the emoji it
  joins. Terminals treat that as a cluster break and draw the parts separately, so the prompt text
  changes just by moving the cursor onto the emoji. A flag such as `🇯🇵` splits the same way, its
  two regional indicators being one column each.
- **Clipping keeps orphaned marks.** Clipping the status line drops columns from the left, and a
  zero-width character always "fits", so the cut can drop a base character while keeping the marks
  that modify it. The line then starts with marks that attach to whatever lands in the first
  column. This one also affects the view-mode position indicator, since a file name can be
  decomposed.

Decomposed text is not exotic: macOS file names are stored in NFD, so pasting one into the search
prompt is enough to produce this input.

Only the third symptom is recent. The other three predate the cursor being drawn as a marker: the
block character that used to represent the cursor was itself inserted between `a` and its accent,
and clipping cut the same way.

## Reproduction

### Cursor with no cell

1. Start the pager and press `/`.
2. Enter `á` in its decomposed form (`a` followed by `U+0301`).
3. Press Left once.

The cursor sits on `U+0301` and is drawn nowhere. Pressing Left again puts it on `a`, where it
becomes visible.

### Backspace over a decomposed character

Same input, but press Backspace at step 3: the prompt shows `a` and the input only shrinks from
two characters to one.

### Joined emoji split by the cursor

1. Press `/` and enter `👨‍👩‍👧x`.
2. Press `Ctrl-a` to put the cursor on the emoji.

The family is drawn as separate `👨` `👩` `👧` glyphs and only the first is highlighted. Calling
`cursor_cell` with that input returns `("👨\u{200D}", "👩\u{200D}👧x")`.

### Orphaned marks after clipping

`clip("/a\u{301}", 0)` returns a bare `\u{301}`: the mark is kept because it takes no column,
while the `a` it modifies is dropped.

## Root Cause

`LineEditor` (`src/line_editor.rs`) holds the input as `Vec<char>` and takes one `char` as one
step, for both cursor movement and deletion. `src/pager/status_line.rs` measures and cuts text the
same way: `char_width` per `char`, `clip` cutting between any two of them, and `cursor_cell`
approximating a cluster as "one character plus the zero-width run after it".

Cursor positions inside a cluster have no counterpart on screen, so no rendering strategy can show
them; and a cut inside a cluster, whether by clipping or by an escape sequence, changes how the
terminal draws the text around it.

A related inaccuracy comes from the same gap: `text_width` sums per-character widths, so
`👨\u{200D}👩\u{200D}👧` counts as 6 columns while it occupies 2. The search prompt uses that sum
to decide how much of the line it can keep, so it gives up more than it needs to.

## Plan

Handle text by grapheme cluster in both places. Add `unicode-segmentation` to find boundaries;
`unicode-width`, already a dependency, only measures columns and cannot segment.

### `src/line_editor.rs`

- `MoveCursorLeft` / `MoveCursorRight` jump to the previous / next cluster boundary, so the cursor
  can only rest on a boundary.
- `DeleteCharBeforeCursor` removes the whole cluster before the cursor.
- `AddChar` keeps inserting a single `char`: typing a combining mark after a base character has to
  extend the cluster rather than start a new one, which falls out of inserting at a boundary.
- `LineEditor::at_cursor` keeps returning an index-based split; what it gains is the invariant that
  the index always sits on a cluster boundary.

### `src/pager/status_line.rs`

- `cursor_cell` takes the whole cluster the cursor sits on, instead of the zero-width run after one
  character.
- `clip` cuts at cluster boundaries, so a base character and its marks are kept or dropped
  together.
- Measure widths per cluster rather than per `char`, so a joined emoji counts as the columns it
  occupies.

The internal `Vec<char>` may be worth revisiting as part of this: segmentation works on `&str`, so
holding a `String` plus a byte offset could avoid rebuilding a string for every query.

### Tests

- Unit tests in `src/line_editor.rs`: left/right over a decomposed `á` moves past the whole
  cluster, and Backspace removes it whole. Cover a ZWJ emoji too, since it is the same rule with
  more `char`s.
- Unit tests in `src/pager/status_line.rs`: the cursor cell covers a whole ZWJ sequence and a flag;
  clipping never leaves a leading zero-width character.
- An e2e test that types a decomposed character and moves the cursor over it, asserting the prompt
  always shows the cursor on a visible cell.
