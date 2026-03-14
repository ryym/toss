# Architecture

## Overview

Toss is a terminal pager with a frame-driven event loop. The system is divided into four layers: data, display state, rendering, and animation.

```
App (event loop, mode dispatch)
  |
  +-- Document (data layer)
  |     +-- LineCache: cached lines read from source
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

## Key Design Principles

1. **Separate data from display** - `Document` owns line data and knows nothing about the screen. `ScreenState` tracks what's on screen as indices into `Document`. Rendering functions in `screen` translate plans into terminal commands.

2. **Incremental rendering** - Instead of full screen redraws (which cause flickering), use terminal scroll commands to shift existing content and only draw newly revealed rows. `ScreenState` computes a `ScrollPlan` as a minimal diff.

3. **Frame-driven event loop** - Instead of blocking on input, use a game-loop style: poll input (non-blocking), update animation state, render if needed, sleep until next frame. This enables smooth scroll animation while remaining responsive.

## Modules

| Module                | Responsibility                                              |
| --------------------- | ----------------------------------------------------------- |
| `app`                 | Event loop, input handling, mode management (View / Search) |
| `document`            | Owns source text, provides line access with wrapping        |
| `line_cache`          | Lazily caches lines read from stdin or file                 |
| `line` / `line_index` | Line representation and indexing utilities                  |
| `screen_state`        | Tracks visible rows, computes `ScrollPlan` diffs            |
| `screen`              | Terminal abstraction (`Screen` trait) and rendering         |
| `scroll`              | Scroll animation with easing                                |
| `search`              | Search logic, match tracking, navigation                    |
| `highlight`           | Search match highlighting within rendered lines             |
| `ansi`                | ANSI escape sequence parsing and passthrough                |
| `status_line`         | Status bar content and formatting                           |
| `line_editor`         | Simple line editor for search input                         |
| `logger`              | Debug logging to file                                       |
| `mock_screen`         | Test double implementing `Screen` trait                     |

## Terminal Library

Toss uses `crossterm` for terminal interaction. Its `poll(timeout)` function enables the non-blocking event detection essential for the frame-driven event loop.
