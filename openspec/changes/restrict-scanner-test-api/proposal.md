## Why

The scanner subsystem and its supporting checkout manager still expose test and helper entry points that appear to be used to shortcut implementation details rather than exercise observable behaviour. That makes some integration tests depend on internal structure, keeps dead-code allowances alive, and weakens the intended public boundary of the scanner.

This change is needed now because the dead-code audit has isolated the remaining allowances to scanner and checkout-manager test-support surface. The project should decide deliberately whether those entry points belong in the supported API or should be removed, hidden from non-test builds, or replaced with behaviour-oriented test coverage.

## What Changes

- Audit the scanner and checkout-manager test/helper API surface structurally rather than as isolated methods.
- Identify integration tests that currently rely on non-public or shortcut access paths and refactor them to use public APIs and observable behaviour instead.
- Remove dead code, hidden shortcuts, and implementation-coupled helper entry points where they are no longer justified.
- Examine any remaining non-public API use case by case and keep only the cases that cannot reasonably be replaced, with explicit justification.

## Capabilities

### New Capabilities
- `scanner-test-api-boundary`: Define and enforce the boundary between scanner production APIs, test-only helpers, and behaviour-oriented integration tests.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/scanner/task/*`, `src/scanner/checkout/manager.rs`, scanner-related tests in `src/scanner/tests/`, `src/scanner/task/tests/`, `src/scanner/checkout/tests/`, and `tests/`.
- Affected systems: scanner public API surface, checkout-manager helper surface, integration test design, and remaining dead-code cleanup in the scanner subsystem.
