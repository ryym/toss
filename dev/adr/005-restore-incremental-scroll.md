# ADR-005: Restore incremental scroll rendering

- Status: Accepted
- Date: 2026-03-28
- Supersedes: [ADR-003](003-remove-decstbm-and-incremental-scroll.md) (partially)

## Context

ADR-003 replaced incremental terminal scroll with full page redraw on every scroll frame.
The rationale was that modern terminal emulators buffer output and render atomically on flush,
so full redraws should not cause visible flickering.

In practice, flickering was observed when scrolling quickly through content with background colors.
Even with Synchronized Output (DEC Private Mode 2026) wrapping each frame, some terminals
did not render the full redraw atomically enough to prevent visible flicker.

ADR-003 also noted that "an earlier prototype did observe flickering with full redraws,
but that was likely due to an implementation issue." This turned out to be incorrect —
the flickering is inherent to full redraws with colored content, not an implementation bug.

## Decision

Restore incremental scroll rendering: issue a terminal scroll command to shift existing content
in-place, then redraw only the newly revealed rows and the header.

### What is kept from ADR-003

- **DECSTBM removal**: The old code used scroll regions (`CSI Ps ; Ps r`) to protect the header
  during terminal scroll. The restored code scrolls the entire screen and redraws the header
  afterwards, which is simpler and avoids the `set_scroll_region`/`reset_scroll_region` complexity.
- **Synchronized Output**: Each rendering cycle (both incremental and full) is still wrapped
  with DEC Private Mode 2026. This provides an additional layer of atomicity on top of
  incremental rendering.

### What is restored

- `ScrollPlan` and `Direction` types in viewport, returned by `scroll_down`/`scroll_up`
- `scroll_terminal` method on the `Screen` trait
- `apply_scroll`/`apply_scroll_no_flush` rendering functions
- `Page::plan_scroll` (previously `Page::scroll`) returning `Option<ScrollPlan>`
- App uses incremental rendering for normal scrolls, falling back to full redraw only when
  the header height changes

### Rendering paths

The system now has two rendering paths again:

1. **Full redraw** (`draw_full_page`): Used on initial draw, resize, mode transitions,
   search jumps, and when header height changes during scroll.
2. **Incremental scroll** (`apply_scroll`): Used for normal line-by-line and page scrolling.
   Issues terminal scroll, redraws dirty rows, redraws header and status line.

## Consequences

### Positive

- No visible flickering during fast scrolling with colored content
- Less data written per scroll frame (only dirty rows instead of all rows)

### Negative

- Two rendering paths instead of one, increasing code complexity (~100 lines added back)
- `MockScreen` must simulate terminal scroll (shifting the grid), making tests slightly
  more complex
- Future rendering changes must consider both paths

### Lessons learned

- Terminal output buffering is not a guarantee of flicker-free rendering, even with
  Synchronized Output. Content with background colors is especially sensitive because
  the terminal must clear and repaint colored regions, making partial frame visibility
  more noticeable.
- The simplicity benefit of full redraw was real but not worth the visual regression.
  When in doubt, prefer the approach that produces better visual output.
