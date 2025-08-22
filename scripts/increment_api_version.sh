#!/bin/bash
# API Version Increment Script  
# This script increments the API version by updating Cargo.toml

echo "Current API version: $(grep 'api_version' Cargo.toml | grep -o '[0-9]*')"

# Get current date in YYYYMMDD format
NEW_VERSION=$(date +%Y%m%d)

echo "Updating API version to: $NEW_VERSION"

# Update Cargo.toml with new version
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS sed
    sed -i '' "s/api_version = [0-9]*/api_version = $NEW_VERSION/" Cargo.toml
else
    # Linux sed
    sed -i "s/api_version = [0-9]*/api_version = $NEW_VERSION/" Cargo.toml
fi

echo "Building to use new API version..."
cargo build --lib

echo "New API version: $(grep 'api_version' Cargo.toml | grep -o '[0-9]*')"
echo ""
echo "API version incremented successfully!"
echo ""
echo "The new version is now committed to Cargo.toml and will be"
echo "used consistently by all developers until manually changed."
echo ""
echo "Don't forget to commit this change to source control!"
