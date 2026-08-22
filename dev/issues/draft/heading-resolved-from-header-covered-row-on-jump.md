---
type: bugfix
tags: [heading]
---

## Overview

**`Pager::jump_to_end` and `Pager::jump_to_bottom` resolve the sticky heading from
`viewport.rows()[0]`, a row the global header covers, so jumping (`G`, or a search match handled
via `jump_to_bottom`) can pin a heading from an earlier section than the one actually visible.**

The heading is meant to stick to the first *visible content* row, which is
`viewport.rows()[header.height()]` — `Pager::snapshot` builds content as
`&self.viewport.rows()[self.total_header_height()..]`, so rows `0..header.height()` are hidden
under the global header. `Pager::scroll_up` and `scroll_down` already resolve the heading against
`rows()[header.height()]`. `jump_to_end` and `jump_to_bottom` are the sites this issue was
reported from, but they are examples, not a verified inventory: other call sites may resolve the
heading from `rows()[0]` (or the equivalent `rows().first()`) too. Search for the current set
when fixing, e.g. `rg 'heading\.resolve' src/`, and judge each hit on its own — resolving against
a jump target rather than a viewport row, as `jump_to` does, is correct and not an instance of
this bug.

## Reproduction

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

Apply this to `jump_to_end`, `jump_to_bottom`, and any other site the search turns up that
resolves the heading from the viewport's top row. Some hits may belong to another issue — if a
site's resolve is being moved or removed elsewhere, leave it to that issue instead of patching it
here.

Add a regression test (e.g. in `src/tests/heading.rs`) that jumps to the end while a header and
heading are both configured and asserts the heading matches the section actually visible, along
the lines of the reproduction above.

`rows()[header.height()]` can be out of bounds on an empty or under-filled viewport; that is a
pre-existing, separately tracked concern
(`dev/issues/draft/empty-viewport-panics-on-key-input.md`) that already applies to the other
call sites using this same index today, so this fix does not need to add new guards for it.
