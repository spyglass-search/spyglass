.PHONY: build-backend build-client build-plugins-dev build-plugins-release \
	build-styles build-release check clippy fmt test test-with-ignored \
	setup-dev setup-dev-linux run-client-dev

TARGET_ARCH := $(shell rustc -Vv | grep host | awk '{print $$2 " "}')
PLUGINS := chrome-importer firefox-importer local-file-indexer
# Set this up if you're working on the plugins
PLUGINS_DEV_FOLDER := ~/Library/Application\ Support/com.athlabs.spyglass-dev/

build-backend:
	cargo build -p spyglass
	mkdir -p crates/tauri/binaries
	cp target/debug/spyglass crates/tauri/binaries/spyglass-server-$(TARGET_ARCH)

build-client:
	cargo build -p spyglass-client -p spyglass-app

build-styles:
	cd ./crates/client && npx tailwindcss -i ./public/input.css -o ./public/main.css

build-plugins-dev:
	@for plugin in $(PLUGINS); do \
		echo "-> building $${plugin}"; \
		mkdir -p "assets/plugins/$${plugin}"; \
		cargo build -p $$plugin --target wasm32-wasi; \
		cp target/wasm32-wasi/debug/$$plugin.wasm assets/plugins/$$plugin/main.wasm; \
	done
	mkdir -p $(PLUGINS_DEV_FOLDER);
	cp -r assets/plugins $(PLUGINS_DEV_FOLDER);

build-plugins-release:
	@for plugin in $(PLUGINS); do \
		echo "-> building $${plugin}"; \
		mkdir -p "assets/plugins/$${plugin}"; \
		cargo build -p $$plugin --target wasm32-wasi --release; \
		cp target/wasm32-wasi/release/$$plugin.wasm assets/plugins/$$plugin/main.wasm; \
	done

build-release: build-backend build-styles build-plugins-release
	cargo tauri build

check:
	cargo check --all

clippy:
	cargo clippy --all

fmt:
	cargo fmt --all

test:
	cargo test --all

test-with-ignored:
	cargo test --all -- --ignored

setup-dev:
	rustup target add wasm32-unknown-unknown
# Required for plugin development
	rustup target add wasm32-wasi
# Install tauri-cli & trunk for client development
	cargo install tauri-cli --locked --version ^1.1
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

run-client-headless:
	cd ./crates/client && HEADLESS_CLIENT=true trunk serve