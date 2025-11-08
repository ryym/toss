.PHONY: test
test:
	cargo test

.PHONY: bench
bench:
	# https://doc.rust-lang.org/cargo/commands/cargo-bench.html
	cargo +nightly bench --features bench

.PHONY: container
container:
	docker compose run -it --rm dev bash
