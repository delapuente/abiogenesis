#!/bin/bash

# Abiogenesis Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/delapuente/abiogenesis/main/install.sh | bash

set -e

REPO_URL="https://github.com/delapuente/abiogenesis"
BINARY_NAME="ergo"
INSTALL_DIR="$HOME/.local/bin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════╗"
    echo "║           Abiogenesis Installer      ║"
    echo "║                                      ║"
    echo "║   AI-powered command interceptor     ║"
    echo "║   cogito, ergo sum                   ║"
    echo "╚══════════════════════════════════════╝"
    echo -e "${NC}"
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check system requirements
check_requirements() {
    print_status "Checking system requirements..."
    
    # Check if Rust is installed
    if ! command_exists rustc || ! command_exists cargo; then
        print_error "Rust is not installed. Please install Rust first:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo "  source ~/.cargo/env"
        exit 1
    fi
    
    # Check if Deno is installed
    if ! command_exists deno; then
        print_warning "Deno is not installed. Installing Deno..."
        if command_exists curl; then
            curl -fsSL https://deno.land/install.sh | sh
            export PATH="$HOME/.deno/bin:$PATH"
        else
            print_error "curl is required but not installed. Please install curl or Deno manually:"
            echo "  https://deno.land/manual/getting_started/installation"
            exit 1
        fi
    fi
    
    # Check if git is available
    if ! command_exists git; then
        print_error "git is required but not installed."
        exit 1
    fi
    
    print_status "✓ All requirements met"
}

# Create install directory
create_install_dir() {
    if [ ! -d "$INSTALL_DIR" ]; then
        print_status "Creating install directory: $INSTALL_DIR"
        mkdir -p "$INSTALL_DIR"
    fi
}

# Download and build
install_abiogenesis() {
    print_status "Installing Abiogenesis..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Clone repository
    print_status "Downloading source code..."
    git clone "$REPO_URL" abiogenesis
    cd abiogenesis
    
    # Build in release mode
    print_status "Building ergo binary (this may take a few minutes)..."
    cargo build --release
    
    # Copy binary to install directory
    print_status "Installing binary to $INSTALL_DIR"
    cp target/release/$BINARY_NAME "$INSTALL_DIR/"
    
    # Make sure it's executable
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    
    # Clean up
    cd /
    rm -rf "$TEMP_DIR"
}

# Update PATH if needed
update_path() {
    # Check if install directory is in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        print_warning "$INSTALL_DIR is not in your PATH"
        echo ""
        echo "To use ergo from anywhere, add this line to your shell profile:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
        echo "Common shell profiles:"
        echo "  ~/.bashrc (Bash)"
        echo "  ~/.zshrc (Zsh)"
        echo "  ~/.config/fish/config.fish (Fish)"
        echo ""
        
        # Try to detect shell and offer to add automatically
        if [[ -n "$ZSH_VERSION" ]] && [[ -f "$HOME/.zshrc" ]]; then
            echo -n "Add to ~/.zshrc automatically? [y/N] "
            read -r response
            if [[ "$response" =~ ^[Yy]$ ]]; then
                echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$HOME/.zshrc"
                print_status "Added to ~/.zshrc. Restart your shell or run: source ~/.zshrc"
            fi
        elif [[ -n "$BASH_VERSION" ]] && [[ -f "$HOME/.bashrc" ]]; then
            echo -n "Add to ~/.bashrc automatically? [y/N] "
            read -r response
            if [[ "$response" =~ ^[Yy]$ ]]; then
                echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$HOME/.bashrc"
                print_status "Added to ~/.bashrc. Restart your shell or run: source ~/.bashrc"
            fi
        fi
    else
        print_status "✓ $INSTALL_DIR is already in your PATH"
    fi
}

# Test installation
test_installation() {
    print_status "Testing installation..."
    
    if [ -x "$INSTALL_DIR/$BINARY_NAME" ]; then
        print_status "✓ Binary installed successfully"
        
        # Test if it's in PATH
        if command_exists $BINARY_NAME; then
            print_status "✓ ergo is available in PATH"
            "$BINARY_NAME" --help > /dev/null 2>&1 || true
        else
            print_warning "ergo is installed but not in PATH"
        fi
    else
        print_error "Installation failed - binary not found"
        exit 1
    fi
}

# Show next steps
show_next_steps() {
    echo ""
    echo -e "${GREEN}🎉 Installation complete!${NC}"
    echo ""
    echo "Next steps:"
    echo ""
    echo "1. Ensure ergo is in your PATH (see above if needed)"
    echo ""
    echo "2. Set up your Anthropic API key:"
    echo "   ergo --set-api-key sk-ant-your-key-here"
    echo "   (Get your key from: https://console.anthropic.com)"
    echo ""
    echo "3. Try some commands:"
    echo "   ergo hello world"
    echo "   ergo timestamp"
    echo "   ergo \"show me the current time in a nice format\""
    echo ""
    echo "4. For testing without API key:"
    echo "   ABIOGENESIS_USE_MOCK=1 ergo hello"
    echo ""
    echo "5. Get help:"
    echo "   ergo --help"
    echo "   ergo --config"
    echo ""
    echo "Happy commanding! 🚀"
}

# Show help
show_help() {
    print_header
    echo "Usage: $0 [INSTALL_DIR]"
    echo ""
    echo "Install Abiogenesis (ergo) command interceptor"
    echo ""
    echo "Options:"
    echo "  INSTALL_DIR    Custom installation directory (default: ~/.local/bin)"
    echo "  -h, --help     Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Install to ~/.local/bin"
    echo "  $0 /usr/local/bin     # Install to /usr/local/bin"
    echo ""
    echo "Environment variables:"
    echo "  REPO_URL      Custom repository URL"
    echo ""
}

# Main installation flow
main() {
    # Handle help option
    if [[ "$1" == "-h" || "$1" == "--help" ]]; then
        show_help
        exit 0
    fi
    
    print_header
    
    # Allow user to specify custom install directory
    if [[ -n "$1" ]]; then
        INSTALL_DIR="$1"
        print_status "Using custom install directory: $INSTALL_DIR"
    fi
    
    check_requirements
    create_install_dir
    install_abiogenesis
    update_path
    test_installation
    show_next_steps
}

# Run main function with all arguments
main "$@"
