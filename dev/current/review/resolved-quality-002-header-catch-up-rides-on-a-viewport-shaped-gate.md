# The header only catches up to its configured lines through a viewport-shaped gate

Review target: `7f909cc..134a6c4` (`src/pager.rs`, `Pager::pump_input` / `header_height_stays_below_num_lines_while_document_is_still_shorter`)

## Summary

**The new test pins that a streamed header grows to its configured `num_lines` as lines arrive,
but that growth is a side effect of a condition about the *viewport*, not about the header.**

- `Header` rebuilds its rows only inside `relayout_page`.
- In the streaming path, `relayout_page` runs only while the viewport is not yet full.
- Nothing states why "viewport not full" is the right trigger for "header still short".

## The gate

`src/pager.rs`, `Pager::pump_input` (L274-287):

```rust
    pub fn pump_input(&mut self) -> Option<PageUpdate> {
        let result = self.doc.pump();

        // Fill the first screen from the top while it is not yet full.
        if result.grew && self.viewport.rows().len() < self.viewport.size().height() {
            self.relayout_page(*self.viewport.size());
```

The new test (`src/pager.rs` L937-960) depends on this branch firing:

```rust
        // Once the rest of the document arrives, the header reaches its configured size.
        send_lines(&tx, 2, 3);
        pager.pump_input();
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0, 1, 2]);
```

Its comment explains the viewport height as "far more than a 3-line header needs" — i.e. the
test is deliberately set up so the viewport stays unfilled. Nothing covers the other side.

## Why it holds today

The two conditions are not the same, but the mismatch is currently unobservable:

- **Header short of `num_lines` + viewport full** requires the existing lines to render at least
  `viewport height` rows, i.e. header lines that wrap a lot.
- **`Header::build_rows` caps at `viewport height - 1`**, so in exactly that situation the header
  is capped too.
- **A capped header would not render the late-arriving line anyway**, so skipping the rebuild
  changes nothing on screen.

That is the same style of coincidence argument already recorded for `min_line_index` in
`dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md`, and it leans on the
capped-header behaviour tracked in
`dev/issues/draft/capped-header-lines-become-unreachable.md`.

## Open question

Should the header's staleness be expressed on its own terms rather than inherited from the
viewport's fill state?

- **Record the argument** — at minimum, note next to the gate (or in the new test) why
  "viewport not full" covers the header case, so a future change to either the gate or the
  header cap does not silently break it.
- **Give the header its own trigger** — e.g. rebuild header rows whenever the document grew and
  `header.height()` is still below what `num_lines` could produce, independent of the viewport.
- **Leave it** — if the capped-header work removes the cap-induced divergence entirely, the
  question dissolves with it.

## Assessment

- Newly introduced issue? No
- Does it block the overall goal? No

**The observation is accurate, but it describes an existing property of `pump_input`, not
something the reviewed range introduced.** The gate and `relayout_page` predate `7f909cc`; the
only new thing is a test (`317a29d`) whose subject — the header catching up to `num_lines` —
happens to be produced by that gate.

Nothing is broken today, and the gap the review points at is narrower than "two conditions that
might diverge". It is a single load-bearing assumption, and it belongs to the capped-header work.

### The invariant that makes the gate correct

"A configured header line exists but is not rendered, and a rebuild would render it" cannot
coexist with a full viewport. The chain:

1. **A missing header line means `doc.line_count() < num_lines`.** `build_rows` renders
   `0..num_lines`, and a streamed `Document` receives lines in order, so "some header line has
   not arrived" is the same statement as "the whole document is shorter than `num_lines`".
2. **So every row the pager can lay out comes from a header line.** There are no other lines to
   lay out.
3. **So a full viewport means those header lines already render `>= size.height()` rows.**
4. **So the header is past its `size.height() - 1` cap**, and the late-arriving line would not
   be rendered even if `relayout_page` did run.

Note that step 1 is what the new test exercises; the *other* cause of `height() < num_lines` —
the cap itself, with the whole document present — has nothing to catch up on, since a rebuild
would reproduce the same rows.

### What would break it

Only step 4, and only under one specific change: **letting the header occupy `size.height()`
rows or more.** Tightening the reserve (`height - 2`) or loosening it (`height`) both keep
`cap <= size.height()`, which is all steps 3-4 need. Of the directions listed in
`dev/issues/draft/capped-header-lines-become-unreachable.md`, only "never let the header drop
configured lines", if implemented by rendering all `num_lines` lines regardless of the row
budget, removes the row-based cap and breaks the chain. The resulting failure:

```
screen 20x10 (viewport height 9), --header 3
  line 0: 100 chars -> 5 rows
  line 1: 100 chars -> 5 rows   => viewport rows = 9, full, gate closed
  line 2 arrives -> pump_input returns StatusOnly, no relayout
                 -> header should now grow, but stays at its old rows
                    until the next resize or scroll
```

The new test does not catch this: it is set up with a 3-line header in a 9-row viewport, i.e.
squarely on the gate-open side.

Step 1 has a second, more remote dependency: lines arriving in order. If `Document` ever
accepted sparse or out-of-order input, "header line missing" would stop implying "document
short" and the chain would break at the top instead.

## Plans

### Plan 1: Record the dependency in the capped-header issue

Add the gate to `dev/issues/draft/capped-header-lines-become-unreachable.md`, next to its
existing "### The gap widens if the header's and heading's row reservations ever diverge"
section, since it is the same kind of hidden dependency on the cap:

````markdown
### The streaming fill gate also leans on the cap

`Pager::pump_input` rebuilds the header only through `relayout_page`, and that call is gated on
the viewport not yet being full:

```rust
if result.grew && self.viewport.rows().len() < self.viewport.size().height() {
    self.relayout_page(*self.viewport.size());
```

"the header is still missing a configured line" and "the viewport is not full" are different
conditions, but they cannot diverge visibly today:

- A missing header line means `doc.line_count() < num_lines` (lines stream in order), so every
  row the pager can lay out comes from header lines.
- A full viewport therefore means those lines already render `>= viewport height` rows, which is
  past the `height - 1` cap — the late line would not appear even after a rebuild.

So the gate is correct only because the cap keeps the header within the viewport. Any resolution
that lets the header occupy `viewport height` rows or more must give `pump_input` its own
header-staleness condition; otherwise a header line arriving after the first screen fills stays
off-screen until the next resize or scroll.
````

And one line in that issue's `## Outcome`:

```markdown
- `Pager::pump_input`'s first-screen fill gate no longer silently depends on the cap to keep a
  late-arriving header line from being missed.
```

No code or test changes. `header_height_stays_below_num_lines_while_document_is_still_shorter`
keeps passing as is.

### Plan 2: Put the argument in the code instead

Extend the `pump_input` doc comment — it already claims "the headers are rebuilt", which is the
sentence that needs the caveat:

```rust
    /// The header is rebuilt only through this same gate, even though "the header
    /// is still missing a configured line" is not the same condition as "the viewport
    /// is not full". They cannot diverge visibly while `Header::build_rows` caps the
    /// header below the viewport height. Letting the header occupy the full viewport
    /// means this gate needs its own header condition.
```

Rejected: the reasoning is four steps deep and spans `Header::build_rows`, `Viewport`'s top
anchor, and the `Document`'s ordering guarantee. That is too much context for a comment on a
method that has no defect, and it would sit in front of every reader of `pump_input` while only
mattering to whoever changes the cap.

### Plan 3: Give the header its own rebuild trigger

Make the header's staleness explicit in the gate:

```rust
        if result.grew
            && (self.viewport.rows().len() < self.viewport.size().height()
                || self.header.is_missing_lines(&self.doc))
        {
            self.relayout_page(*self.viewport.size());
```

Rejected: the extra condition is unreachable today, so it is a second code path plus a new
`Header` accessor for no observable change. It also cannot be written from `height()` /
`num_lines()` alone — those are a row count and a line count, the exact confusion `82d6b0b`
just untangled — so it needs `Header` to expose how many lines it actually rendered. Worth
revisiting only as part of the capped-header work, at which point the condition becomes
reachable and can be added with a test that fails without it.

## Recommendation

**Plan 1.** The feedback identifies a documentation gap, not a defect, and the only audience for
the missing note is whoever changes the header's row cap. That person necessarily works from
`dev/issues/draft/capped-header-lines-become-unreachable.md`, so recording it there puts the
warning exactly where it will be read, keeps `pump_input` free of reasoning no current reader
needs, and costs one section in a document that already exists to track this class of hidden
cap dependency.
