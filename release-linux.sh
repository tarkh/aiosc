#!/bin/sh

# Add required targets if missing
rustup target add x86_64-unknown-linux-musl

# Clean previous builds
cargo clean

# Release
echo "Building release..."
#cargo build --release
# Static release
cargo build --release --target x86_64-unknown-linux-musl
echo "Static release built successfully."

# Pack binary
echo "Packing binary..."
upx --best target/x86_64-unknown-linux-musl/release/aiosc
echo "Binary packed successfully."

# Distribute locally
echo "Distributing locally..."
sudo cp target/x86_64-unknown-linux-musl/release/aiosc /usr/local/bin
echo "Local distribution completed."