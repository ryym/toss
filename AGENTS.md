# Guidance for AI Agents

## Project Overview

Toss is a terminal pager written in Rust - an alternative to `less` that focuses on simplicity.
It does not aim to be a full `less` remplacement. No multi-file support, no marks, etc.
Instead, Toss will have these modern features:

- **Smooth scroll** - Animated scrolling with easing for page navigation
- **Incremental search** - Highlight matches as you type
- **Sticky header** - Pin a pattern-matched header line at the top while scrolling

## Status

The development is still in an early stage.
See `dev/architecture.md` for the overall design.

## Development

- Follow `dev/conventions/rust.md` when writing Rust code.
- Follow `dev/conventions/git.md` when using Git.

Whenever you develop, commit changes as you progress.
When you finish work, all your changes must be committed.

### Common Commands

```bash
cargo check  # Run typecheck
cargo test   # Run tests
cargo clippy # Run linter
cargo fmt    # Run formatter
```

NOTE: Since it uses terminal interactively, AI agents should not run `cargo run`.
Instead, you must rely on tests to check the validity of the program.
