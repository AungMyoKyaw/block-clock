.PHONY: dev build bundle test check ci

dev:
	cargo run

build:
	cargo build --locked --release

bundle:
	./scripts/bundle-macos.sh

test:
	cargo test --locked

check:
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings

ci: check test build bundle
