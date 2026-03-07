# Toss Proto - Design Document

## Goal

Build a minimal terminal pager ("toss") from scratch with a cleaner architecture
that separates data management from display concerns. This prototype explores
a new design before potentially replacing the current implementation.

## Target Features (Full Vision)

1. **Smooth scroll** - Animated scrolling with easing for page navigation
2. **Incremental search** - Highlight matches as you type
3. **Sticky header** - Pin a pattern-matched header line at the top while scrolling
4. **Vi-like keybindings** - j/k/g/G/d/u/f/b navigation

We do NOT aim to be a full `less` replacement. No multi-file support, no marks, etc.

## Architecture Overview

```
App (event loop, mode dispatch)
  |
  +-- Document (data layer)
  |     +-- LineBuffer: cached lines read from source
  |     +-- (future) SearchIndex: search match positions
  |
  +-- ScreenState (display state)
  |     +-- Vec<ScreenRow>: what's currently shown on each screen row
  |     +-- scroll_down/up -> ScrollPlan (minimal diff for rendering)
  |
  +-- Renderer (terminal output)
  |     +-- applies ScrollPlan using terminal scroll + partial redraw
  |     +-- full redraw on resize or mode change
  |
  +-- ScrollAnimation (animation state)
        +-- easing function, start/target offset, timing
```

### Key Design Principle: Separate Data from Display

The current implementation's `Pager` + `FilledPage` conflates data caching
(which lines are loaded) with display state (which lines are visible on screen).
This makes it hard to add features like sticky headers.

In this design:

- **Document** owns line data. It knows nothing about the screen.
- **ScreenState** tracks what's on screen as indices into Document.
  It computes scroll diffs (ScrollPlan) but does no I/O.
- **Renderer** translates ScrollPlans into terminal commands.

### Incremental Rendering

Full screen redraws cause flickering on fast scroll. Instead:

1. Use terminal scroll commands to shift existing content
2. Only draw newly revealed rows
3. ScreenState tracks the before/after state to compute the diff

### Frame-Driven Event Loop

Instead of blocking on input, use a game-loop style:

1. Poll input (non-blocking)
2. Update animation state
3. Render if needed
4. Sleep until next frame

This allows smooth scroll animation while remaining responsive to input.
crossterm's `poll(timeout)` provides non-blocking event detection.

## Scope of This Prototype

### Phase 1 (current target): Smooth scroll

- [x] Project setup
- [ ] Document: read file into lines
- [ ] Screen trait: abstract terminal operations
- [ ] ScreenState: track displayed rows, compute ScrollPlan
- [ ] Renderer: full redraw + incremental scroll rendering
- [ ] App: frame-driven event loop with key handling
- [ ] Smooth scroll animation (d/u/f/b with easing)
- [ ] Line wrapping

No search, no sticky header, no stdin support, no ANSI escape handling.
Keep it simple to validate the architecture.

### Phase 2 (future): Search + Sticky Header

- Incremental search with highlight
- Sticky header with pattern matching
- ANSI escape sequence passthrough
- stdin support

## Module Structure

```
src/
  main.rs          - entry point, file reading, terminal setup
  app.rs           - event loop, input dispatch, animation coordination
  document.rs      - line storage and access
  screen.rs        - Screen trait + crossterm implementation
  screen_state.rs  - display state tracking, ScrollPlan computation
  render.rs        - translates ScrollPlan into terminal draw commands
  scroll.rs        - ScrollAnimation, easing functions
  line.rs          - Line type with wrapping support
```

## Development Approach

- Write tests for Document, ScreenState, and ScrollAnimation
  (these are pure logic, easy to test without a terminal)
- Commit frequently at each logical step
- Renderer and App are harder to unit test; verify manually
