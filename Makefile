# Copy the Makefile.dev.template over to Makefile.dev to setup dev related
# variables.
-include Makefile.dev

.PHONY: build-backend build-client build-plugins-dev build-plugins-release \
	build-release check clippy fmt test test-with-ignored \
	setup-dev setup-dev-linux run-client-dev

TARGET_ARCH := $(shell rustc -Vv | grep host | awk '{print $$2 " "}')
PLUGINS :=

# By default just run fmt & clippy
default: fmt clippy

build-backend:
	cargo build -p spyglass
	mkdir -p apps/tauri/binaries
	cp target/debug/spyglass apps/tauri/binaries/spyglass-server-$(TARGET_ARCH)
	cp target/debug/spyglass-debug apps/tauri/binaries/spyglass-debug-$(TARGET_ARCH)
ifneq ($(strip $(findstring windows,$(TARGET_ARCH))),)
	cp utils/win/pdftotext.exe apps/tauri/binaries/pdftotext-$(strip $(TARGET_ARCH)).exe
else ifneq ($(strip $(findstring mac,$(TARGET_ARCH))),)
	cp utils/mac/pdftotext apps/tauri/binaries/pdftotext-$(strip $(TARGET_ARCH))
else
	cp utils/linux/pdftotext apps/tauri/binaries/pdftotext-$(strip $(TARGET_ARCH))
endif

build-release: build-backend
	cargo tauri build

check:
	cargo check --all

clippy: check fmt
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
# install tauri-cli and node dependencies
	cargo install --locked tauri-cli
	cd ./apps/desktop-client && npm i
# Download whisper model used in development
	mkdir -p assets/models/embeddings
	mkdir -p assets/models/whisper
	curl -L --output model.safetensors https://huggingface.co/openai/whisper-base.en/resolve/main/model.safetensors?download=true
	mv model.safetensors assets/models/whisper
	curl -L --output config.json https://huggingface.co/openai/whisper-base.en/resolve/main/config.json?download=true
	mv config.json assets/models/whisper
	curl -L --output tokenizer.json https://huggingface.co/openai/whisper-base.en/resolve/main/tokenizer.json?download=true
	mv tokenizer.json assets/models/whisper
# Check if .env exists and if not create it
	test -f .env || cp .env.template .env
# Build backend to copy binaries for Tauri
	make build-backend

# Specifically for debian based distros
setup-dev-linux:
	sudo apt update
	sudo apt install libwebkit2gtk-4.1-dev \
		build-essential \
		curl \
		wget \
		file \
		libxdo-dev \
		libssl-dev \
		libayatana-appindicator3-dev \
		librsvg2-dev \
		cmake \
		libsdl2-dev \
		clang

run-backend-dev:
	cargo run -p spyglass

run-client-dev:
	cd apps/tauri && cargo tauri dev

upload-debug-symbols-windows:
	cargo build -p spyglass --profile sentry
	npx sentry-cli difutil check target/sentry/spyglass.exe
	npx sentry-cli upload-dif -o spyglass -p spyglass-server --include-sources target/sentry/spyglass.exe
	mkdir -p apps/tauri/binaries
	cp target/sentry/spyglass.exe apps/tauri/binaries/spyglass-server-x86_64-pc-windows-msvc.exe
	cd crates/client && trunk build
	cargo build -p spyglass-app --profile sentry
	npx sentry-cli difutil check target/sentry/spyglass-app.exe
	npx sentry-cli upload-dif -o spyglass -p spyglass-frontend --include-sources target/sentry/spyglass-app.exe

# Generate typescript bindings for rust <> typescript interop
generate-bindings:
	TS_RS_EXPORT_DIR="../../apps/desktop-client/src/bindings" cargo test -p shared
