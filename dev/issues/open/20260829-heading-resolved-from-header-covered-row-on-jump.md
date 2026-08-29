---
type: bugfix
tags: [heading]
status: doing
opened_at: 2026-08-29T05:05:34Z
---

## Overview

**Several `Pager` methods resolve the sticky heading from a row the global header covers
(`viewport.rows()[0]`, or a hardcoded line 0), so the pinned heading can belong to an earlier
section than the one actually visible.**

The heading is meant to stick to the first _visible content_ row, which is
`viewport.rows()[header.height()]` — `Pager::snapshot` builds content as
`&self.viewport.rows()[self.total_header_height()..]`, so rows `0..header.height()` are hidden
under the global header. `Pager::scroll_up` and `scroll_down` already resolve the heading against
`rows()[header.height()]`.

The bug was reported from `jump_to_end` and `jump_to_bottom`. A full sweep of
`heading.resolve` / `heading.resolve_if_found` call sites was then done; the inventory below is
complete as of that sweep (`src/pager.rs`, 2026-08-29).

### Affected sites

| Site                    | Line               | Reference used   | Reproduced                                                |
| ----------------------- | ------------------ | ---------------- | --------------------------------------------------------- |
| `Pager::jump_to_end`    | `src/pager.rs:354` | `rows()[0]`      | Yes — `jump_to_end_resolves_heading_below_header`         |
| `Pager::jump_to_bottom` | `src/pager.rs:364` | `rows()[0]`      | Yes — `jump_to_match_below_resolves_heading_below_header` |
| `Pager::pump_input`     | `src/pager.rs:285` | `rows().first()` | No — needs an under-filled viewport; see below            |
| `Pager::new`            | `src/pager.rs:173` | literal `0`      | No, and no observable symptom — see below                 |

Both reproduced cases live in `src/tests/heading.rs`, encode the _expected_ frame and are marked
`#[should_panic]`; the fix drops the attributes.

### Not affected

- **`Pager::jump_to` (`src/pager.rs:335`)** — resolves against the jump _target_ line, not a
  viewport row. That is correct by design and not an instance of this bug.
- **`Pager::scroll_up` (`src/pager.rs:425`)**, **`Pager::scroll_down` (`src/pager.rs:435-437`)** —
  already use `rows()[header.height()]`.

### `Pager::pump_input`

The streaming refill path resolves from `rows().first()`. It is a plain instance and is fixed
here along with the other sites.

`dev/issues/draft/heading-state-not-recomputed-on-resize.md` will later delete this `resolve`
call outright — the `relayout_page` call right above it resolves the heading correctly on its
own. That is not a reason to skip it now: the change is one line, the deletion is a trivial
conflict, and there is no schedule tying the two issues together.

**But the reference row must not be indexed raw here.** This branch only runs while
`viewport.rows().len() < viewport.size().height()`, i.e. while the viewport is under-filled, so
`rows()[header.height()]` can be out of bounds — unlike the jump sites, where the viewport is
full. Use `rows().get(header.height())` and skip the resolve when it is `None`: with no content
row below the header there is nothing for a heading to stick to.

### `Pager::new`

`Pager::new` calls `heading.resolve(&mut doc, 0)` before `Viewport::new`, so no viewport row is
available yet — the reference row would have to be read after the viewport is built.

**The state it produces is wrong, but the wrongness is not observable.** No reproduction exists,
and the analysis says none can:

- At startup the viewport top is line 0, so the correct reference line is exactly
  `min_line_index` (= `header.num_lines()`), whether or not header lines wrap.
- `resolve(doc, 0)` searches `min_line_index..1`, which is empty whenever a header is
  configured, so the heading is left unset.
- The only divergence is therefore when the **first visible content line is itself a heading
  start**. But at that position the heading's rows and the content rows they would replace are
  the same rows, so the rendered frame is identical whether or not the heading is set. Confirmed
  empirically: the startup frame equals the frame after `j` `k` (which does set the heading),
  down to the status line.
- Any move away from the top re-resolves: `scroll_down` via `resolve_if_found`, `jump_to*` via
  their own `resolve`. `scroll_up` keeps the existing heading, but cannot run at the top.

So this is a latent inconsistency, worth fixing for uniformity, not a user-visible bug.

## Reproduction

### `jump_to_end` (`G`)

`src/tests/heading.rs::jump_to_end_resolves_heading_below_header`.
`--heading '^# ' --heading-lines 1`, terminal 20x7, `--header 2`:

```
H1
H2
# A
a1
a2
# B
b1
b2
b3
b4
```

Pressing `G` (jump to end):

```
H1
H2
# A
b2
b3
b4
lines 5-10/10 100%
```

- **Shown** — `# A`.
- **Expected** — `# B`; the visible content `b2 b3 b4` belongs to section B.
- **Why** — the viewport top row after the jump is `a2` (line 4), hidden under the 2-row header.
  `heading.resolve` searches back from that hidden row and finds `# A` on line 2. `# B` on line 5
  is also hidden by the header, so the compensating scan in `push_up_heading_if_needed` (which
  starts at `header.height()`) never sees it, and no push-up correction happens either.

### `jump_to_bottom` (`n` onto a match below the page)

`src/tests/heading.rs::jump_to_match_below_resolves_heading_below_header`. Same options and
terminal size:

```
H1
H2
# A
az
# B
b1
b2
b3
zz
b4
```

`/z` matches `az` (line 3), which is already on screen, so nothing moves. Pressing `n` targets
`zz` (line 8), which is below the page, so `Pager::jump_to_next_match` routes through
`reveal_match` to `jump_to_bottom`:

```
H1
H2
# A            <- shown; "# B" expected
b2
b3
zz
lines 4-9/10 90%
```

- **Why** — `jump_to_bottom` anchors line 8's last row at the bottom, putting the viewport top at
  `az` (line 3). Rows 3 and 4 (`az`, `# B`) are covered by the 2-row header, so resolving from
  `rows()[0]` finds `# A` and never sees `# B`.
- The incremental search _preview_ does not reach `jump_to_bottom`; only `n` / `N` do
  (`Pager::jump_to_next_match` -> `reveal_match`). A repro has to submit the search first.

## Root Cause

`src/pager.rs`, `Pager::jump_to_end`:

```rust
pub fn jump_to_end(&mut self) -> PageUpdate {
    self.doc.pump();
    self.viewport.jump_to_end(&mut self.doc);

    let top_line_index = self.viewport.rows()[0].line_index();
    self.heading.resolve(&mut self.doc, top_line_index);
    self.push_up_heading_if_needed();

    PageUpdate::Full
}
```

`Pager::jump_to_bottom` has the identical pattern. Both index row `0`, which is under the global
header whenever `header.height() > 0`, instead of the first row below it.

## Plan

Use the row just below the global header as the reference row:

```rust
let top_line_index = self.viewport.rows()[self.header.height()].line_index();
self.heading.resolve(&mut self.doc, top_line_index);
self.push_up_heading_if_needed();
```

- **`jump_to_end`, `jump_to_bottom`** — apply directly.
- **`pump_input`** — apply, but via `rows().get(header.height())`; the viewport is under-filled
  on this path, so the index can be out of bounds.
- **`Pager::new`** — move the `resolve` call after `Viewport::new` so a reference row exists.
  Nothing observable changes; do it so the helper below is the only way to resolve a heading.

The repeated three-line sequence across four sites is itself a hazard: extracting a single
`Pager` helper (e.g. `resolve_heading_for_top_row`) would make the correct reference row the only
reachable one. Worth doing as part of this fix.

The two `#[should_panic]` tests in `src/tests/heading.rs` are the regression tests; drop the
attributes once the fix lands.

At the jump sites `rows()[header.height()]` can still be out of bounds on an empty or
under-filled viewport. That is a pre-existing, separately tracked concern
(`dev/issues/draft/empty-viewport-panics-on-key-input.md`) which already applies to `scroll_up`
and `scroll_down` using the same index today, so this fix does not need to add new guards there.
`pump_input` is the exception: under-filling is that branch's entry condition, not an edge case,
so it needs the `get` form noted above.
