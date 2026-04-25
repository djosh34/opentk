.PHONY: check lint test test-long

check: lint

lint:
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings

test:
	./scripts/cargo-test-with-postgres.sh

test-long:
	cargo test --workspace --ignored
