# Rust Conventions

## General

- Write a brief description comment for any `pub` interfaces (modules, functions, structs, etc).
- Avoid unnecessary `.clone()`. If the value is only used to pass a reference, borrow it directly instead of cloning into a temporary variable.

## Module structure

When a module has submodules, use `foo.rs` + `foo/` (not `foo/mod.rs`).
The parent module file stays alongside the directory, not inside it.

```
# Good
src/
  page.rs          # mod header; mod viewport;
  page/
    header.rs
    viewport.rs

# Bad — do NOT use mod.rs
src/
  page/
    mod.rs
    header.rs
    viewport.rs
```
