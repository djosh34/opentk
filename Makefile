.PHONY: check lint test test-long

check: lint

lint:
	CARGO_INCREMENTAL=0 cargo fmt --check
	CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings

test:
	CARGO_INCREMENTAL=0 ./scripts/cargo-test-with-postgres.sh

test-long:
	CARGO_INCREMENTAL=0 cargo test --workspace --ignored
