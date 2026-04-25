.PHONY: check lint test test-long

check: lint

lint:
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

test-long:
	cargo test --workspace --ignored
