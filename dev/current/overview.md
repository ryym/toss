# Goal

Let the user turn off the scroll animation from the command line.

```
toss --scroll instant README.md
```

`--scroll` takes `smooth` (the default) or `instant`. Under `instant`, a page or
half-page scroll lands at once instead of easing over several frames.

# Context

Smooth scrolling is one of the features toss exists for, so it stays on by default.
But there are ordinary reasons to want it off:

- A slow or remote terminal (SSH, tmux over a poor link) redraws the intermediate
  frames badly, so the animation looks worse than an instant jump.
- Screen recording and demos want deterministic frames.
- Plain preference: some people read faster without motion.

Instant scrolling already exists in the code, but only as a test-only backdoor:
`RunConfig::instant_scroll` sets `App::set_instant_scroll` so tests can assert on the
page within the call that scrolled it. Exposing it as a real option removes that
duplication - the tests drive the same path a user does, and the backdoor goes away.

## Why a mode option instead of a boolean flag

`--no-smooth-scroll` would be shorter, but a negative flag cannot be undone. toss is
typically launched through `PAGER` / `GIT_PAGER`, so a setting made once in the
environment must stay overridable for a single invocation. `--scroll smooth` overrides
an earlier `--scroll instant` by last-wins; `--no-smooth-scroll` would need a second,
opposite flag to reach the same place.

A mode option also absorbs future values (easing variants, for example) without another
breaking addition to the interface.

## Where the setting lives

The flag belongs to `cli::Args`, not to `pager::Options`. `Options` describes how the
pager composes a page - the fixed header and the sticky heading - and is handed to
`Layout`. Scroll animation is the event loop's concern: `App` consults it every time a
page-sized scroll is requested, so `App` holds it and `run` passes it in at construction.
