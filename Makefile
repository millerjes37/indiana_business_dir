# Makefile for indiana_business_dir
# Compatible with macOS and Linux. Avoids shell aliases by using full paths.
# NOTE: If CARGO_TARGET_DIR is set in your environment, binaries are placed
# there rather than ./target/. This Makefile resolves the actual path via
# cargo metadata so builds and runs always align.

CARGO_BIN := $(shell which cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)
RUSTC_BIN := $(shell which rustc 2>/dev/null || echo $(HOME)/.cargo/bin/rustc)
NODE_BIN := $(shell which node 2>/dev/null || echo $(HOME)/.local/share/npm/bin/node)
NPM_BIN := $(shell which npm 2>/dev/null || echo $(HOME)/.local/share/npm/bin/npm)

# Resolve the real target directory from cargo metadata using Python (robust against JSON escaping)
CARGO_TARGET_DIR := $(shell $(CARGO_BIN) metadata --format-version=1 --no-deps 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['target_directory'])")

# If metadata failed, fall back to env var or ./target
ifeq ($(CARGO_TARGET_DIR),)
  CARGO_TARGET_DIR := $(or $(CARGO_TARGET_DIR),./target)
endif

RELEASE_BIN := $(CARGO_TARGET_DIR)/release/indiana_business_dir
DEBUG_BIN := $(CARGO_TARGET_DIR)/debug/indiana_business_dir
LOCAL_LINK := ./indiana_business_dir

.PHONY: all build release link run run-gas-city resume-gas-city export-grant debug test clean install-deps help

all: release link

## Ensure Node dependencies are present
install-deps:
	@echo "==> Installing Node.js dependencies..."
	$(NPM_BIN) install

## Build debug binary
build:
	@echo "==> Building debug binary with $(CARGO_BIN) ..."
	"$(CARGO_BIN)" build
	@echo "==> Debug binary: $(DEBUG_BIN)"

## Build optimized release binary
release:
	@echo "==> Building release binary with $(CARGO_BIN) ..."
	"$(CARGO_BIN)" build --release
	@echo "==> Release binary: $(RELEASE_BIN)"

## Create a local symlink so you can run ./indiana_business_dir directly
link: release
	@if [[ -L "$(LOCAL_LINK)" ]]; then rm -f "$(LOCAL_LINK)"; fi
	ln -sf "$(RELEASE_BIN)" "$(LOCAL_LINK)"
	@echo "==> Symlinked: $(LOCAL_LINK) -> $(RELEASE_BIN)"

## Run the release binary interactively (county list)
run: release link
	$(LOCAL_LINK) list

## Run the release binary for Gas City, headful mode (solves CAPTCHA in browser)
run-gas-city: release link
	$(LOCAL_LINK) scrape --county "Grant" --city "Gas City" --search-mode city --headful

## Resume Gas City scrape
resume-gas-city: release link
	$(LOCAL_LINK) scrape --county "Grant" --city "Gas City" --search-mode city --headful --resume

## Export existing Grant County DB records to CSV without scraping
export-grant: release link
	$(LOCAL_LINK) export --county "Grant"

## Run with debug binary and tracing enabled
debug: build
	RUST_LOG=debug $(DEBUG_BIN) scrape --county "Grant" --city "Gas City" --search-mode city --headful

## Run cargo tests
test:
	"$(CARGO_BIN)" test

## Clean build artifacts
clean:
	"$(CARGO_BIN)" clean
	rm -rf node_modules
	@if [[ -L "$(LOCAL_LINK)" ]]; then rm -f "$(LOCAL_LINK)"; fi

## Show this help
help:
	@echo "Indiana Business Directory Scraper — Makefile Targets"
	@echo ""
	@echo "  make install-deps    Install npm dependencies (playwright + stealth)"
	@echo "  make build           Build debug Rust binary"
	@echo "  make release         Build optimized release Rust binary"
	@echo "  make link            Symlink the release binary to ./indiana_business_dir"
	@echo "  make run             Run release binary with interactive county list"
	@echo "  make run-gas-city    Quick target: scrape Gas City, Grant County"
	@echo "  make resume-gas-city Resume an interrupted Gas City scrape"
	@echo "  make export-grant    Export current Grant County DB to CSV"
	@echo "  make debug           Run debug build with full RUST_LOG=debug"
	@echo "  make test            Run cargo test"
	@echo "  make clean           Remove build artifacts, node_modules, and symlink"
	@echo "  make help            Show this message"
	@echo ""
	@echo "Important:"
	@echo "  - You MUST use --headful when scraping live data; Indiana SOS serves"
	@echo "    a reCAPTCHA challenge on every new search submission."
	@echo "  - Because CARGO_TARGET_DIR may be overridden in your shell, this"
	@echo "    Makefile resolves the actual binary path via cargo metadata."
	@echo "  - 'make link' creates ./indiana_business_dir as a symlink so you can"
	@echo "    run the binary directly without memorizing the cargo target path."
