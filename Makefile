.PHONY: build-backend build-client build-plugins build-styles build-release check clippy fmt test setup-dev setup-dev-linux run-client-dev

TARGET_ARCH := $(shell rustc -Vv | grep host | awk '{print $$2 " "}')

build-backend:
	cargo build -p spyglass
	mkdir -p crates/tauri/binaries
	cp target/debug/spyglass crates/tauri/binaries/spyglass-server-$(TARGET_ARCH)

build-client:
	cargo build -p spyglass-client -p spyglass-app

build-styles:
	cd ./crates/client && npx tailwindcss -i ./public/input.css -o ./public/main.css

build-plugins-dev:
# Build chrome-importer plugin
	cd ./plugins/chrome-importer \
		&& cargo build --target wasm32-wasi \
		&& cp target/wasm32-wasi/debug/chrome-importer.wasm ../../assets/plugins/chrome-importer/main.wasm

	cd ./plugins/firefox-importer \
		&& cargo build --target wasm32-wasi \
		&& cp target/wasm32-wasi/debug/firefox-importer.wasm ../../assets/plugins/firefox-importer/main.wasm

	cp -r assets/plugins ~/Library/Application\ Support/com.athlabs.spyglass-dev/

build-plugins-release:
	cd ./plugins/chrome-importer \
		&& cargo build --target wasm32-wasi --release \
		&& cp target/wasm32-wasi/release/chrome-importer.wasm ../../assets/plugins/chrome-importer/main.wasm

	cd ./plugins/firefox-importer \
		&& cargo --target wasm32-wasi --release \
		&& cp target/wasm32-wasi/release/firefox-importer.wasm assets/plugins/firefox-importer/main.wasm

build-release: build-backend build-styles
	cargo tauri build

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
# Install tailwind
	cd ./crates/client && npm install

setup-dev-linux:
	sudo apt install libwebkit2gtk-4.0-dev \
		build-essential \
		curl \
		wget \
		libssl-dev \
		libgtk-3-dev \
		libayatana-appindicator3-dev \
		librsvg2-dev

run-client-dev:
	cargo tauri dev
