# Rust Conventions

- Write a brief description comment for any `pub` interfaces (modules, functions, structs, etc).
- Use `foo.rs` + `foo/` for submodules instead of `foo/mod.rs`.
- Avoid unnecessary `.clone()`. If the value is only used to pass a reference, borrow it directly instead of cloning into a temporary variable.
