#!/bin/bash
# Minimal build script without Xcode-dependent features
# This builds the Meetily app with limited audio support

set -e

echo "=== Building Meetily with Minimal Dependencies ==="
echo "Note: Advanced audio features require Xcode. This build uses basic audio."

cd "$(dirname "$0")"

# Clean previous builds
echo "Cleaning previous builds..."
rm -rf src-tauri/target/debug/build/cidre-*
rm -rf src-tauri/target/debug/deps/libcidre*

# Patch Cargo.toml to remove cidre dependency temporarily
echo "Patching Cargo.toml for minimal build..."
cp src-tauri/Cargo.toml src-tauri/Cargo.toml.backup

# Remove cidre dependency and replace with simpler alternative
sed -i.bak '/cidre = { git/d' src-tauri/Cargo.toml

# Install dependencies
echo "Installing Node.js dependencies..."
pnpm install

# Build with minimal features
echo "Building Tauri app (minimal mode)..."
pnpm run tauri dev -- --features platform-default