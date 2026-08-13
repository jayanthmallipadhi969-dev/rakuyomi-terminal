# Makefile for building and deploying rakuyomi CLI/server for Kindle Paperwhite (armv7)
# Usage examples:
#   make build-release           # build release for host
#   make cross-build            # cross-compile (glibc) for armv7
#   make musl-build             # build static musl armv7 (requires `cross`)
#   make strip                  # strip gnueabihf release binary
#   make install KINDLE_HOST=192.168.1.42 KINDLE_USER=root KINDLE_PATH=/mnt/us/rakuyomi/ # scp to device
#   make deploy                 # scp and run via SSH (uses KINDLE_ env vars)

# Configuration
TARGET ?= armv7-unknown-linux-gnueabihf
MUSL_TARGET ?= armv7-unknown-linux-musleabihf
BINARY_NAME ?= rakuyomi-cli
GNUEABI_BINARY = target/$(TARGET)/release/$(BINARY_NAME)
MUSL_BINARY = target/$(MUSL_TARGET)/release/$(BINARY_NAME)
STRIP ?= arm-linux-gnueabihf-strip
CROSS_CMD ?= cross

KINDLE_HOST ?=
KINDLE_USER ?=root
KINDLE_PATH ?=/mnt/us/koreader/rakuyomi
SSH ?= ssh
SCP ?= scp

.PHONY: help build-native build-release cross-build musl-build strip strip-musl install deploy run

help:
	@sed -n '1,120p' Makefile | sed -n '1,120p'

build-native:
	cargo build

build-release:
	cargo build --release

cross-build: ## Cross-compile for glibc armv7 (requires gcc-arm-linux-gnueabihf and rustup target)
	rust="$(shell command -v rustup || true)"; \
	if [ -z "$$trust" ]; then echo "rustup not found: install rustup first (https://rustup.rs)"; exit 1; fi; \
	rustup target add $(TARGET) || true; \
	cargo build --release --target $(TARGET)

strip: ## Strip gnueabihf binary to reduce size
	if [ -f $(GNUEABI_BINARY) ]; then \
		$(STRIP) $(GNUEABI_BINARY) || true; \
		echo "Stripped $(GNUEABI_BINARY)"; \
	else \
		echo "Binary not found: $(GNUEABI_BINARY)"; exit 1; \
	fi

musl-build: ## Build musl static via cross (requires cargo install cross)
	command -v $(CROSS_CMD) >/dev/null 2>&1 || { echo >&2 "'$(CROSS_CMD)' not found. Install with: cargo install --locked cross"; exit 1; }
	$(CROSS_CMD) build --workspace --release --target $(MUSL_TARGET)

strip-musl:
	if [ -f $(MUSL_BINARY) ]; then \
		strip $(MUSL_BINARY) || true; \
		echo "Stripped $(MUSL_BINARY)"; \
	else \
		echo "Binary not found: $(MUSL_BINARY)"; exit 1; \
	fi

install: ## Copy gnueabihf release binary to Kindle via scp
	@if [ -z "$(KINDLE_HOST)" ]; then echo "Set KINDLE_HOST to your device IP e.g. make install KINDLE_HOST=192.168.1.42"; exit 1; fi
	@if [ ! -f $(GNUEABI_BINARY) ]; then echo "Binary not found: $(GNUEABI_BINARY). Run 'make cross-build' first."; exit 1; fi
	mkdir -p $(shell dirname $(KINDLE_PATH)/$(BINARY_NAME)) || true
	$(SCP) $(GNUEABI_BINARY) $(KINDLE_USER)@$(KINDLE_HOST):$(KINDLE_PATH)/

run: ## SSH to Kindle and run the binary
	@if [ -z "$(KINDLE_HOST)" ]; then echo "Set KINDLE_HOST to your device IP e.g. make run KINDLE_HOST=192.168.1.42"; exit 1; fi
	$(SSH) $(KINDLE_USER)@$(KINDLE_HOST) 'chmod +x $(KINDLE_PATH)/$(BINARY_NAME) && $(KINDLE_PATH)/$(BINARY_NAME) --help'

deploy: cross-build install run
	@echo "Deployed and ran $(BINARY_NAME) on $(KINDLE_HOST)"

.PHONY: clean
clean:
	cargo clean
	@echo "Cleaned workspace"