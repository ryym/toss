# Goal

Make the slash (`/`) and question-mark (`?`) incremental search interpret user
input as a Rust `regex` crate pattern, instead of the current literal
substring match.

# Context

The search engine internals (`src/search.rs`, `src/line.rs`,
`src/renderer/highlight.rs`) already work in terms of `regex::Regex` and can
handle match ranges, multiple matches, anchors, and character classes without
changes. The only place preventing regex search today is `src/pager.rs`,
where the user's raw input is passed through `regex::escape` before being
compiled, forcing every character to be treated literally.

Removing that escaping introduces two problems that this work addresses:

- Backward compatibility with literal searches is intentionally dropped.
  `toss` is still in early development, so this is an acceptable breaking
  change. Users who need a literal match can escape metacharacters manually
  (as in vim). A literal/regex mode toggle may be considered later as a
  separate task.
- Incremental search calls `update_search_query` on every keystroke, so the
  input is frequently a syntactically invalid regex mid-edit (e.g. right
  after typing `(` or `[`). Instead of panicking or clearing the search
  result, an invalid regex must freeze the current preview (highlight and
  jump target) and leave it unchanged until the input becomes valid again.
  No visual feedback indicating invalidity (e.g. a status-line note) is
  shown during this freeze; only the input field text itself keeps updating
  live as the user types. Likewise, pressing Enter while the input is
  invalid must be a no-op that keeps the search input mode active, rather
  than confirming a stale search or giving any error feedback. Surfacing a
  transient error message when Enter is ignored this way is planned as a
  follow-up, not part of this work.

Out of scope: case-sensitivity toggling and match-count display in the
status line.
