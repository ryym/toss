---
type: development
status: cancelled
opened_at: 2026-08-16T11:20:59Z
tags: [resize, heading]
---

## Reason for Cancel

**The behavior below is arguably more natural than what toss does today, but the spec does not
resolve to a single layout without introducing path dependence (see Trade-offs).**

- The cases it improves are narrow: they need `--heading` plus a resize that changes how many
  viewport rows the overlay covers.
- Recorded rather than deleted so the judgment does not have to be re-derived. Reopen if the
  current anchoring turns out to be annoying in practice.

## Goal

On a resize, keep the frame the user is looking at, rather than the viewport's top row — which,
with a sticky heading, may be a row hidden underneath the overlay.

## Context

`Viewport::resize` (`src/pager/viewport.rs:51-58`) preserves the viewport's top row. The heading
overlay is drawn over the topmost viewport rows, so that anchor can be invisible, and the rows
the user actually sees can shift even when nothing about them changed.

Two measured cases, both with `--heading '^# '`:

### A width change moves visible rows that did not reflow

`--heading-lines 2`, 12x7, scrolled so that `# B` sits directly below the heading, then resized
to 8x7. `# A` / `sub a` fit on one row at either width; only `long body 1` rewraps.

```
before (12x7)      today's anchor (8x7)   frame-preserving (8x7)
sub a              # A                    sub a
# B                sub a                  # B
sub b              # B                    sub b
body b1            sub b                  body b1
body b2            body b1                body b2
body b3            body b2                body b3
```

Nothing the user could see was reflowed, yet the content moves down a row and the title
reappears. The frame-preserving column is identical to the pre-resize screen.

### Growing the screen can hide a visible line

`--heading-lines 3`, 20x4 grown to 20x10. The small screen caps the heading at 2 rows; the large
one fits all 3, so the overlay covers one more row.

```
before (20x4)      today's anchor (20x10) frame-preserving (20x10)
# A                # A                    # A
a1                 a1                     a1
body a1            a2                     a2
                   body a2                body a1
                   # B                    body a2
                   ...                    # B
```

`body a1` was visible on the smaller screen and is swallowed by the taller overlay on the larger
one.

## Trade-offs

**Anchoring the content top does not determine a single layout.**

Today the layout derives in one direction:

```
viewport top T (preserved)
  -> viewport rows
    -> heading resolve -> full_height
      -> push-up: scan rows[0..full_height] for the next heading start at index i -> k = i
        -> content top = rows[k]
```

Anchoring the content top `C` instead inverts it: `T` is `k` rows above `C`, but `k` comes from a
scan whose window starts at `T`. The system has more than one self-consistent solution. For case
A above (`C` = `# B`, `full_height` = 2, width 8):

- **k = 2** — `T` = `long body 1` row 0. The scan window (`long bod`, `y 1`) holds no heading
  start, so push-up is 0 and `k` is 2. Consistent. Renders today's anchor column.
- **k = 1** — `T` = `long body 1` row 1. The scan window (`y 1`, `# B`) finds `# B` at index 1,
  so push-up is 1 and `k` is 1. Consistent. Renders the frame-preserving column.

A tie-break is therefore part of the spec, and the two candidates are unattractive:

- **Keep the pre-resize `k`.** Produces the desired frame, but the page stops being a function of
  (position, size): both states are stable at the same position and size, so which one is shown
  depends on how the user got there.
- **Maximize `k`.** Well-defined and history-free, but it selects today's layout, so nothing is
  gained.

**Scrolling has the same swallowing behavior.** The overlay grows and hides content on scroll
too, since it is inherent to the overlay model. Fixing it for resize alone would make the two
paths inconsistent, so a frame-preserving spec should cover scrolling as well — which widens
this from a resize fix into a layout-model change.

## Plan

If revisited:

- Choose the tie-break rule explicitly, and accept the path dependence it implies.
- Give `Viewport` a way to place a given row at a given row index (`jump_to` is close, but it
  anchors a line, not a wrapped row).
- Let `Pager` drive heading resolution and viewport placement as one pass, since neither can be
  computed before the other.
- Decide the same question for scrolling, so both paths follow one model.

## Related

- `dev/issues/draft/heading-state-not-recomputed-on-resize.md` — the push-up must be re-derived
  from the new geometry under either anchoring rule; that fix does not depend on this decision.
