---
type: bugfix
tags: [header, heading]
---

## Overview

**When `--header` is set to more lines than fit on the terminal, the lines that don't fit are
dropped silently, and a user who tries to jump straight to one of them can't get there —
`toss` sends them back to the top of the file instead.**

For example, with a 5-row terminal and `toss --header 5 file`, only 3 of the 5 requested
header lines actually fit and stay pinned at the top; the other 2 are quietly left out of the
header. If the user then jumps to one of those 2 lines directly (e.g. via search), `toss`
bounces them back to the very top of the file rather than showing the line. Scrolling normally
past it does eventually reveal it, but as an ordinary body line, not as the pinned header line
it was configured to be. In short: some lines the user asked to pin end up neither pinned,
nor showable on demand, nor discoverable except by accident.

This happens because of how `toss` internally decides which lines "belong" to the header:

- `Header::num_lines()` records what the header is *configured* to cover; `Header::height()`
  is how many rows actually got rendered once the cap kicked in. They diverge whenever
  `num_lines > viewport_height - 1`.
- `Header::contains(line_index)` compares against `num_lines`, not `height()`, so it reports
  `true` for lines the cap dropped and that never actually appear as header rows.
- `Pager::jump_to` uses `contains()` to decide whether to redirect to line 0:

  ```rust
  pub fn jump_to(&mut self, mut line_index: usize) -> PageUpdate {
      if self.header.contains(line_index) {
          line_index = 0;
      }
      ...
  ```

  So an explicit jump to a dropped header line always bounces back to the top. The only way
  to see such a line at all is to scroll past it, where it spills into an ordinary content row
  instead of a header row (`Viewport` builds rows sequentially from line 0 and never consults
  `Header::contains()`).

## Reproduction

```rust
#[test]
fn jump_to_dropped_header_line_bounces_to_top() {
    let doc = Document::from_string("H0\nH1\nH2\nH3\nH4\nbody1\nbody2\n".into());
    let opts = Options { header: 5, ..Default::default() };
    // Viewport height ends up 4 rows (5 screen rows minus the status line); the header
    // reserves 1 row for content, so only 3 of the 5 configured header lines render.
    let mut pager = Pager::new(doc, opts, ScreenSize::new(20, 5));

    pager.jump_to(4); // H4 was configured as a header line but never rendered as one.
    let (snap, _doc) = pager.snapshot();

    assert_eq!(line_indices(snap.header), vec![0, 1, 2]); // still capped at 3 rows
    // Expected: line 4 is now visible somewhere. Actual: jump_to redirected to line 0,
    // so this is the same view as the very first render.
    assert_eq!(line_indices(snap.content), vec![3]);
}
```

## Root Cause

`Header::contains` and `Heading`'s `min_line_index` both treat "configured header lines"
(`num_lines`) as equivalent to "lines the header actually occupies on screen". That
equivalence only holds when the header isn't capped. Once it is, the lines between what's
rendered (`height()`) and what's configured (`num_lines()`) fall into a gap no code path
accounts for:

- Not shown as header (rendering stopped at the cap).
- Not reachable via `jump_to` (`contains()` still claims them).
- Shown only incidentally as regular content when scrolled past, since `Viewport` doesn't
  know about `Header::contains()` at all.

### The gap widens if the header's and heading's row reservations ever diverge

`Header::build_rows` and `Heading`'s `HeadingConfig::new` each independently reserve one row
so neither one covers the full viewport:

```rust
// src/pager/header.rs
let max_height = size.height().saturating_sub(1);
```

```rust
// src/pager/heading.rs
let max_heading_height = size
    .height()
    .saturating_sub(global_header_height)
    .saturating_sub(1);
```

Today both reserve exactly `1`, and that coincidence is what keeps this bug partially
contained: whenever the header is capped, `max_heading_height` is forced to `0`, so `Heading`
never activates at the same time as a capped header. Bumping the header's reserve to `2`
(heading's left at `1`) breaks that: the header still caps, but `max_heading_height` becomes
`1`, so a heading can resolve while the header is capped. When that happens, the dropped
header lines don't even spill into content anymore — they vanish from the screen entirely,
while remaining unreachable via `jump_to`:

```rust
#[test]
fn capped_header_with_active_heading_loses_lines() {
    // Demonstrates the compounded failure. Requires bumping the header's reserved-row
    // count from 1 to 2 in `Header::build_rows` to make the state reachable (heading's
    // own reserve stays 1) — with both reserves equal to 1, as they are today, this state
    // cannot occur.
    let doc = Document::from_string("H0\nH1\nH2\nH3\nH4\n# heading\nbody1\nbody2\n".into());
    let opts = Options {
        header: 5,
        heading: Some(heading_opts("^# ", 1)),
        ..Default::default()
    };
    let mut pager = Pager::new(doc, opts, ScreenSize::new(20, 6));
    pager.scroll(2);
    let (snap, _doc) = pager.snapshot();

    let shown: Vec<usize> = line_indices(snap.header)
        .into_iter()
        .chain(line_indices(snap.heading))
        .chain(line_indices(snap.content))
        .collect();
    assert!(shown.contains(&3)); // H3: fails once the reserves diverge
    assert!(shown.contains(&4)); // H4: fails once the reserves diverge
}
```

So this is one gap, not two coincidentally related bugs: capped-but-configured header lines
have nowhere to render. Its visible symptom today is a `jump_to` misdirect; it would silently
turn into outright data loss on screen the moment the header's and heading's reserved-row
constants stop matching.

## Outcome

- A header line the cap drops from rendering is either genuinely reachable (as header,
  heading, or content — pick one, consistently) or the pager makes it clear such lines are
  intentionally never shown.
- `jump_to` no longer redirects to line 0 for a line that isn't actually covered by the
  rendered header.
- The header's and heading's reserved-row amounts no longer need to coincidentally match for
  correctness — or if they must, that's enforced (e.g. a shared constant) rather than left
  implicit.

## Plan

Not decided. Candidate directions:

- **Make `Header::contains` (and anything gating on it) reflect what's rendered, not what's
  configured** — compare against something derived from the capped row count rather than
  `num_lines`. Simplest, but changes what "part of the header" means when capped, and needs a
  decision on where the dropped lines *do* show up (content, most likely, consistent with
  today's incidental spillover).
- **Never let the header actually drop configured lines** — e.g. clamp `num_lines` itself to
  what fits, either silently or with a startup warning, so `contains()`/`num_lines()`/rendered
  rows always agree by construction. Removes the gap instead of patching each consumer, but
  changes the user-visible behavior of the `--header` option when given a count too large for
  the terminal.
- **Extract the "reserve one row" policy into one shared place** used by both
  `Header::build_rows` and `Heading`'s `HeadingConfig::new`, so the two can't drift apart even
  if neither stays fixed at `1` forever. Addresses only the compounding risk, not the
  underlying `jump_to` gap by itself — would need pairing with one of the above.

Whichever direction is chosen should add regression coverage at the `Pager` level (not just
`Header`/`Heading` unit tests) for the case of `header` configured larger than the viewport
can render, both with and without an active heading, since that is exactly where this gap
lives.
