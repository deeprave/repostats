#!/bin/bash
# API Version Increment Script
# This script increments the API version by updating Cargo.toml

# Extract version from [package.metadata] section specifically
echo "Current API version: $(awk '/^\[package.metadata\]/ {found=1} found && /plugin_api_version/ {print $0; exit}' Cargo.toml | grep -o '[0-9]\{8\}')"

# Get current date in YYYYMMDD format
NEW_VERSION=$(date +%Y%m%d)

echo "Updating API version to: $NEW_VERSION"

# Update Cargo.toml with new version - only in [package.metadata] section
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS sed - update only within [package.metadata] section
    sed -i '' "/^\[package.metadata\]/,/^\[/ s/plugin_api_version = [0-9]\{8\}/plugin_api_version = $NEW_VERSION/" Cargo.toml
else
    # Linux sed - update only within [package.metadata] section
    sed -i "/^\[package.metadata\]/,/^\[/ s/plugin_api_version = [0-9]\{8\}/plugin_api_version = $NEW_VERSION/" Cargo.toml
fi

echo "Building to use new API version..."
cargo build --lib

echo "New API version: $(awk '/^\[package.metadata\]/ {found=1} found && /plugin_api_version/ {print $0; exit}' Cargo.toml | grep -o '[0-9]\{8\}')"
echo ""
echo "API version incremented successfully!"
echo ""
echo "The new version is now committed to Cargo.toml and will be"
echo "used consistently by all developers until manually changed."
echo ""
echo "Don't forget to commit this change to source control!"
