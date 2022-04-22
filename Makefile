.PHONY: build-backend build-client build-release check clippy fmt test setup-dev run-client-dev

build-backend:
	cargo build -p spyglass

build-client:
	cargo build -p spyglass-client -p spyglass-app

build-release:
# Build backend binaries
	cargo build -p spyglass --release
	cp target/release/spyglass target/release/spyglass-server-aarch64-apple-darwin
# Build client
	cargo tauri build
# Run macOS binary signing utility

check:
	cargo check --all

clippy:
	cargo clippy --all

fmt:
	cargo fmt --all

test:
	cargo test --all

setup-dev:
# Install tauri-cli & trunk for client development
	cargo install tauri-cli --locked --version ^1.0.0-rc.8
	cargo install --locked trunk

run-client-dev:
	cargo tauri dev