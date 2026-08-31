# Architecture

This document summarizes an architecture overview of Toss, a terminal pager.

## Main Components

The application consists of the following main components and each component has its own clear responsibility.

- `App`
- `Pager`
  - `Document`
- `Renderer`
  - `Screen`
- `Line` and `Row`

### `App`

`App` is a controller that runs event loop, handle events, and cordinate `Pager` and `Renderer` to update screen based on terminal events.
An event loop basically looks like:

1. Poll events.
2. Call `Pager` to update the page state based on the event.
3. Call `Renderer` with the latest page state to render it to the terminal screen.

### `Pager`

`Pager` is a core state manager.
It decides which lines should be displayed, reads lines from `Document`, wrap lines as needed, and updates the page state.
But `Pager` itself never directly writes to the screen. It focuses on maintaining a correct state in memory.

#### Anchor and composition

The only page state `Pager` mutates is the **anchor**: the document row the viewport starts at.
Everything else — the global header, which heading is pinned, how far that heading has been pushed up, and the content rows —
is derived from the anchor by `pager::layout::compose`.
Every operation therefore reduces to picking an anchor and recomposing,
so no operation has to keep several pieces of state consistent with each other afterwards.

#### Sticky rows are an overlay

The header and the heading **cover** the first rows of the viewport rather than pushing them down.
This is what keeps scrolling uniform: advancing the anchor by one row always moves the visible content by exactly one row,
whether or not a heading appeared or disappeared in the same step.
It is also what allows the CSS's `position: sticky` transition between sections —
a heading that has scrolled into the covered band pushes the current one out row by row and takes over when it reaches the top of the band.

### `Renderer`

`Renderer` is responsible for rendering the page state to the terminal screen correctly and effectively.
It minimizes actual re-rendering to make screen updates as smooth as possible.
It applies the page state to `Screen` but never modifies the page state.

### `Document`

`Document` abstracts read operations for the target text paginated by Toss. It supports two backends:

- File: Lines are loaded on demand via byte-offset seeking. An LRU cache holds recently accessed parsed Line objects.
- In-memory (stdin or test strings): All lines are parsed upfront and held in memory.

It provides access to each `Line` by index in the document.

### `Screen`

`Screen` abstracts write operations for the terminal.
Tests use an in-memory mock screen to verify any `App`'s behavior without depending on its internal details.

### `Line` and `Row`

- `Line`: A sequence of text in `Document` delimited by line breaks (`\n`).
- `Row`: A segment of a line wrapped based on display width.

For example, the following 2 lines

```
abc-01234
hello-world
```

will be displayed like below when the screen width is 4:

```
abc-
0123
4
hell
o-wo
rld
```

Each logical line in the second text is `Row`. So the 2 lines are split into 6 rows in this case.
Wrap positions are determined based on plain text while Toss supports ANSI escape sequences in the original text.

## Frame-Driven Event Loop

Looking at the `App`'s event loop in more detail, it is a non-blocking game-loop like below:

1. Poll input with a timeout (short during scroll animation, longer when idle).
2. Handle the event if any (key press, resize).
   - Call appropriate operation of `Pager` to update the page state based on the event.
3. Advance scroll animation if one is running.
4. Render if the page state changed.
   - Each `Pager` operation reports whether anything changed; that is all the loop needs.
   - Pass the page state to `Renderer`, which works out what actually has to be written.

With this design, the application can handle user inputs while running smooth scroll animation.
For example, the user can speed up the scroll by repeating the key quickly during scroll.
