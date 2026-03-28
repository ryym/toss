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

### `--section REGEX`

Regex pattern to identify section start lines. When a section scrolls above the viewport, its header is pinned at the top.

```bash
cat README.md | toss --section '^#{1,6} '
```

### `--section-header N` (default: 1)

Number of lines per section header block, starting from the matched line. Used with `--section`.

```bash
git diff | toss --section '^diff --git' --section-header 4
git log  | toss --section '^commit [0-9a-f]' --section-header 3
```

## Behavior

### Fixed header (`--header`)

- The first N lines of the document are always shown at the top.
- The viewport starts below these lines; the user cannot scroll above them.
- The visible content area shrinks by the number of screen rows the header occupies (accounting for line wrapping).

### Section header (`--section`)

A section starts at a line matching the `--section` pattern.
The section header block is the N lines starting from that match (where N = `--section-header`, default 1).
Only the most recent section header is shown at the top; there is no stacking or nesting of multiple sections.

#### Pattern matches within a header block

When `--section-header` is greater than 1, lines within the header block may also match the `--section` pattern. These intra-block matches are **not** treated as new section starts.

For example, with `--section '^#'` and `--section-header 3`:

```
# Changelog          ← section start (line 0)
                     ← part of header block (line 1)
## 1.0.23            ← matches '^#' but is part of line 0's block (line 2)
                     ← first content line (line 3)
```

Line 2 (`## 1.0.23`) matches the pattern but falls within the 3-line header block of line 0. It is treated as part of that block, not as a separate section. The next section can only start at line 3 or later.

### Combining `--header` and `--section`

Fixed header and section header can be used together. The fixed header is always shown at the top, with the section header displayed below it.

If a section header block overlaps with the fixed header lines, only the non-overlapping portion of the section header is shown (the fixed header takes priority).
