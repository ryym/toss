# Integration tests

These tests drive the real `App`/`Pager` against `MockScreen`, an in-memory
`Screen` implementation, and assert on the text log it produces. This avoids
depending on internal state (scroll offset, search state, etc.) and instead
checks what a user would actually see and do.

## Writing a test

Build a `TestCase` (content, screen size, key `events`) and pass it to
`run_test_screen`, then compare `screen.out()` against an expected string.
See `mod.rs` for `TestCase`/`run_test_screen` and shared event helpers
(`key`, `esc`, `enter`, `backspace`).

`streaming.rs` bypasses `run_test_screen` and drives `App`/`MockScreen`
directly, since it needs to push lines through a channel between app
construction and `run()`.

## Output format (`screen.out()`)

The log alternates between input and output:

- `[EVENT]:...` — one line per key event as it is consumed (e.g.
  `[EVENT]:char:j`, `[EVENT]:esc`). Unhandled key codes log as
  `[EVENT]:ERROR:unexpected:...`, which will fail a diff loudly rather than
  silently passing.
- A grid snapshot — one line per screen row, taken whenever the app flushes
  the screen. Rows are separated by a `-----` line.

Within a snapshot row:

- A trailing `>` marks a row that soft-wrapped into the next row.
- ANSI styling is rendered as labels instead of raw escapes, so diffs stay
  readable: `{b}`/`{/b}` bold, `{line}`/`{/line}` underline, `{rev}`/`{/rev}`
  reverse, `{red}` red, `{reset}` reset. Any other sequence falls back to
  `{ESC:...}`. Add new labels in `mock_screen.rs::escape_to_label` if a test
  needs a style not listed here.

When updating expected output after an intentional behavior change, run
`cargo test` and diff the `pretty_assertions` output rather than
hand-editing the expected string — it's easy to get the escape labels or
`-----` separators subtly wrong by hand.

## Always assert the full output

Always compare the entire `screen.out()` with `assert_eq!`, covering every
event and every snapshot from start to end, rather than checking a
substring (`contains`, slicing, matching only the last snapshot, etc.).
A partial check can miss unwanted side effects on unrelated rows or earlier
steps, silently passing tests that should fail.
