# Sticky Header

Pin header lines at the top of the screen while scrolling.
This helps users keep context (e.g., column names in query results, which file in a diff, which commit in a log).

## CLI Options

### `--header N`

Pin the first N lines of the input as a fixed header. These lines are always visible at the top regardless of scroll position.

```bash
psql ... | toss --header 2
ps aux   | toss --header 1
```

### `--heading REGEX`

Regex pattern matching section heading lines. When a section scrolls above the viewport, its heading is pinned at the top.

```bash
cat README.md | toss --heading '^#{1,6} '
```

### `--heading-lines N` (default: 1)

Number of lines per section heading block, starting from the matched line. Used with `--heading`.

```bash
git diff | toss --heading '^diff --git' --heading-lines 4
git log  | toss --heading '^commit [0-9a-f]' --heading-lines 3
```

## Behavior

### Fixed header (`--header`)

- The first N lines of the document are always shown at the top.
- The viewport starts below these lines; the user cannot scroll above them.
- The visible content area shrinks by the number of screen rows the header occupies (accounting for line wrapping).

### Section heading (`--heading`)

A section starts at a line matching the `--heading` pattern.
The section heading block is the N lines starting from that match (where N = `--heading-lines`, default 1).
Only the most recent section heading is shown at the top; there is no stacking or nesting of multiple sections.

#### Pattern matches within a heading block

When `--heading-lines` is greater than 1, lines within the heading block may also match the `--heading` pattern. These intra-block matches are **not** treated as new section starts.

For example, with `--heading '^#'` and `--heading-lines 3`:

```
# Changelog          ← section start (line 0)
                     ← part of heading block (line 1)
## 1.0.23            ← matches '^#' but is part of line 0's block (line 2)
                     ← first content line (line 3)
```

Line 2 (`## 1.0.23`) matches the pattern but falls within the 3-line heading block of line 0. It is treated as part of that block, not as a separate section. The next section can only start at line 3 or later.

### Combining `--header` and `--heading`

Fixed header and section heading can be used together. The fixed header is always shown at the top, with the section heading displayed below it.

If a section heading block overlaps with the fixed header lines, only the non-overlapping portion of the section heading is shown (the fixed header takes priority).
