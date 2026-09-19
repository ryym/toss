# Goal

Render the search prompt cursor as a position marker instead of a character occupying its own cell.

The cursor is drawn by reverse-videoing the cell it sits on:

- In the middle of the input: `/abc` with the cursor on `b` reverses `b`.
- At the end of the input: a trailing space is appended and reversed, which looks the same as today.

`LineEditor` stops producing display text and only exposes the input and the cursor position.
Composing the status line, including the cursor, becomes the status line module's job.

# Context

The search prompt currently renders the cursor by inserting a `█` character into the input text at the
cursor position. That happens to look right while the cursor is at the end (`/ab█`), but moving the
cursor to the start shows `/█ab`: the cursor pushes the characters aside instead of sitting on `a`.
A cursor is supposed to mark a position, not occupy a cell.

# Out of Scope

## Using the terminal's real cursor

Showing the real cursor (`cursor::Show` plus positioning it after each frame) is conceptually the most
faithful way to mark a position, and it matches `less`. It also lets the terminal's own cursor shape
and blinking settings apply.

It is not done here because it reaches much further than the status line: the `Screen` trait needs
cursor control, `Renderer` becomes responsible for placing the cursor every frame, and `MockScreen`
needs a way to express the cursor in its snapshots. Reverse video needs none of that and reuses the
escape handling the status line already has.

## Moving the cursor by grapheme cluster

`LineEditor` moves the cursor one character at a time, so it can stop between a base character and
a combining mark that follows it, for example between `a` and `U+0301` in the decomposed form of
`á`. That position has no cell of its own on screen: the two characters render as one. The cursor
then marks a zero-width cell and is not visible.

The same gap runs through the rendering as well, where a cluster can be cut by clipping or by the
escape sequences that highlight the cursor. It is tracked as a whole in
`dev/issues/draft/text-handling-ignores-grapheme-clusters.md`.

## Horizontally scrolling the search input

The status line is clipped to the screen width. When the input is longer than the screen and the
cursor is moved far to the left, the cursor falls outside the clipped range and its marker is simply
not drawn.

Keeping the cursor visible means replacing the current clipping with a cursor-following viewport,
which is a separate change, tracked in
`dev/issues/draft/search-input-does-not-scroll-horizontally.md`.
