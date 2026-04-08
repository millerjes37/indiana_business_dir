#!/usr/bin/env bash
# run.sh — Unix-compatible wrapper for indiana_business_dir
# Usage: ./run.sh [COMMAND] [ARGS...]
#
# Commands:
#   build              cargo build --release
#   gas-city           scrape Gas City, Grant County (headful, city mode)
#   resume-gas-city    resume Gas City scrape
#   export-grant       export Grant County CSV without scraping
#   help               show this message
#   *                  pass through to the release binary
#
# Examples:
#   ./run.sh build
#   ./run.sh gas-city
#   ./run.sh export-grant

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_BIN="${CARGO:-$(command -v cargo 2>/dev/null || echo "$HOME/.cargo/bin/cargo")}"
RELEASE_BIN=""
LOCAL_LINK="$SCRIPT_DIR/indiana_business_dir"

# Resolve release binary path from cargo metadata (robust against CARGO_TARGET_DIR overrides)
if command -v "$CARGO_BIN" >/dev/null 2>&1; then
    TARGET_DIR="$($CARGO_BIN metadata --format-version=1 --no-deps 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('target_directory',''))")"
    if [[ -n "$TARGET_DIR" ]]; then
        RELEASE_BIN="${TARGET_DIR}/release/indiana_business_dir"
    fi
fi

# Fallback if metadata fails
if [[ -z "$RELEASE_BIN" || ! -x "$RELEASE_BIN" ]]; then
    RELEASE_BIN="$SCRIPT_DIR/target/release/indiana_business_dir"
fi

build_release() {
    echo "==> Building release binary..."
    cd "$SCRIPT_DIR"
    "$CARGO_BIN" build --release
    echo "==> Built: $RELEASE_BIN"
}

link_binary() {
    if [[ -L "$LOCAL_LINK" ]]; then
        rm -f "$LOCAL_LINK"
    fi
    ln -sf "$RELEASE_BIN" "$LOCAL_LINK"
    echo "==> Symlinked: $LOCAL_LINK -> $RELEASE_BIN"
}

case "${1:-}" in
    build)
        build_release
        link_binary
        ;;
    gas-city)
        if [[ ! -x "$RELEASE_BIN" ]]; then
            build_release
        fi
        link_binary
        echo "==> Scraping Gas City, Grant County (headful mode)..."
        "$LOCAL_LINK" scrape --county "Grant" --city "Gas City" --search-mode city --headful "${@:2}"
        ;;
    resume-gas-city)
        if [[ ! -x "$RELEASE_BIN" ]]; then
            build_release
        fi
        link_binary
        echo "==> Resuming Gas City, Grant County (headful mode)..."
        "$LOCAL_LINK" scrape --county "Grant" --city "Gas City" --search-mode city --headful --resume "${@:2}"
        ;;
    export-grant)
        if [[ ! -x "$RELEASE_BIN" ]]; then
            build_release
        fi
        link_binary
        echo "==> Exporting Grant County records to CSV..."
        "$LOCAL_LINK" export --county "Grant" "${@:2}"
        ;;
    help|--help|-h)
        cat << 'EOF'
run.sh — Indiana Business Directory Scraper Wrapper

SYNOPSIS
    ./run.sh [COMMAND] [OPTIONS...]

COMMANDS
    build
        Compile the release binary via cargo build --release,
        then symlink it to ./indiana_business_dir for easy access.

    gas-city
        Scrape all registered businesses in Gas City, Grant County.
        This launches a visible browser (--headful) so you can solve
        the Indiana SOS reCAPTCHA manually. Uses city search mode.

    resume-gas-city
        Resume an interrupted Gas City scrape. Already-discovered and
        already-enriched businesses are skipped.

    export-grant
        Export the current Grant County SQLite records to CSV without
        opening a browser or performing any network requests.

    help, --help, -h
        Show this message.

    (anything else)
        Pass all arguments directly to the release binary.
        Example: ./run.sh scrape --county "Marion" --search-mode city --headful

ENVIRONMENT
    CARGO             Path to cargo executable (default: $HOME/.cargo/bin/cargo)
    RUST_LOG          Set to "debug" or "trace" for verbose logging.

FILES
    scripts/browser_driver.js    Playwright stealth browser driver
    data/in_zips.json            County → ZIP code mappings
    data/in_cities.json          County → city/town mappings
    indiana_business_dir.db      Local SQLite database
    outputs/                     Generated CSV exports
    indiana_business_dir         Symlink to the actual release binary

NOTES
    - You MUST use --headful when scraping live data; Indiana SOS serves
      a reCAPTCHA challenge on every new search submission.
    - Pagination within a single search result does NOT trigger additional
      CAPTCHAs, so the scraper paginates exhaustively before moving on.
    - This script resolves the real binary path from cargo metadata, so it
      works even if CARGO_TARGET_DIR is overridden in your environment.
    - After building, a symlink ./indiana_business_dir is created so you
      can run the binary directly without typing the full cargo target path.
EOF
        ;;
    *)
        if [[ ! -x "$RELEASE_BIN" ]]; then
            build_release
        fi
        link_binary
        cd "$SCRIPT_DIR"
        exec "$LOCAL_LINK" "$@"
        ;;
esac
