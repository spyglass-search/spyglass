.PHONY: build-backend build-client check clippy fmt test run-client-dev

build-backend:
	cargo build -p spyglass-bin

build-client:
	cargo build -p spyglass-client -p spyglass-app

check:
	cargo check --all

clippy:
	cargo clippy --all

fmt:
	cargo fmt --all

test:
	cargo test --all

run-client-dev:
	cargo tauri dev