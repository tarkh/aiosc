#!/bin/sh

# Add required targets if missing
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Clean previous builds
cargo clean

# Universal Release
echo "Building universal release..."
# Build for both architectures
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Combine with lipo
lipo -create \
    target/x86_64-apple-darwin/release/aiosc \
    target/aarch64-apple-darwin/release/aiosc \
    -output target/release/aiosc
echo "Universal release built successfully."

#
# Distribute locally
echo "Distributing locally..."
sudo cp target/release/aiosc /usr/local/bin
echo "Local distribution completed."