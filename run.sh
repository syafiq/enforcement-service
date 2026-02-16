#!/bin/bash

# Quick start script for HAL Enforcement Service

set -e

echo "=================================="
echo "HAL Enforcement Service Quick Start"
echo "=================================="
echo ""

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Cargo not found. Please install Rust first."
    echo "Visit: https://rustup.rs/"
    exit 1
fi

# Build the service
echo "Building enforcement service..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✓ Build successful"
else
    echo "✗ Build failed"
    exit 1
fi

echo ""
echo "Choose a policy configuration:"
echo "  1) Example policy (development)"
echo "  2) Production policy (recommended for production)"
echo "  3) Custom policy path"
read -p "Enter choice [1-3]: " choice

case $choice in
    1)
        POLICY_FILE="policies/example.yaml"
        ;;
    2)
        POLICY_FILE="policies/production.yaml"
        ;;
    3)
        read -p "Enter custom policy path: " POLICY_FILE
        ;;
    *)
        echo "Invalid choice. Using example policy."
        POLICY_FILE="policies/example.yaml"
        ;;
esac

# Validate policy
echo ""
echo "Validating policy: $POLICY_FILE"
cargo run --release -- --policy "$POLICY_FILE" --validate-only

if [ $? -ne 0 ]; then
    echo "✗ Policy validation failed"
    exit 1
fi

echo "✓ Policy is valid"
echo ""

# List entities
echo "Entities in policy:"
cargo run --release -- --policy "$POLICY_FILE" --list-entities

echo ""
read -p "Port to listen on [8080]: " PORT
PORT=${PORT:-8080}

echo ""
echo "Starting HAL Enforcement Service..."
echo "Press Ctrl+C to stop"
echo ""

cargo run --release -- --policy "$POLICY_FILE" --port "$PORT"
