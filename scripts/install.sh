#!/usr/bin/env bash
#
# ContextDB Installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/charliewilco/contextdb/main/scripts/install.sh | bash
#
# Or with options:
#   curl -fsSL ... | bash -s -- --prefix=/custom/path
#

set -e

# Configuration
REPO="charliewilco/contextdb"
BINARY_NAME="contextdb"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
info() {
    echo -e "${BLUE}==>${NC} $1"
}

success() {
    echo -e "${GREEN}==>${NC} $1"
}

warn() {
    echo -e "${YELLOW}Warning:${NC} $1"
}

error() {
    echo -e "${RED}Error:${NC} $1"
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix=*)
            INSTALL_DIR="${1#*=}"
            shift
            ;;
        --prefix)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "ContextDB Installer"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --prefix=DIR    Install to DIR (default: ~/.local/bin)"
            echo "  --help, -h      Show this help message"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            ;;
    esac
done

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)     os="linux";;
        Darwin*)    os="darwin";;
        MINGW*|MSYS*|CYGWIN*) os="windows";;
        *)          error "Unsupported operating system: $(uname -s)";;
    esac

    case "$(uname -m)" in
        x86_64|amd64)   arch="x86_64";;
        aarch64|arm64)  arch="aarch64";;
        armv7l)         arch="armv7";;
        *)              error "Unsupported architecture: $(uname -m)";;
    esac

    echo "${os}-${arch}"
}

# Check for required tools
check_dependencies() {
    local missing=()

    for cmd in curl tar; do
        if ! command -v "$cmd" &> /dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing required tools: ${missing[*]}"
    fi
}

# Get the latest release version
get_latest_version() {
    local version
    version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')

    if [[ -z "$version" ]]; then
        error "Could not determine latest version. Check your internet connection or install manually."
    fi

    echo "$version"
}

# Download and install
install_binary() {
    local platform="$1"
    local version="$2"
    local download_url
    local temp_dir

    # Construct download URL
    # Format: https://github.com/user/repo/releases/download/v0.1.0/contextdb-0.1.0-darwin-aarch64.tar.gz
    download_url="https://github.com/${REPO}/releases/download/v${version}/${BINARY_NAME}-${version}-${platform}.tar.gz"

    info "Downloading ${BINARY_NAME} v${version} for ${platform}..."

    # Create temp directory
    temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT

    # Download
    if ! curl -fsSL "$download_url" -o "${temp_dir}/contextdb.tar.gz" 2>/dev/null; then
        # If pre-built binary not available, try building from source
        warn "Pre-built binary not available. Attempting to build from source..."
        install_from_source "$version"
        return
    fi

    # Extract
    info "Extracting..."
    tar -xzf "${temp_dir}/contextdb.tar.gz" -C "$temp_dir"

    # Create install directory if needed
    mkdir -p "$INSTALL_DIR"

    # Install
    info "Installing to ${INSTALL_DIR}..."
    mv "${temp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    success "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"
}

# Build from source using cargo
install_from_source() {
    local version="$1"

    if ! command -v cargo &> /dev/null; then
        error "Rust is not installed. Install it from https://rustup.rs or download a pre-built binary."
    fi

    info "Building from source (this may take a few minutes)..."

    if [[ -n "$version" ]]; then
        cargo install --git "https://github.com/${REPO}" --tag "v${version}"
    else
        cargo install --git "https://github.com/${REPO}"
    fi

    success "Installed ${BINARY_NAME} via cargo"
}

# Check if install directory is in PATH
check_path() {
    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        warn "${INSTALL_DIR} is not in your PATH"
        echo ""
        echo "Add it to your shell profile:"
        echo ""

        local shell_name=$(basename "$SHELL")
        case "$shell_name" in
            bash)
                echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
                echo "  source ~/.bashrc"
                ;;
            zsh)
                echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc"
                echo "  source ~/.zshrc"
                ;;
            fish)
                echo "  fish_add_path ${INSTALL_DIR}"
                ;;
            *)
                echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
                ;;
        esac
        echo ""
    fi
}

# Verify installation
verify_installation() {
    if command -v "$BINARY_NAME" &> /dev/null; then
        local installed_version
        installed_version=$("$BINARY_NAME" --version 2>/dev/null | awk '{print $2}')
        success "Successfully installed ${BINARY_NAME} v${installed_version}"
    elif [[ -x "${INSTALL_DIR}/${BINARY_NAME}" ]]; then
        local installed_version
        installed_version=$("${INSTALL_DIR}/${BINARY_NAME}" --version 2>/dev/null | awk '{print $2}')
        success "Successfully installed ${BINARY_NAME} v${installed_version}"
    else
        error "Installation verification failed"
    fi
}

# Main
main() {
    echo ""
    echo "  ContextDB Installer"
    echo "  ==================="
    echo ""

    check_dependencies

    local platform version

    platform=$(detect_platform)
    info "Detected platform: ${platform}"

    version=$(get_latest_version)
    info "Latest version: v${version}"

    install_binary "$platform" "$version"

    check_path
    verify_installation

    echo ""
    echo "Get started:"
    echo "  contextdb init mydata.db      # Create a database"
    echo "  contextdb --help              # Show all commands"
    echo ""
}

main "$@"
