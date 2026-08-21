# Heading takes two bare usize header metrics that are easy to swap

Review target: `7f909cc..82d6b0b`

## Summary

**The fix removes one unit confusion and opens the door for another: `Heading::new` and
`Heading::resize` now take two adjacent `usize` parameters that differ only in unit.**

`src/pager/heading.rs` (L23-34):

```rust
    pub fn new(
        options: Option<HeadingOptions>,
        size: &ViewportSize,
        global_header_height: usize,
        global_header_num_lines: usize,
    ) -> Self {
```

- **The compiler cannot tell them apart**
  - `global_header_height` — screen rows, feeds `max_heading_height`
  - `global_header_num_lines` — document lines, feeds `min_line_index`
  - Swapping the two arguments at a call site compiles and silently restores the exact bug
    this commit fixes.
- **Both call sites already hold the `Header` itself**, so the split buys nothing.

`src/pager.rs` (L315-320):

```rust
        self.heading.resize(
            &mut self.doc,
            &size,
            self.header.height(),
            self.header.num_lines(),
        );
```

## Related finding

The new unit test in this commit builds a `Header` shape that no real viewport can produce.
See `correctness-001-capped-header-direction-is-unreachable.md`.

## Possible fixes

- **Pass `&Header`** — `Heading` and `Header` are both `pub(super)` under `pager`, and
  `HeadingConfig::new` would read `header.height()` / `header.num_lines()` itself.
  One argument, no order to get wrong, and the tests are forced to build a real `Header`.
  Cost: a `Heading` -> `Header` dependency where there is none today.
- **Pass a small carrier struct** (e.g. `HeaderMetrics { height, num_lines }`) —
  keeps the modules independent while making the fields named at the call site.
  It does not stop a `Header` from filling the struct wrongly, but the mistake becomes local
  to one place.
- **Newtype the units** (e.g. `Rows(usize)` / `Lines(usize)`) — the strongest guarantee,
  and it would generalise: `dev/issues/draft/heading-line-range-clamped-by-row-count.md`
  records the same row/line mix-up elsewhere in this file. Largest change of the three.

## Assessment

Valid. Verified against current code (`src/pager/heading.rs`, `src/pager/header.rs`,
`src/pager.rs`): `Heading::new`/`resize` take `global_header_height: usize` and
`global_header_num_lines: usize` back to back, both feed into `HeadingConfig::new` in the
same order, and every test call site (`heading.rs` tests, e.g. `Heading::new(..., 2, 2)`,
`Heading::new(..., 2, 1)`) passes them as bare adjacent integer literals — nothing stops a
transposition from compiling.

`Heading` and `Header` are both declared as private submodules of `pager` (`src/pager.rs:15-16`,
no `pub` re-export), so they are only reachable from within `pager` itself. There is no real
encapsulation boundary being crossed by having `Heading` depend on `Header`'s shape directly —
they are already implementation details of the same parent module.

## Plans

### Plan 1: Pass `&Header` instead of two `usize`s (Recommended)

Change `Heading::new` and `Heading::resize` (and the private `HeadingConfig::new`) to take
`header: &Header` instead of `global_header_height: usize, global_header_num_lines: usize`,
and read `header.height()` / `header.num_lines()` internally. Update the two call sites in
`src/pager.rs` (construction and `relayout_page`) to pass `&self.header`.

This removes the swappable-argument pair entirely: there is exactly one thing to pass, and
it is a value the caller already owns. It also fixes the related finding for free — a test
building `Heading` must now go through a real `Header` (or a helper that builds one), instead
of fabricating an unreachable `(height, num_lines)` pair directly.

Cost: introduces a `Heading -> Header` compile-time dependency. Given both are private
submodules of `pager` with no existing architectural separation to protect, this cost is
negligible.

### Plan 2: Introduce a small named carrier struct

Add e.g. `struct HeaderMetrics { height: usize, num_lines: usize }` (or reuse/extract from
`Header` if one already fits) and pass one `HeaderMetrics` value instead of two raw `usize`s.
Keeps `Heading` decoupled from `Header`'s type, at the cost of a new struct that exists only
to carry two fields between two call sites, and does not by itself fix the related
unreachable-test-shape finding.

## Recommendation

Plan 1. `Heading` and `Header` are both private to `pager` already, so there is no
encapsulation to lose by having one depend on the other's public accessor methods, and this
is the same solution the review's "Possible fixes" section already leads with. It is a small,
local, mechanical change (two call sites, two constructors), removes the transposable
`usize` pair outright rather than just narrowing where the mistake can happen (as Plan 2
would), and additionally forces the heading test builders to go through a real `Header`,
closing the related "unreachable header shape" finding as a side effect.
