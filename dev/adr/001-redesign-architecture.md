# ADR-001: Redesign Architecture from Scratch

- Status: Accepted
- Date: 2026-03-08

## Context

The original implementation of toss had an architecture where data caching and display state were tightly coupled.
Specifically, the `Pager` + `FilledPage` design conflated two concerns:

- **Data caching**: which lines are loaded from the source
- **Display state**: which lines are currently visible on screen

This coupling made it difficult to implement planned features such as sticky headers.
Adding these features would require significant refactoring of the core abstractions.

A prototype (`proto/`) was built to explore a cleaner architecture that separates these concerns.
The prototype successfully validated the new design by implementing smooth scroll animation and line wrapping.

## Decision

Replace the entire existing implementation with a new architecture based on the prototype design.
The old code is moved to `old/` during transition and will be deleted once the new implementation is stable.

### New Architecture

The new design separates the system into four distinct layers:

```
App (event loop, mode dispatch)
  |
  +-- Document (data layer)
  |     +-- LineBuffer: cached lines read from source
  |
  +-- ScreenState (display state)
  |     +-- Vec<ScreenRow>: what's currently shown on each screen row
  |     +-- scroll_down/up -> ScrollPlan (minimal diff for rendering)
  |
  +-- Screen (terminal abstraction + rendering)
  |     +-- Screen trait: abstract terminal operations
  |     +-- draw functions: apply ScrollPlan using terminal scroll + partial redraw
  |     +-- full redraw on resize or mode change
  |
  +-- ScrollAnimation (animation state)
        +-- easing function, start/target offset, timing
```

**Key design principles:**

1. **Separate data from display** - `Document` owns line data and knows nothing about the screen. `ScreenState` tracks what's on screen as indices into `Document`. rendering functions in `screen` translate plans into terminal commands.

2. **Incremental rendering** - Instead of full screen redraws (which cause flickering), use terminal scroll commands to shift existing content and only draw newly revealed rows. `ScreenState` computes a `ScrollPlan` as a minimal diff.

3. **Frame-driven event loop** - Instead of blocking on input, use a game-loop style: poll input (non-blocking), update animation state, render if needed, sleep until next frame. This enables smooth scroll animation while remaining responsive.

### Terminal Library Change

The new implementation uses `crossterm` instead of `termion`. This provides `poll(timeout)` for non-blocking event detection, which is essential for the frame-driven event loop.

## Consequences

### Positive

- Clean separation of data, state, and rendering makes each layer independently testable
- `Document` and `ScreenState` are pure logic, enabling straightforward unit testing without a terminal
- Incremental rendering via `ScrollPlan` eliminates flickering during fast scroll
- Frame-driven event loop naturally supports smooth scroll animation
- Architecture is ready for future features (search, sticky headers) without structural changes

### Negative

- Full rewrite discards the existing implementation
- Features from the old implementation (if any beyond basic paging) need to be reimplemented
- `crossterm` dependency replaces `termion`, requiring adaptation to a different API

### Scope

The initial implementation focuses on core paging with smooth scroll. The following features are deferred to future work:

- Incremental search with highlight
- Sticky header with pattern matching
- ANSI escape sequence passthrough
