// With the `bench` feature flag, enable unstable test features to run benchmarks.
// https://doc.rust-lang.org/unstable-book/library-features/test.html#test
#![cfg_attr(feature = "bench", feature(test))]
// Clippy configuration. https://rust-lang.github.io/rust-clippy/master/index.html
#![deny(clippy::dbg_macro)]
#![deny(clippy::todo)]

pub mod app;
mod error;
mod logger;
mod pager;
mod reader;
mod screen;
mod source;
