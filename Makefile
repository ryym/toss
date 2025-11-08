.PHONY: test
test:
	cargo test

.PHONY: bench
bench:
	# https://doc.rust-lang.org/cargo/commands/cargo-bench.html
	cargo +nightly bench --features bench
