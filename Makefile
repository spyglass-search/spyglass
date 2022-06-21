.PHONY: build-backend build-client build-release check clippy fmt test setup-dev setup-dev-linux run-client-dev

TARGET_ARCH := $(shell rustc -Vv | grep host | awk '{print $$2 " "}')

build-backend:
	cargo build -p spyglass
	mkdir -p crates/tauri/binaries
	cp target/debug/spyglass crates/tauri/binaries/spyglass-server-$(TARGET_ARCH)

build-client:
	cargo build -p spyglass-client -p spyglass-app

build-release:
# Build backend binaries
	cargo build -p spyglass --release
	mkdir -p crates/tauri/binaries
	cp target/release/spyglass crates/tauri/binaries/spyglass-server-$(TARGET_ARCH)
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
	cargo install tauri-cli --locked --version ^1.0.0
	cargo install --locked trunk

setup-dev-linux:
	sudo apt install \
		libwebkit2gtk-4.0-dev \
		build-essential \
		curl \
		wget \
		libssl-dev \
		libgtk-3-dev \
		libayatana-appindicator3-dev \
		librsvg2-dev

run-client-dev:
	cargo tauri dev
