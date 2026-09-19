---
type: bugfix
tags: [pager, search]
---

## Overview

**The search input cursor can be moved onto a position that has no cell on screen, where it
becomes invisible.**

A character and the combining marks that follow it render as a single cell, for example `a` +
`U+0301` shown as `á`. The search input moves the cursor one character at a time, so it can stop
between the two. There is no column for that position, so the cursor marker is drawn on a
zero-width cell and nothing appears on the prompt.

- Expected: every cursor position the arrow keys can reach is visible on the prompt.
- Actual: the cursor disappears while it sits on a combining mark, and reappears on the next
  move.

Decomposed text is not exotic: macOS file names are stored in NFD, so pasting one into the search
prompt is enough to produce this input.

The same character-at-a-time model makes Backspace delete only the combining mark of such a pair,
leaving the base character behind. That looks like the deletion did nothing, since `á` becomes `a`.

This predates the cursor being rendered as a position marker. While the cursor was a block
character inserted into the text, the same position instead attached the combining mark to the
block character itself and garbled it.

## Reproduction

1. Start the pager and press `/`.
2. Enter `á` in its decomposed form (`a` followed by `U+0301`), e.g. by pasting it from a macOS
   file name.
3. Press Left once.

The cursor is now on `U+0301` and is not drawn anywhere on the prompt. Pressing Left again puts it
on `a` and it becomes visible.

For the Backspace variant, do the same but press Backspace instead at step 3: the prompt shows `a`,
and the character count only goes from 2 to 1.

## Root Cause

`LineEditor` (`src/line_editor.rs`) holds the input as `Vec<char>` and treats one `char` as one
step: `MoveCursorLeft` / `MoveCursorRight` move by one element and `DeleteCharBeforeCursor` removes
one element.

A user-perceived character is a grapheme cluster, which can span several `char`s. Cursor positions
inside a cluster have no counterpart on screen, so no rendering strategy can show them:

- As a reverse-video marker (current), the cell to reverse is zero-width and invisible.
- As an inserted block character (previous), the block lands inside the cluster and the combining
  mark applies to the block instead of its base character.

The status line renders whatever position it is given, so the fix belongs in the editing model, not
in `src/pager/status_line.rs`.

## Plan

Make `LineEditor` move and delete by grapheme cluster instead of by `char`.

- Add `unicode-segmentation` and find cluster boundaries with it. `unicode-width`, already a
  dependency, only measures columns and cannot segment.
- `MoveCursorLeft` / `MoveCursorRight` jump to the previous / next cluster boundary, so the cursor
  can only ever rest on a boundary.
- `DeleteCharBeforeCursor` removes the whole cluster before the cursor.
- `AddChar` keeps inserting a single `char`: typing a combining mark after a base character has to
  extend the cluster rather than start a new one, which falls out of inserting at a boundary.

`LineEditor::cursor()` stays an index into the characters; the invariant it gains is that the index
always sits on a cluster boundary. The status line needs no change.

The internal `Vec<char>` may be worth revisiting as part of this: segmentation works on `&str`, so
holding a `String` plus a byte offset could avoid rebuilding a string on every segmentation query.

### Tests

- Unit tests in `src/line_editor.rs`: left/right over a decomposed `á` moves past the whole
  cluster, and Backspace removes it whole. Cover an emoji with a zero-width joiner too, since it
  is the same rule with more `char`s.
- An e2e test that types a decomposed character and moves the cursor over it, asserting the
  prompt always shows the cursor on a visible cell.
