---
type: development
tags: [pager, search]
---

## Goal

Keep the search input cursor visible when the input is wider than the screen, by scrolling the
prompt horizontally instead of always keeping its right side.

## Context

The status line is clipped to the screen width by dropping columns from the left, so the right
side of the search prompt is what survives. The cursor is drawn by highlighting the cell it
covers, and that cell shows up only while it falls inside what survives the clipping.

Once the cursor cell and the text after it are wider than the screen on their own, nothing to
their left is left to give up, and the prompt is shown with no cursor marker at all. Editing a
long query then gives no feedback about where the cursor is.

This is accepted deliberately for now: the alternative is a viewport that follows the cursor
instead of being anchored to the right edge of the line, which is a different model from the
clipping the status line uses everywhere else.

The gap is not new. While the cursor was rendered as a block character inside the text, clipping
dropped it in exactly the same situation.

## Acceptance Criteria

- With an input longer than the screen, moving the cursor anywhere in it keeps the cursor visible
  on the prompt.
- The text surrounding the cursor is visible with it, so editing does not turn into guesswork.
- Covered by tests that build the prompt at widths narrower than the input.

## Plan

Give the search prompt a viewport that contains the cursor, in place of the right-anchored
clipping it shares with the position indicator.

- Derive the visible range from the cursor position, or track an offset that is adjusted when the
  cursor would fall outside the current range.
- Scroll only when the cursor would otherwise leave the visible range, so the text does not shift
  on every keystroke.
- Leave the view-mode position indicator on `clip`: the right side is the useful end there, and
  nothing in it moves.
