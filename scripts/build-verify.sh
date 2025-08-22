#!/usr/bin/env bash
set -euo pipefail

echo "🔧 Build Verification Script for gstats"
echo "======================================="

# Test debug build
echo "📦 Building debug target..."
cargo build

echo "🧪 Testing debug binary..."
DEBUG_OUTPUT=$(./target/debug/gstats)
if [ "$DEBUG_OUTPUT" = "Hello, world!" ]; then
    echo "✅ Debug build: Output correct"
else
    echo "❌ Debug build: Output incorrect. Got: '$DEBUG_OUTPUT'"
    exit 1
fi

# Test release build
echo "📦 Building release target..."
cargo build --release

echo "🧪 Testing release binary..."
RELEASE_OUTPUT=$(./target/release/gstats)
if [ "$RELEASE_OUTPUT" = "Hello, world!" ]; then
    echo "✅ Release build: Output correct"
else
    echo "❌ Release build: Output incorrect. Got: '$RELEASE_OUTPUT'"
    exit 1
fi

# Compare binary sizes
DEBUG_SIZE=$(stat -f%z target/debug/gstats)
RELEASE_SIZE=$(stat -f%z target/release/gstats)

echo "📊 Binary size comparison:"
echo "   Debug:   ${DEBUG_SIZE} bytes"
echo "   Release: ${RELEASE_SIZE} bytes"

if [ $RELEASE_SIZE -lt $DEBUG_SIZE ]; then
    echo "✅ Release binary is smaller than debug (optimized)"
else
    echo "⚠️  Release binary is not smaller than debug"
fi

# Test cargo check
echo "🔍 Running cargo check..."
cargo check
echo "✅ Cargo check passed"

# Test cargo test (when we have tests)
echo "🧪 Running cargo test..."
cargo test
echo "✅ Cargo test passed"

echo "🎉 All build verification tests passed!"
