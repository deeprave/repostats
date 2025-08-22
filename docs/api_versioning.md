# API Versioning System

## Overview

The repostats project uses a **Cargo.toml-based API versioning system** that provides stable, reproducible versions across all developers and environments while allowing for manual increments when breaking changes occur.

## How It Works

### Source-Controlled Versioning
- API version is defined in `Cargo.toml` under `package.metadata.plugin_api_version`
- Build script reads version from Cargo.toml and generates constant at build time
- **Same source code always produces same API version** across all developers
- No dependency on build date or environment

### Current Version
```bash
# Check current API version in source
grep 'plugin_api_version' Cargo.toml

# Check generated constant
grep 'BASE_API_VERSION' target/*/build/repostats-*/out/version.rs
```

### Version Increment Process
```bash
# Method 1: Use the provided script
./scripts/increment_api_version.sh

# Method 2: Manual edit
# Edit Cargo.toml: package.metadata.plugin_api_version = YYYYMMDD
# Commit the change and rebuild
```

## Benefits

### Reproducible Builds
- ✅ **Same source = same version**: All developers get identical API versions
- ✅ **Source controlled**: Version changes are tracked in git history
- ✅ **CI/CD friendly**: Build servers produce same versions as development machines
- ✅ **Plugin stability**: Consistent API versioning for plugin compatibility

### Development Workflow
- 🔧 **Easy increment**: Edit single line in Cargo.toml
- 📝 **Clear audit trail**: Version changes visible in git commits
- 🚀 **Zero setup**: No environment variables or special build requirements

Whenever api_version version changes, regardless the method, `Cargo.toml` needs to be committed.

## Version Format

Uses **YYYYMMDD** format for human-readable dates:
- `20250727` = 27 July 2025
- `20250801` = 1 August 2025
- `20251215` = 15 December 2025

## Implementation Details

### Cargo.toml Configuration
```toml
[package.metadata]
plugin_api_version = 20250822
```

### Build Script (`build.rs`)
- Reads `package.metadata.plugin_api_version` from Cargo.toml
- Generates `version.rs` with `BASE_API_VERSION` constant
- Triggers rebuild when Cargo.toml changes
- Implements comprehensive version generation including command name and CLI interface integration

### Version Module Integration
The build script generates version constants that are included at compile time, providing stable version information throughout the application. This integrates with the three-stage CLI parsing system to ensure consistent version reporting across all application components.

## When to Increment

Increment the API version when making:
- ✋ **Breaking changes** to CLI interfaces or argument structures
- 📦 **Configuration format changes** affecting TOML parsing
- ⚙️ **Plugin architecture** modifications
- 🔌 **Module interface** updates affecting public APIs

## Development Integration

The versioning system is integrated with the repostats build process:
- Version generation occurs during `cargo build`
- Generated constants are available throughout the application
- Command name extraction from Cargo.toml package metadata
- Integration with the three-stage CLI parsing architecture