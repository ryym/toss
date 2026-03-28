# ADR-004: Section Header Display via Viewport Overlay

- Status: Accepted
- Date: 2026-03-28

## Context

Toss supports two kinds of sticky headers:

- **Fixed header** (`--header N`): The first N lines of the document, always pinned at the top. Static and known at startup.
- **Section header** (`--section REGEX --section-header N`): A pattern-matched header that changes dynamically as the user scrolls through different sections.

The fixed header is straightforward: the viewport simply starts below the header lines (`min_top_line`),
and the content area height is reduced by the header's screen row count. The viewport never needs to know about the header.

Section headers are harder. They appear and disappear depending on scroll position,
and the header height can vary due to line wrapping and the push-up effect (see below).

The key design question is: **how should the section header's screen space relate to the viewport's row tracking?**

## Decision

Use an **overlay model**: the section header is drawn on top of the viewport's first N rows, rather than resizing the viewport when a section becomes sticky.

```
┌──────────────────┐
│ Fixed Header     │  ← outside viewport (min_top_line)
├──────────────────┤
│ Section Header   │  ← overlays viewport row 0
│ Section Header   │  ← overlays viewport row 1 (when section header spans 2+ rows)
│ Viewport Row 2   │  ← first visible row (overlay = 2 in this example)
│ Viewport Row 3   │
│ Viewport Row 4   │
├──────────────────┤
│ Status Line      │
└──────────────────┘
```

### How it works

- **Viewport height stays constant** regardless of whether a section header is sticky.
  The sizing formula absorbs the overlay: `content_height = screen_height - 1 - (header_height - overlay)`.
  The section header rows added to `header_height` are cancelled out by the same `overlay` value,
  so the viewport size does not change when a section becomes sticky.
- **Rendering skips overlaid rows.** `draw_full_page` draws header rows at y=0 and then viewport rows starting from `rows[overlay..]`.
- **`cached_overlay`** in `Header` tracks how many viewport rows are currently overlaid. It is recomputed on each `resolve()` call.

### Push-up effect

When the next section approaches the top of the viewport, the current section header is gradually "pushed up" — fewer rows of it are displayed.
This emerges naturally from the overlay model: `current_header_visible_rows = min(current_header_rows, distance_from_top_to_next_section)`.
No special state machine is needed.

### Interaction with search jumps

When jumping to a search match, the target line must be placed below the overlay, not at viewport row 0.
`Page::jump_to_visible` handles this by iteratively scrolling up until the target row index exceeds the overlay.
The loop is necessary because with multi-line section headers, scrolling up changes the push-up distance and thus the overlay value.
The loop converges within at most as many iterations as the section header's row count.

## Alternatives Considered

### Resize model: shrink viewport when section header appears

Instead of overlaying, resize the viewport each time a section header becomes or stops being sticky.
The viewport would always contain only truly visible rows.

Rejected because:

- **Push-up requires continuous resizing.** As the next section approaches, the header shrinks one row at a time.
  Each step would trigger a viewport resize and rebuild.
- **State transitions are complex.** The viewport height depends on whether a section is sticky,
  which depends on the viewport top, which depends on the viewport height.
  This circular dependency requires careful stabilization that the overlay model avoids.

**Re-evaluation after ADR-003:** The removal of DECSTBM and incremental scroll (ADR-003) weakened both rejection reasons above.
Since every scroll now triggers a full page redraw, viewport resizing is no longer expensive (just a rows vector rebuild),
and the circular dependency is already handled by the `sync_section_for_redraw` convergence loop. The resize model is now viable.

Its main advantage would be **mental model simplicity**:
`viewport.rows()` would always correspond exactly to what the user sees, eliminating the overlay concept entirely.
This would simplify `draw_full_page` (no row skipping), `jump_to_visible` (no iterative overlay correction), and general reasoning about visible content.

However, the overlay model's complexity is already well-localized (a single `usize`, a slice skip, and one iterative loop),
so the mental model benefit alone does not justify the migration cost.

### Separate header viewport

Manage the section header in its own dedicated viewport, completely independent of the content viewport.

Rejected because:

- **Push-up still needs coordination.** The header viewport's height affects where the content viewport starts,
  reintroducing the same coupling the overlay model avoids.
- **More abstraction for no clear benefit.** The overlay model is a lightweight mechanism (a single `usize` and a slice skip) that handles all cases.

## Consequences

### Positive

- Viewport height is stable — no mid-scroll resizing or viewport rebuild on section transitions.
- Push-up is a natural consequence of limiting overlay rows, not a separate feature.
- Minimal state: one `cached_overlay: usize` field, recomputed on each header resolve.

### Negative

- Any operation that positions the viewport by line index (e.g., search jump, `g`/`G` in the future) must account for overlay.
  The viewport's row 0 is not necessarily the first visible row. This was the source of two bugs (1c448c4, 12f4d0d).
- Reasoning about "visible rows" requires understanding that `viewport.rows()[overlay..]` is what the user actually sees,
  not `viewport.rows()` directly. This is a recurring mental model cost for anyone working on rendering or positioning logic.

### Mitigations

- `Page::jump_to_visible` centralizes the overlay-aware positioning logic so call sites don't need to think about it.
- `Page::sync_section_for_redraw` ensures viewport sizing and overlay are consistent before full redraws.
