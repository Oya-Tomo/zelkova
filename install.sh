#!/bin/bash
set -e

PREFIX="${1:-$HOME/.local}"

echo "Installing Zelkova to $PREFIX/bin ..."

cargo install --path crates/cli --root "$PREFIX"
cargo install --path crates/daemon --root "$PREFIX"
cargo install --path crates/gui --root "$PREFIX"

echo ""
echo "Installed:"
echo "  $PREFIX/bin/zelkova      — GUI"
echo "  $PREFIX/bin/zelkovad     — Daemon"
echo "  $PREFIX/bin/zelkova-cli  — CLI"
echo ""
echo "Make sure $PREFIX/bin is in your PATH."
