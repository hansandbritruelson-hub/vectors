#!/bin/bash
set -e

# Get the absolute path of the project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export RUSTUP_HOME="$PROJECT_ROOT/.rustup_home"
export CARGO_HOME="$PROJECT_ROOT/.cargo_home"

# Add local toolchain to PATH
TOOLCHAIN_NAME=$(ls "$RUSTUP_HOME/toolchains" | head -n 1)
if [ -z "$TOOLCHAIN_NAME" ]; then
    echo "Error: No toolchain found in $RUSTUP_HOME/toolchains"
    exit 1
fi
TOOLCHAIN_BIN="$RUSTUP_HOME/toolchains/$TOOLCHAIN_NAME/bin"
export PATH="$TOOLCHAIN_BIN:$CARGO_HOME/bin:$PATH"

echo "Building engine with local toolchain..."
echo "RUSTUP_HOME: $RUSTUP_HOME"
echo "CARGO_HOME: $CARGO_HOME"
echo "PATH: $PATH"

# Remove macOS quarantine flag if it exists (fixes "Operation not permitted" on binaries)
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Removing quarantine flags from local toolchain..."
    xattr -rd com.apple.quarantine "$RUSTUP_HOME" "$CARGO_HOME" 2>/dev/null || true
fi

cd "$PROJECT_ROOT/engine"

# Ensure wasm32-unknown-unknown target is available (it should be, but just in case)
# rustup target add wasm32-unknown-unknown --toolchain stable-aarch64-apple-darwin

cargo build --target wasm32-unknown-unknown --release

echo "Generating WASM bindings..."
wasm-bindgen \
    "$PROJECT_ROOT/engine/target/wasm32-unknown-unknown/release/engine.wasm" \
    --out-dir "$PROJECT_ROOT/web/src/pkg" \
    --target web

echo "Build complete! Artifacts in web/src/pkg"
