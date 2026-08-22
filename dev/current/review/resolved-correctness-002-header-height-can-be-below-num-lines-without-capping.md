# `height() < num_lines` is reachable without capping

Review target: `7f909cc..a4a1ae3` (`src/pager/header.rs`, `src/pager/heading.rs`,
`dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md`)

## Summary

**The fix is right, but every place that records *why* the opposite divergence is harmless — the
new test, the issue doc, and the new `num_lines()` doc comment — covers only one of the two ways
it happens.**

`Header::height()` also falls below `num_lines` when the header's lines are simply **not in the
document** — a file shorter than `--header-lines`, or a streamed document whose header lines have
not arrived yet. `build_rows` skips missing lines instead of capping:

`src/pager/rows.rs` (L13-22):

```rust
for i in line_index_range {
    if let Some(line) = doc.line(i) {
        rows.extend_from_slice(&line.wrap(width));
    }
    if rows.len() >= max_rows {
        break;
    }
}
```

Verified on `a4a1ae3`: `Header::new(doc("a\nb\n"), size(10, 10), 3)` yields `height() == 2`,
`num_lines() == 3`, and `max_heading_height == 10 - 2 - 1 == 7`.

## Why it matters

The `max_heading_height == 0` argument does not apply to this shape, so these places assert
something the argument does not establish.

### The new test and its comment

`src/pager/heading.rs`:

```rust
fn unreachable_capped_header_still_uses_header_line_count() {
    // height (2) < num_lines (3) only to pin the arithmetic, not to model a capped header:
    // that would force max_heading_height to 0, which find_heading short-circuits on first.
```

`(2, 3)` is not an unreachable shape. It is exactly what a 3-line header over a 2-line document
produces, with `max_heading_height > 0`.

### The issue doc

`dev/issues/open/20260817-heading-min-line-index-uses-header-row-count.md` — "They do not
diverge the other way" is argued purely from the cap.

### `Header::num_lines()`'s new doc comment

It is documented as "the number of document lines the header covers", but it is the *configured*
count, which can exceed both the lines rendered and the lines that exist. Now that `Heading`
reads this value to decide which lines may become headings, the doc comment is the only place
stating its contract, so it should be exact.

`src/pager/header.rs` (L34-39):

```rust
    /// The number of document lines the header covers, which differs from [`Self::height`]
    /// whenever a header line wraps or the header is capped to fit the viewport.
    pub fn num_lines(&self) -> usize {
        self.num_lines
    }
```

Where "covers" is wrong:

- **Capped header** — `build_rows` truncates at `size.height() - 1` rows. Trailing header lines
  are never rendered, yet `num_lines` still counts them. The module's own test says so:

  ```rust
  let h = Header::new(&mut doc, &size, 5);
  assert_eq!(h.height(), 4);
  // contains() reflects the configured num_lines, not the visible row count.
  assert!(h.contains(4));
  ```

- **Document shorter than `num_lines`** — `rows::from_lines` skips lines that do not exist, so
  `num_lines` counts lines the document does not have. `Header::new(doc, size, 5)` on a 2-line
  document reports `num_lines() == 5`.

The second clause of the comment is also uneven: wrapping makes `height() > num_lines`, capping
makes `height() < num_lines`. Reading them as one undifferentiated "differs" is what the
original bug did.

## Open question

**Is the missing-lines shape actually harmless?**

Tracing `a4a1ae3` it appears to be, but for a different and more fragile reason than the cap:

- `header.height() < num_lines` only while the document has not yet reached line `num_lines`.
- The heading search range then ends at or below `num_lines`, so it is empty either way.
- `pump_input` calls `relayout_page`, which rebuilds the header as lines arrive.

That argument depends on stream ordering and on when `relayout_page` runs, not on arithmetic.
Worth confirming and recording explicitly, since the same shape is what the fixed bug was about.

## Suggestions

- The test could stop describing its state as unreachable and instead build the `(height,
  num_lines)` pair from a real short document — which also removes the fabricated pair called out
  in `dev/issues/draft/heading-swappable-usize-header-metrics.md`.

- Describe `num_lines()` as configuration, and name the direction of each divergence.
  Something like:

  ```rust
      /// The number of leading document lines configured as the header.
      /// This is the header's extent in *lines*, matching [`Self::contains`]; it is not the
      /// number of lines actually rendered. [`Self::height`] counts *rows* instead, and is
      /// larger when header lines wrap and smaller when the header is capped to fit the viewport.
  ```

## Related: a diff-referential comment in `HeadingConfig::new`

`src/pager/heading.rs` (L216-219):

```rust
        Self {
            // Bound candidates by the lines the header covers, not by the rows it occupies.
            min_line_index: global_header_num_lines,
```

- The "not by the rows it occupies" half explains the old code, not this code.
- A reader with no memory of `82d6b0b` gets nothing from it.
- The parameter name already says `num_lines`, so the remaining half adds little either.

## Assessment

- Newly introduced issue? No — the missing-lines shape existed before `82d6b0b`/`a4a1ae3`; what's
  new is the fix's test, doc comment, and issue writeup claiming to fully explain
  `height() < num_lines` while only covering the cap.
- Does it block the overall goal? No — the goal (bound heading candidates by header lines, not
  header rows) is achieved correctly. This is a documentation/comment-accuracy gap, not a
  behavioral bug.

The missing-lines shape is real, but harmless — it's a plain arithmetic invariant: every line
`Heading` searches has already streamed in, so the search range is always empty while the header
is short. The review's own suggested test fix doesn't hold up, though — pairing this shape with
real content at the boundary is itself unreachable, and the original test's assertion never
distinguished what it aimed to pin. The review's other points (doc comment, issue doc,
`HeadingConfig::new` comment) are accurate and worth fixing.

### Details

#### The missing-lines shape is real and harmless

Verified on `a4a1ae3`: a streamed 2-line document with `--header-lines 3` reports
`header.height() == 2` while `header.num_lines() == 3`, viewport tall enough that this isn't the
cap — the same shape as the review's own `Header::new(doc("a\nb\n"), size(10, 10), 3)` example.

Why it's harmless: every call site reaching `Heading::resolve` / `resolve_if_found` derives its
`line_index` from a row already present in `self.viewport.rows()` (`src/pager.rs:173, 285, 335,
355, 379, 427, 441`), and those rows come from `rows::from_lines` (`src/pager/rows.rs`), which
only emits rows for lines that actually exist. So while the document is shorter than `num_lines`,
`line_index < doc.line_count() <= header.num_lines()` always holds (`num_lines` is fixed
configuration, never shrinks), making the search range `min_line_index..(line_index + 1)` empty
unconditionally — a plain arithmetic invariant, not "while stream ordering holds" as the review's
"Open question" frames it. Concretely: while a header line configured to be at index N hasn't
streamed in yet, no call site can ever pass `line_index >= N` to `resolve`, so a heading search
starting at `min_line_index == N` never has anything to find; once that line does arrive, it's
already inside the header and stays excluded permanently.

#### One of the review's own suggested fixes doesn't hold up

It proposes rebuilding the new `unreachable_capped_header_still_uses_header_line_count` test's
`(height=2, num_lines=3)` pair "from a real short document" instead of constructing `Heading`
directly.

- The pair alone is real (see above), but the test pairs it with a 4-line document that has real,
  pattern-matching content at lines 2 and 3 — and *that* combination is what no `Header` can ever
  produce: real content up to line 2 makes `Header::height()` equal `num_lines` there, closing the
  gap.
- Building the pair from an actually-short document removes the fabricated numbers but also
  removes any content past `num_lines` for `resolve` to search over, so the assertion collapses to
  "nothing is found because there is nothing there" — already implied by the arithmetic above.
- Separately, the assertion never distinguished what the test claims to pin:
  `h.start_line_index().is_none()` is true whether `find_heading`'s `max_heading_height == 0`
  early return fires or the scan simply doesn't match `"x"`.

#### The rest checks out

The imprecise `num_lines()` doc comment, the issue doc's one-sided "they do not diverge the other
way" claim, and the diff-referential comment in `HeadingConfig::new` are all accurate and worth
fixing — low-risk doc/comment edits, none touching runtime behavior.

## Plans

### Plan 1: Fix the doc/comment inaccuracies, drop the test that can't be salvaged (recommended)

- **`Header::num_lines()` doc comment** — replace with wording that states it's configuration and
  names both divergence directions:

  ```rust
      /// The number of leading document lines configured as the header.
      /// This is the header's extent in *lines*, matching [`Self::contains`]; it is not the
      /// number of lines actually rendered. [`Self::height`] counts *rows* instead, and is
      /// larger when header lines wrap and smaller when the header is capped to fit the
      /// viewport or the document has fewer lines than configured.
      pub fn num_lines(&self) -> usize {
  ```

- **Issue doc `20260817-heading-min-line-index-uses-header-row-count.md`** — extend "They do not
  diverge the other way" to cover both shapes that produce `height() < num_lines`: the cap (as
  already written) and the missing-lines case, with the arithmetic argument above (every
  `line_index` reaching `resolve`/`resolve_if_found` is bounded by `doc.line_count() <=
  num_lines`, so the search range is always empty).

- **`HeadingConfig::new` comment** — reword to describe the current code without referencing the
  pre-`82d6b0b` behavior, e.g.:

  ```rust
  Self {
      // Header lines are never heading candidates, regardless of how many rows they render as.
      min_line_index: global_header_num_lines,
  ```

- **`unreachable_capped_header_still_uses_header_line_count`** — delete it. As shown above, no
  edit makes it both realistic and meaningful: realistic inputs collapse the assertion to a case
  already covered elsewhere, and the assertion never distinguished the two code paths it was
  meant to pin in the first place. If direct coverage of `HeadingConfig::new`'s arithmetic is
  wanted, add a narrow replacement that asserts on `max_heading_height` itself instead of routing
  through `resolve`:

  ```rust
  #[test]
  fn max_heading_height_reserves_one_row_below_the_header() {
      let config = HeadingConfig::new(&size(10, 10), 2, 3);
      assert_eq!(config.max_heading_height, 7); // 10 - 2 - 1
  }
  ```

  This is optional — the arithmetic is already implicitly exercised by every other test that
  successfully finds a heading.

### Plan 2: File a follow-up issue instead of fixing now

Record the same points as a new issue under `dev/issues/` for later cleanup, without touching the
current commits. Viable since none of this blocks correctness, but since every affected file is
already mid-edit in this same review chain (`82d6b0b`..`a4a1ae3`) and the fixes are small,
deferring adds coordination overhead (a future agent has to re-locate and re-verify the same
spots) for no real benefit.

## Recommendation

**Plan 1.** The fixes are small, self-contained doc/comment edits plus one test deletion, all with
no behavioral risk, and they live in files already touched by this review chain — deferring them
via a new issue (Plan 2) only adds re-discovery cost later for no offsetting gain. Do the
`num_lines()` doc comment and the issue-doc extension first (pure text, highest value for future
readers); the test deletion and the `HeadingConfig::new` comment reword are good to bundle in the
same pass but lower priority individually. The optional `HeadingConfig` arithmetic test can be
skipped unless direct coverage is specifically wanted.
