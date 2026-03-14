# Architecture

## Layers

The system has four layers. Each layer has a single responsibility and depends only on the layer(s) below it.

```
App          Event loop, mode dispatch, animation coordination
ScreenState  What is currently displayed (row-level bookkeeping)
Document     Line data (loading, caching, wrapping)
Screen       Terminal I/O abstraction
```

- **Document** loads lines from a file or stdin and owns the parsed data. It knows nothing about the screen.
- **ScreenState** tracks which document lines (and which wrap segments) occupy each screen row. It computes a **ScrollPlan** — a minimal diff describing what changed — but never touches the terminal itself.
- **Screen** is a trait that abstracts terminal operations. Production code uses crossterm; tests use an in-memory mock. Rendering functions live alongside the trait and translate a ScrollPlan into terminal commands.
- **App** ties everything together: it runs the event loop, dispatches input by mode, drives animations, and decides when to render.

## Frame-Driven Event Loop

Instead of blocking on input, App runs a game-loop:

1. Poll input with a timeout (short during animation, longer when idle)
2. Handle the event if any (key press, resize)
3. Advance animation if one is running
4. Render if the screen state changed

This design exists because smooth scroll animation requires rendering intermediate frames between user inputs. A blocking-input loop cannot do this.

## Incremental Rendering via ScrollPlan

Full screen redraws cause visible flicker. To avoid this, scrolling works incrementally:

1. ScreenState receives "scroll N rows down/up"
2. It shifts its internal row array and fills in the newly exposed rows
3. It returns a ScrollPlan: the direction, how many rows shifted, and the new row contents
4. The rendering function issues a terminal scroll command (which shifts existing content in-place) and only draws the newly revealed rows

Full redraws happen only on resize or mode transitions.

## ANSI-Aware Wrapping and Highlighting

Source lines may contain ANSI escape sequences. The design principle is: **width calculation and search operate on escape-stripped plain text, while rendering preserves the original raw text.** `Line` maintains both views and a byte-level mapping between them (see `line.rs` for details).

This duality surfaces in two cross-cutting concerns:

- **Wrapping**: Wrap positions are determined by display widths on plain text, then translated to raw byte offsets. ScreenState tracks which wrap segment of which line occupies each screen row. When rendering, wrapped rows from the same logical line are written as a single continuous string so the terminal handles line breaks as soft wraps.
- **Search highlighting**: Matches are found on plain text, then the match ranges are mapped to raw text positions where escape sequences for highlight are injected.

## Mode System

App has two modes: **View** and **Search**.

- **View**: Normal paging. Keys scroll, 'n'/'N' navigate search matches, '/' and '?' enter search mode.
- **Search**: A line editor captures the query. Each keystroke triggers an incremental search from the current position, updating a **preview** highlight. On Enter the preview becomes the committed search state. On Esc the screen returns to the saved position and the preview is discarded.

The committed search state (`App.search`) persists across mode transitions and is used for 'n'/'N' navigation and rendering highlights in View mode. The preview state exists only inside the Search mode variant.

## Data Loading

Document supports two backends:

- **File**: Lines are loaded on demand via byte-offset seeking. A LineIndex (built once at open by scanning newlines) enables O(1) access to any line. An LRU cache holds recently accessed parsed Line objects.
- **In-memory** (stdin or test strings): All lines are parsed upfront and held in memory. No caching needed.

## Testing

The Screen trait enables end-to-end testing without a terminal. MockScreen records an in-memory grid and tracks soft-wrap markers. Tests inject a sequence of key events, run the app to completion (a 'q' key), and assert on grid snapshots. ANSI escapes in snapshots are visualized as readable tags (e.g., `{reverse}`) for easy comparison.
