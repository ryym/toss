# Goal

Let users move between the sections defined by `--heading`, not only see the current one pinned.

- `)` jumps to the next section heading.
- `(` jumps to the previous section heading.

A jump lands at the start of the section: the heading is pinned and the content starts right after it.
Like `g` / `G` / `n` / `N`, it is an instant jump without scroll animation.

The target is decided relative to the heading currently pinned, so the result always agrees with what the user sees:

- `)` goes to the next heading after the pinned one.
- `(` in the middle of a section (including the middle of a multi-line heading or of a wrapped heading line)
  goes back to the start of that section first. Pressing it again goes to the previous section.

"The heading currently pinned" is found from the row the pinned heading covers:
the pinned heading overlays the top rows rather than pushing the content down,
so the row it would show without the overlay is what the user is reading.
Using the first row visible below the pinned heading instead would break the agreement:
when the next heading is right there, `)` would skip it and go to the one after.

Add this behavior to `dev/specs/sticky-header.md` as well, from the user's point of view,
alongside the rest of the sticky header behavior.

# Context

`--heading` already makes sections visible by pinning their headings (see `dev/specs/sticky-header.md`),
but moving between them still requires scrolling or searching. Headings are a natural unit of navigation
for the inputs `--heading` targets, such as Markdown documents, `git diff`, and `git log`.

`(` and `)` are used in `less` for bracket matching, which is rarely used, so Toss takes them for this.

Behaviors decided on purpose:

- `(` in the middle of a section returns to the start of that section rather than going to the previous one,
  even when the position is inside the heading block itself.
  In that state the start of the section is hidden under the pinned heading, and `(` is the way to reveal it.
- When no heading exists in the direction, nothing happens.
  Unlike vim's `[[` / `]]`, the page does not move to the document start / end;
  moving without landing on a heading makes it unclear where the user is. `g` / `G` already cover that.
- Near the document end, the page cannot scroll a heading on the last page up to the top.
  The jump lands with the heading somewhere in the page, and headings further down the last page
  cannot be reached with `)`. They are already visible, so the jump just stops there.
- As a rare edge case, the screen can be too small to show any heading
  (e.g. `--header` fills nearly the whole viewport). Jumps still work there and bring the heading line
  to the top of the content. This follows from the normal jump logic with no special handling,
  and is not worth mentioning in the spec.

Known limitation:

- At the end of a streaming input, a line can look like a heading start only because the lines after it
  have not arrived yet (with `--heading-lines` 2 or more, a later match within the block disqualifies it).
  `)` may land on such a line and it may stop being a heading right after.
  The sticky heading already behaves the same way, so the jump follows it.

# Out of Scope

## Documenting key bindings

Key bindings, including existing ones, are not documented in README or help text yet.
That is handled separately for all keys together.
