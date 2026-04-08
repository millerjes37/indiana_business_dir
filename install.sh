#!/usr/bin/env bash
# install.sh — One-line installer for indiana_business_dir prebuilt binaries
# Usage: curl -fsSL https://raw.githubusercontent.com/millerjes37/indiana_business_dir/main/install.sh | bash

set -euo pipefail

REPO="millerjes37/indiana_business_dir"
API_URL="https://api.github.com/repos/${REPO}/releases/latest"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    echo -e "${GREEN}[install]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[install]${NC} $1"
}

error() {
    echo -e "${RED}[install]${NC} $1" >&2
    exit 1
}

# Detect OS and architecture
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)
                TARGET="x86_64-unknown-linux-gnu"
                ;;
            *)
                error "Unsupported architecture: $ARCH on Linux. Only x86_64 is supported."
                ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            x86_64)
                TARGET="x86_64-apple-darwin"
                ;;
            arm64)
                TARGET="aarch64-apple-darwin"
                ;;
            *)
                error "Unsupported architecture: $ARCH on macOS. Only x86_64 and arm64 are supported."
                ;;
        esac
        ;;
    *)
        error "Unsupported operating system: $OS. This installer supports macOS and Linux."
        ;;
esac

info "Detected platform: $TARGET"

# Fetch latest release tag
info "Fetching latest release from GitHub..."
if ! command -v curl >/dev/null 2>&1; then
    error "curl is required but not installed. Please install curl and try again."
fi

TAG=$(curl -fsSL "$API_URL" | grep '"tag_name"' | head -n 1 | sed -E 's/.*"tag_name": "v?([^"]+)".*/\1/')
if [ -z "$TAG" ]; then
    error "Could not determine latest release tag. GitHub API may be rate-limited or unavailable."
fi

info "Latest release: v$TAG"

ARCHIVE_NAME="indiana_business_dir-v${TAG}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${TAG}/${ARCHIVE_NAME}"

# Determine install directory
if [ -n "${INSTALL_DIR:-}" ]; then
    INSTALL_DIR="$INSTALL_DIR"
elif [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
fi

# Determine data directory (where we keep scripts/ data/ node_modules/)
DATA_DIR="${DATA_DIR:-$HOME/.indiana_business_dir}"

info "Install directory: $INSTALL_DIR"
info "Data directory: $DATA_DIR"

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$DATA_DIR"

# Download to temp
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

info "Downloading $ARCHIVE_NAME..."
curl -fsSL -o "$TMP_DIR/$ARCHIVE_NAME" "$DOWNLOAD_URL"

info "Extracting archive..."
tar xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"

# The archive contains a single directory: indiana_business_dir/
EXTRACTED_DIR="$TMP_DIR/indiana_business_dir"
if [ ! -d "$EXTRACTED_DIR" ]; then
    error "Archive did not contain expected 'indiana_business_dir' directory."
fi

# Move contents to DATA_DIR
info "Installing to $DATA_DIR..."
rm -rf "$DATA_DIR"
mv "$EXTRACTED_DIR" "$DATA_DIR"

# Install Node dependencies
info "Installing Node.js dependencies (this may take a minute)..."
cd "$DATA_DIR"
if ! command -v npm >/dev/null 2>&1; then
    warn "npm was not found in your PATH. Please install Node.js 18+ and run 'npm install' in $DATA_DIR manually."
else
    npm install --silent
fi

# Symlink binary
BINARY_NAME="indiana_business_dir"
BINARY_PATH="$DATA_DIR/$BINARY_NAME"
LINK_PATH="$INSTALL_DIR/$BINARY_NAME"

# Remove old symlink if it points somewhere else
if [ -L "$LINK_PATH" ]; then
    rm -f "$LINK_PATH"
fi

ln -sf "$BINARY_PATH" "$LINK_PATH"
info "Created symlink: $LINK_PATH -> $BINARY_PATH"

# Verify binary is executable
if [ ! -x "$BINARY_PATH" ]; then
    error "Binary is not executable: $BINARY_PATH"
fi

# Check if install dir is on PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    warn "$INSTALL_DIR is not in your PATH."
    warn "Add the following line to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
    warn "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

info "Installation complete!"
info "Run '$BINARY_NAME --help' to get started."
