# ADR-002: Enable crossterm's `use-dev-tty` feature

## Status

Accepted

## Context

When toss receives input via pipe (e.g. `echo hello | toss`, `bat --pager toss file`),
it crashed on macOS with the error "Failed to initialize input reader".
This did not occur on Linux.

### Root cause

The root cause is a known incompatibility between macOS's kqueue and `/dev/tty`.

- [tokio-rs/mio#1377](https://github.com/tokio-rs/mio/issues/1377): kqueue cannot poll `/dev/tty` on macOS, returning `EINVAL` (os error 22) on `register()`.
- [crossterm-rs/crossterm#500](https://github.com/crossterm-rs/crossterm/issues/500): crossterm's default event source uses mio, which hits this kqueue limitation.

When stdin is a pipe, crossterm falls back to opening `/dev/tty` for keyboard input.
The file opens successfully and `enable_raw_mode()` works, but mio's `registry.register()`
fails because macOS's kqueue does not support monitoring pty device file descriptors.
Linux's epoll does not have this limitation.

crossterm silently discards the initialization error (`source.ok().map(...)` in
`InternalEventReader::default()`), setting the event source to `None`.
Any subsequent `event::poll()` call then returns the "Failed to initialize input reader" error.

The previous implementation (termion-based) avoided this by opening `/dev/tty` directly
and reading events from it without relying on kqueue/epoll.

## Decision

Enable the `use-dev-tty` feature of crossterm:

```toml
crossterm = { version = "0.29", features = ["use-dev-tty"] }
```

This feature was introduced in [crossterm-rs/crossterm#735](https://github.com/crossterm-rs/crossterm/pull/735)
specifically to fix issue #500. It replaces mio-based polling with `filedescriptor::poll()`,
which internally falls back to `select()` — a system call that handles `/dev/tty` correctly
on macOS.

The PR also addresses signal handling (`SIGWINCH`) and the waker system by using unix
socket pairs instead of relying on mio's signal integration.

## Alternatives Considered

### Switch from crossterm to termion

The old implementation used termion and opened `/dev/tty` directly, which never had
this issue. Switching back is feasible because crossterm usage is limited to:

- `TermScreen` implementation in `screen.rs` (raw mode, alternate screen, cursor, scroll, events)
- `Event` / `KeyCode` / `KeyEvent` types imported in `app.rs`, `mock_screen.rs`, and tests

The `Screen` trait already abstracts terminal operations, so replacing the `TermScreen`
implementation is straightforward. The main work would be replacing the crossterm event
types that leak into `App` and tests — either by defining our own key event types or
by using termion's types directly.

This was not chosen because:

- The `use-dev-tty` feature resolves the issue with a one-line change.
- crossterm is more actively maintained than termion.
- crossterm supports Windows, which could matter in the future.

## Consequences

- Pipe input (`echo ... | toss`, `bat --pager toss`, etc.) works on macOS.
- The `use-dev-tty` backend is less battle-tested than the default mio backend.
  If issues arise, we may need to manage `/dev/tty` directly as the old termion-based
  implementation did, or consider switching to termion.
- An additional dependency (`filedescriptor` crate) is pulled in.
