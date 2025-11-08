// With the `bench` feature flag, enable unstable test features to run benchmarks.
// https://doc.rust-lang.org/unstable-book/library-features/test.html#test
#![cfg_attr(feature = "bench", feature(test))]

pub mod app;
mod error;
mod logger;
mod pager;
mod reader;
mod screen;
mod source;
