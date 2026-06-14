#!/bin/bash
# Install Zelkova binaries (zelkova, zelkovad, zelkova-cli) to ~/.cargo/bin/
# using standard `cargo install --path`. No custom PREFIX, no dual install.
set -euo pipefail

print_help() {
    cat <<'EOF'
Usage: ./install.sh

Installs the three Zelkova binaries to ~/.cargo/bin/ using cargo's
default install location. Make sure ~/.cargo/bin is on your PATH.

Binaries installed:
    zelkova       GUI editor
    zelkovad      Background daemon
    zelkova-cli   Terminal CLI

Flags:
    -h, --help    Show this help and exit
EOF
}

case "${1:-}" in
    -h|--help)
        print_help
        exit 0
        ;;
    "")
        ;;
    *)
        echo "Unknown argument: $1" >&2
        echo "Run './install.sh --help' for usage." >&2
        exit 2
        ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found on PATH." >&2
    echo "Install Rust from https://www.rust-lang.org/tools/install and try again." >&2
    exit 1
fi

CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
INSTALL_DIR="$CARGO_HOME/bin"

echo "Installing Zelkova binaries to $INSTALL_DIR ..."
echo ""

cargo install --path crates/cli --locked
cargo install --path crates/daemon --locked
cargo install --path crates/gui --locked

echo ""
echo "Installed:"
echo "  $INSTALL_DIR/zelkova       — GUI editor"
echo "  $INSTALL_DIR/zelkovad      — Background daemon"
echo "  $INSTALL_DIR/zelkova-cli   — Terminal CLI"
echo ""
echo "Make sure $INSTALL_DIR is on your PATH."
echo "Standard Rust installs already add it; if not, add this to your shell rc:"
echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
