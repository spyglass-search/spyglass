.PHONY: build-backend build-client run-client-dev check fmt test

build-backend:
	cargo build -p spyglass-bin

build-client:
	cargo build -p spyglass-client -p spyglass-app

check:
	cargo check --all

fmt:
	cargo fmt --all

test:
	cargo test --all

run-client-dev:
	cargo tauri dev