## Why

The recent `clippy -D warnings` cleanup included a number of targeted `#[allow(dead_code)]` annotations on APIs that were treated as compatibility surface, documented boundary, or future-facing helper surface. That got the repository back to a lint-clean baseline, but it also creates a risk that speculative or obsolete APIs remain in the tree longer than they should.

If these allowances are not audited promptly, they can hide design drift and weaken the project’s preference for minimum-first public surfaces and YAGNI-driven cleanup.

The general repository policy for this is now recorded in [ADR-0004](../../adr/ADR-0004-allow-dead-code-policy.md). This change exists to apply that policy to the current post-cleanup allowance set.

## What Changes

- Audit the `#[allow(dead_code)]` annotations added during the facade and lint-cleanup work.
- Classify each allowed item as:
  - still justified compatibility/documented boundary
  - removable now
  - needing an explicit architectural decision
- Remove allowances and unused APIs where they are no longer justified.
- Keep only the smallest set of dead-code allowances that are intentionally supported and explicitly defensible.

## Capabilities

### New Capabilities
- `dead-code-audit`: Define the policy and cleanup work for reviewing intentionally allowed dead code after the facade/lint cleanup.

### Modified Capabilities
- None.

## Impact

- Affected code: recently annotated items across `src/core/`, `src/notifications/`, `src/plugin/`, `src/queue/`, and `src/scanner/`.
- Affected systems: API surface hygiene, lint policy, compatibility boundaries, and follow-up architectural cleanup.
