# ADR-003: Remove DECSTBM and incremental scroll in favor of full page redraw

## Status

Accepted

## Context

Toss used DECSTBM (DEC Set Top and Bottom Margins, `CSI Ps ; Ps r`) to implement incremental scrolling with sticky headers.
The rendering pipeline worked as follows:

1. Set scroll region to exclude header and status line rows
2. Issue terminal scroll command (`ScrollUp`/`ScrollDown`) to shift content within the region
3. Reset scroll region
4. Redraw only the "dirty" rows (newly revealed content)

This approach avoided full screen redraws by letting the terminal hardware-scroll unchanged rows,
and DECSTBM ensured the sticky header stayed in place during the scroll.

However, this added significant complexity:

- The `Screen` trait required `set_scroll_region` and `reset_scroll_region` methods
- `MockScreen` needed to simulate scroll regions for testing
- Multiple rendering paths existed: `draw_full_page`, `apply_scroll`, and `apply_scroll_and_redraw_header`
- The app layer needed to track section changes and header height changes to choose the right rendering path
- `ScrollPlan` and `Direction` types existed solely to carry incremental rendering parameters

## Decision

Remove DECSTBM and terminal scroll entirely. Always use full page redraw (`draw_full_page`) after every scroll.

### Why full redraw doesn't cause flickering

Modern terminal emulators buffer output and render it in a single frame when the application flushes.
Since toss queues all drawing operations via `crossterm::queue!` and issues a single `flush()` at the end,
intermediate states are never visible to the user. Manual testing confirmed no perceptible flickering.

An earlier prototype did observe flickering with full redraws,
but that was likely due to an implementation issue (e.g. multiple flushes per frame) rather than a fundamental limitation of the approach.

## Consequences

### Positive

- Removed ~250 lines of code across 5 files
- Single rendering path (`draw_full_page`) instead of three
- `Screen` trait is simpler: no scroll region or terminal scroll methods
- `MockScreen` no longer needs to simulate scroll regions
- `ScrollPlan` and `Direction` types eliminated
- Easier to reason about rendering behavior

### Negative

- Every scroll frame redraws all visible rows, which is more work than redrawing only dirty rows.
  This has not been a problem in practice, but could matter on very large terminals or slow terminal emulators.

### Mitigations

- To prevent tearing in environments where output may be split across multiple `write()` syscalls (e.g. SSH, tmux/screen),
  toss wraps each rendering cycle with Synchronized Output (DEC Private Mode 2026: `CSI ? 2026 h` / `CSI ? 2026 l`).
  Supporting terminals (iTerm2, kitty, WezTerm, foot, etc.) buffer all output between these markers and render atomically.
  Non-supporting terminals simply ignore the sequences.
- `EndSynchronizedUpdate` is also sent in `TermScreen::drop` to ensure the terminal exits synchronized mode on error or panic.
