# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Toss is a minimal terminal pager written in Rust - an alternative to `less` that focuses on simplicity.
The develop has just started so its functionality and architecture are very early stage.

It does not implement all features of `less`, but aims to support these features:

- Basic navigation by keyboard
- Key binding customization
- Persisted search history

Additionally, it aims to add these modern features:

- Smooth scroll: Animate navigation to make it easier to understand how it is moving.
- Incremental search: Highlight matched parts as you type.

## Architecture

- **Input handling**: Supports both file input (via command line argument) and stdin piping
- **Terminal management**: Uses `termion` crate for raw mode, alternate screen, and input event handling
- **Navigation system**: Implements vi-like keybindings (j/k, g/G, d/u/f/b for scrolling)
- **Smooth scrolling**: Custom animation system with easing for page navigation
- **Panic handling**: Custom panic hook that logs errors to `toss-panic.log`

## Development Commands

### Building and Running

```bash
# Build the project
cargo build

# Build with optimizations
cargo build --release

# Run with a file
cargo run <filename>

# Run with stdin input
cat <filename> | cargo run
```

### Testing and Validation

```bash
# Run tests (if any exist)
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy for linting
cargo clippy
```

## Key Navigation Controls

The application implements these vi-like keybindings:

- `j`: Move down one line
- `k`: Move up one line
- `g`: Go to beginning of file
- `G`: Go to end of file
- `d`: Scroll down half page (with smooth animation)
- `u`: Scroll up half page (with smooth animation)
- `f`: Scroll down full page (with smooth animation)
- `b`: Scroll up full page (with smooth animation)
- `q` or `Esc`: Quit

## Dependencies

- `termion 4.x`: Terminal manipulation library for raw mode, alternate screen, and input handling

## Error Handling

The application includes a custom panic handler that logs all panic information to `toss-panic.log` in the project root. This file helps debug issues that occur during development or usage.
