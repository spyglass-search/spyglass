.PHONY: build-backend build-client build-plugins-dev build-plugins-release \
	build-styles build-release check clippy fmt test test-with-ignored \
	setup-dev setup-dev-linux run-client-dev

TARGET_ARCH := $(shell rustc -Vv | grep host | awk '{print $$2 " "}')
PLUGINS := chrome-importer firefox-importer local-file-indexer
# Set this up if you're working on the plugins
PLUGINS_DEV_FOLDER := ~/Library/Application\ Support/com.athlabs.spyglass-dev/

# By default just run fmt & clippy
default: fmt clippy

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

generate-icon:
	cargo tauri icon assets/app-icon.png

test:
	cargo test --all

test-with-ignored:
	cargo test --all -- --ignored

setup-dev:
	rustup target add wasm32-unknown-unknown
# Required for plugin development
	rustup target add wasm32-wasi
# Install tauri-cli & trunk for client development
	cargo install --locked tauri-cli
	cargo install --locked trunk
# Install tailwind
	cd ./crates/client && npm install

# Specifically for debian based distros
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

upload-debug-symbols-windows:
	cargo build -p spyglass --profile sentry
	npx sentry-cli difutil check target/sentry/spyglass.exe
	npx sentry-cli upload-dif -o spyglass -p spyglass-server --include-sources target/sentry/spyglass.exe
	mkdir -p crates/tauri/binaries
	cp target/sentry/spyglass.exe crates/tauri/binaries/spyglass-server-x86_64-pc-windows-msvc.exe
	cd crates/client && trunk build
	cargo build -p spyglass-app --profile sentry
	npx sentry-cli difutil check target/sentry/spyglass-app.exe
	npx sentry-cli upload-dif -o spyglass -p spyglass-frontend --include-sources target/sentry/spyglass-app.exe