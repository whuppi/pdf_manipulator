#!/bin/sh
# Universal installer for pdf-oxide CLI
# Usage: curl -fsSL oxide.fyi/install.sh | sh
set -eu

REPO="yfedoseev/pdf_oxide"
BINARY_NAME="pdf-oxide"
INSTALL_DIR="${PDF_OXIDE_INSTALL_DIR:-$HOME/.local/bin}"

info() { printf '  \033[1;34m>\033[0m %s\n' "$*"; }
error() { printf '  \033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  PLATFORM="linux" ;;
        Darwin) PLATFORM="macos" ;;
        *)      error "Unsupported OS: $OS" ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH_NAME="x86_64" ;;
        aarch64|arm64) ARCH_NAME="aarch64" ;;
        *)             error "Unsupported architecture: $ARCH" ;;
    esac

    # Detect musl vs glibc on Linux
    MUSL_SUFFIX=""
    if [ "$PLATFORM" = "linux" ] && [ "$ARCH_NAME" = "x86_64" ]; then
        if command -v ldd >/dev/null 2>&1; then
            if ldd --version 2>&1 | grep -qi musl; then
                MUSL_SUFFIX="-musl"
            fi
        elif ls /lib/ld-musl-* >/dev/null 2>&1; then
            MUSL_SUFFIX="-musl"
        fi
    fi

    ARTIFACT="pdf_oxide-${PLATFORM}-${ARCH_NAME}${MUSL_SUFFIX}"
}

get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' | head -1 | sed 's/.*"v\(.*\)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' | head -1 | sed 's/.*"v\(.*\)".*/\1/')
    else
        error "Neither curl nor wget found. Please install one of them."
    fi

    if [ -z "$VERSION" ]; then
        error "Could not determine latest version. Check https://github.com/${REPO}/releases"
    fi
}

download_and_install() {
    URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARTIFACT}-${VERSION}.tar.gz"
    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT

    info "Downloading pdf-oxide v${VERSION} for ${PLATFORM} ${ARCH_NAME}${MUSL_SUFFIX}..."

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$URL" -o "$TMPDIR/archive.tar.gz"
    else
        wget -qO "$TMPDIR/archive.tar.gz" "$URL"
    fi

    info "Extracting..."
    tar xzf "$TMPDIR/archive.tar.gz" -C "$TMPDIR"

    if [ ! -f "$TMPDIR/${BINARY_NAME}" ]; then
        error "Binary '${BINARY_NAME}' not found in archive"
    fi

    info "Installing to ${INSTALL_DIR}..."
    mkdir -p "$INSTALL_DIR"
    mv "$TMPDIR/${BINARY_NAME}" "$INSTALL_DIR/${BINARY_NAME}"
    chmod +x "$INSTALL_DIR/${BINARY_NAME}"

}

check_path() {
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            info ""
            info "Add ${INSTALL_DIR} to your PATH:"
            info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            info ""
            info "To make it permanent, add the above line to your shell profile (~/.bashrc, ~/.zshrc, etc.)"
            ;;
    esac
}

main() {
    info "pdf-oxide installer"
    info ""

    detect_platform
    get_latest_version

    download_and_install

    info ""
    info "Successfully installed ${BINARY_NAME} v${VERSION} to ${INSTALL_DIR}/${BINARY_NAME}"

    check_path

    info ""
    info "Run '${BINARY_NAME} --help' to get started."
}

main
