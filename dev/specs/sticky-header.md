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

When `--heading-lines` is greater than 1, multiple lines within an `--heading-lines` window may match the `--heading` pattern. In that case, the **last** matching line in the window becomes the heading start; earlier matches are not treated as headings.

For example, with `--heading '^#'` and `--heading-lines 2`:

```
# Changelog          ← matches, but the next line within the 2-line window also matches → not a heading (line 0)
## 1.0.23            ← matches and no further match within its window → heading start (line 1)
                     ← part of line 1's heading block (line 2)
release notes...     ← first content line (line 3)
```

Line 0 matches the pattern, but because line 1 also matches within its 2-line window, line 0 is not treated as a heading. Line 1 becomes the section heading start, and its block spans lines 1–2.

### Combining `--header` and `--heading`

Fixed header and section heading can be used together. The fixed header is always shown at the top, with the section heading displayed below it.

If a section heading block overlaps with the fixed header lines, only the non-overlapping portion of the section heading is shown (the fixed header takes priority).
