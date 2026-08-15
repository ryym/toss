---
type: bugfix
tags: [terminal, diagnostics]
---

## Overview

**A panic while the pager is running shows nothing on the terminal.**

- The panic message goes to stderr, which is the alternate screen buffer at that point.
- `TermScreen`'s `Drop` leaves the alternate screen while unwinding, discarding it.
- From the user's side, toss simply disappears: no message, no hint of a crash.
  Only the exit code (101) shows anything happened.

The message is not lost entirely: the panic hook in `src/logger.rs` writes `toss-panic.log` to
the current directory. But nothing tells the user that the file exists, so a crash is
indistinguishable from a normal quit unless one already knows to look.

## Reproduction

```
$ printf '' | toss
# press G -> toss exits immediately, terminal looks untouched
$ echo $?
101
$ cat toss-panic.log
panicked at src/pager.rs:350:30:
index out of bounds: the len is 0 but the index is 0
```

Captured through a pty (stdin is an empty pipe, stdout/stderr are the pty), the raw byte stream
shows the message being written between entering and leaving the alternate screen:

```
\x1b[?1049h ... (page drawn) ...
thread 'main' panicked at src/pager.rs:350:30:
index out of bounds: the len is 0 but the index is 0
note: run with `RUST_BACKTRACE=1` ...
\x1b[?25h\x1b[?1049l
```

Everything printed after `\x1b[?1049h` and before `\x1b[?1049l` is dropped by the terminal when
the primary buffer is restored.

## Root Cause

**The default panic handler prints while the alternate screen is still active.**

- **`TermScreen::new`** (`src/screen/term_screen.rs:26-32`)
  - Enables raw mode and enters the alternate screen.
- **`Drop for TermScreen`** (`src/screen/term_screen.rs:35-45`)
  - Leaves the alternate screen and disables raw mode.
  - Runs during unwinding, i.e. *after* the panic hook has already run.
- **The hook installed by `store_logs_on_panic`** (`src/logger.rs:83-104`)
  - Ends by delegating to the previous hook:

    ```rust
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // flush the debug log, then write toss-panic.log
        original_hook(panic_info);
    }));
    ```

  - `original_hook` is the default handler, which prints to stderr immediately — while the
    alternate screen is still active.
  - Raw mode also mangles the line breaks of such output.

`src/main.rs` regains control only for the `Err` path of `toss::run()`; a panic unwinds straight
past it, so there is currently no point at which the message could be re-printed after the
screen is restored.

The same applies to anything else written to stderr while the pager is up.

## Plan

**The existing hook's output timing is what has to change.**

- Adding another hook does not help on its own.
- Hooks chain correctly (every installer uses `panic::take_hook`), but the chain still
  terminates in the default handler that prints at the wrong time.

### Two options

1. **Deferred print**
   - Stop calling `original_hook` in `store_logs_on_panic`; store the message instead.
   - Wrap `toss::run()` in `std::panic::catch_unwind` in `src/main.rs`.
   - Print the stored message to stderr once unwinding has restored the screen
     (`TermScreen::drop` runs during unwinding, before `catch_unwind` returns).
   - Also lets the exit code be chosen explicitly.
2. **Restore the terminal inside the hook**
   - Leave the alternate screen and disable raw mode at the top of the hook, then delegate as
     today.
   - `TermScreen::drop` repeating the same work afterwards is harmless.
   - Smaller diff, but terminal restoration ends up split between `logger` and `screen`.

Either way, point the user at `toss-panic.log` in the printed message.

### Defects worth fixing at the same time

- **Empty `RUST_LOG` skips the hook entirely**
  - `setup_file_logger` returns at `src/logger.rs:65-67` before `store_logs_on_panic` is ever
    called, so no hook is installed and no `toss-panic.log` is written.
  - With `RUST_LOG` unset the hook *is* installed (`src/logger.rs:60`).
  - The two cases should behave the same.
- **The recorded backtrace is normally empty**
  - `Backtrace::capture()` (`src/logger.rs:93`) is a no-op unless `RUST_BACKTRACE` is set.
  - `Backtrace::force_capture()` would make the file useful by default.

### Verification

Possible without a terminal by driving the binary under a pty harness (a `pty.openpty` +
`TIOCSCTTY` fork is enough) and asserting the panic text survives in the output after
`\x1b[?1049l`.
