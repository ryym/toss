# Goal

Make toss pass its input straight through to stdout when stdout is not a terminal,
instead of starting the interactive pager.

`toss README.md | less` and `toss README.md > out.txt` should behave like `cat`.

# Context

toss only checks whether *stdin* is a terminal. It decides to start the pager
regardless of where its output goes, so piping its output hangs the terminal:

- The drawing escape sequences (alternate screen, cursor moves, ...) go into the pipe
  instead of the screen, so the downstream command receives them as content.
- crossterm is built with the `use-dev-tty` feature, so toss reads keys from the
  real terminal. A downstream pager reads the same terminal, and the two processes
  fight over the key input and over the terminal mode.
- toss keeps waiting for `q`, which never reliably arrives, and never closes the pipe.

`less` and `bat` avoid this by not paginating when stdout is not a terminal.
toss follows the same rule.

## Why relay bytes instead of reusing the `-F` print path

The `-F` (`--quit-if-one-screen`) short-circuit already prints a document without
paginating, but it is a poor fit for this case:

- It loads the whole input before printing, so an endless stream (`tail -f | toss | grep`)
  would produce nothing and grow in memory.
- It reprints decoded lines with `writeln!`, which does not reproduce the input bytes:
  a missing trailing newline is added, CRLF becomes LF, and invalid UTF-8 is replaced.

Copying the bytes from the input to stdout keeps the output identical to the input and
streams without buffering. `-F` keeps its own path, which is about skipping the pager
while still writing to a terminal.

## Terminal size

`run` currently reads the terminal size before anything else, and that fails when there
is no controlling terminal (CI, cron). The size is not needed to relay bytes, so
`RunConfig` takes a factory for it, as it already does for the screen, and calls it only
when the pager actually runs.

## Broken pipe

`toss big.txt | head` makes stdout fail with `BrokenPipe` once the downstream command
exits. Rust ignores `SIGPIPE`, so this surfaces as a write error. It is a normal way for
a pipeline to end, so toss exits successfully instead of reporting it.
