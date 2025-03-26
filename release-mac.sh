#!/bin/bash

# Add required targets if missing
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Clean previous builds
cargo clean

# Build Intel (x86_64) version
echo "Building Intel (x86_64) version..."
cargo build --release --target x86_64-apple-darwin

# Build ARM64 (M1/M2) version
echo "Building ARM64 (Apple Silicon) version..."
cargo build --release --target aarch64-apple-darwin

# Create zip archives for distribution
echo "Creating distribution packages..."
zip -j target/release/aiosc_x86_64-macos.zip target/x86_64-apple-darwin/release/aiosc
zip -j target/release/aiosc_arm64-macos.zip target/aarch64-apple-darwin/release/aiosc

# Verify architectures
echo "Build complete. Binary info:"
file target/x86_64-apple-darwin/release/aiosc
file target/aarch64-apple-darwin/release/aiosc

echo "Packages created in release/:"
ls -lh target/release/*.zip

# Distribute locally
echo "Distributing locally..."
#sudo cp target/x86_64-apple-darwin/release/aiosc /usr/local/bin
sudo cp target/aarch64-apple-darwin/release/aiosc /usr/local/bin
echo "Local distribution completed."