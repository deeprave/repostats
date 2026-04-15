## Context

The `introduce-service-facades` change restored a clean `cargo clippy --all-targets --all-features -- -D warnings` baseline, but part of that cleanup used targeted `#[allow(dead_code)]` annotations across `core`, `notifications`, `plugin`, `queue`, and `scanner`.

Some of those allowances protect APIs that already existed and are plausibly part of a compatibility or documented boundary. Others may simply be speculative surface that survived because removing it was outside the scope of the facade change.

That distinction matters. If the codebase keeps `#[allow(dead_code)]` on APIs that have no real callers, no documented contract, and no active migration need, then the repo becomes lint-clean by policy suppression rather than by improved design. That would directly conflict with the project’s minimum-first export policy and YAGNI-driven cleanup.

This change therefore treats the current set of dead-code allowances as audit input, not as permanent architectural decisions.

The governing repository policy is recorded in [ADR-0004](../../adr/ADR-0004-allow-dead-code-policy.md). This design applies that policy to the concrete allowance set created during the recent clippy cleanup.

## Goals / Non-Goals

**Goals:**

- identify every `#[allow(dead_code)]` added during the recent lint cleanup
- classify each allowed item as justified, removable, or needing an explicit architectural decision
- remove dead-code allowances and unused APIs where there is no concrete production, test, or supported-library need
- keep the repository clippy-clean after the audit
- document the rule for when future dead-code allowances are acceptable

**Non-Goals:**

- redesigning subsystem architecture beyond what the audit requires
- widening any public API just to justify an existing unused item
- introducing new facade or subsystem APIs for hypothetical future consumers
- rewriting historical compatibility paths that are still actively needed by tests, docs, or current migration work

## Decisions

### Decision: Treat deletion as the default outcome

For each audited dead-code allowance, the default answer should be to remove the unused item entirely unless there is a concrete reason to keep it.

Concrete reasons include:

- current production usage
- current test usage that reflects supported behavior rather than convenience-only scaffolding
- published or documented library contract that the project intends to preserve
- active migration compatibility for an in-flight architectural change

If none of those apply, the item should be removed instead of remaining behind `#[allow(dead_code)]`.

This is the central policy adopted in [ADR-0004](../../adr/ADR-0004-allow-dead-code-policy.md).

Rationale:

- this keeps the codebase aligned with YAGNI
- it prevents lint suppressions from becoming a quiet substitute for design cleanup
- it forces stable boundary claims to be backed by real consumers or explicit documentation

Alternatives considered:

- keep allowances by default until a future cleanup pass
  - rejected because that is exactly how speculative API becomes permanent clutter

### Decision: `#[allow(dead_code)]` is acceptable only for narrow, explicit categories

After the audit, remaining dead-code allowances should only exist for one of these categories:

- compatibility shims with an active consumer or migration path
- documented public boundary items that are intentionally supported even if this repo does not currently exercise them directly
- narrowly scoped test-only helpers where the cost of removal exceeds the value and the helper is still part of an intentional test support surface

Any remaining allowance should be specific to the item rather than applied broadly to a whole module unless the module itself is intentionally a dormant extension contract.

This categorization rule comes directly from [ADR-0004](../../adr/ADR-0004-allow-dead-code-policy.md).

Rationale:

- narrow scope keeps future audits simple
- broad module-level suppression makes it too easy to hide unrelated dead code
- category-based justification makes review standards explicit

Alternatives considered:

- allow broad module-level suppression for convenience
  - rejected because it weakens lint signal too much

### Decision: Architectural uncertainty should become an explicit decision, not a lingering allowance

If an allowed item cannot be clearly classified as “keep” or “remove,” it should be recorded as needing an architectural decision rather than silently retained.

Possible outcomes for such items:

- create or update an ADR if the issue affects stable boundary policy
- keep a temporary, narrowly justified allowance with a note in the active change
- defer removal only when a near-term dependent change has already been identified

This follows the ADR policy that ambiguous dead-code survivors should become explicit decisions rather than silent suppressions.

Rationale:

- unresolved architectural questions should be visible
- this prevents ambiguous APIs from surviving indefinitely behind lint suppression

Alternatives considered:

- keep ambiguous items by default because removal might be inconvenient later
  - rejected because it optimizes for speculative reuse over clarity

### Decision: The audit must preserve a green clippy baseline

The audit should not trade dead-code cleanup for a noisier validation state. Any removals or allowance reductions must leave:

- `cargo clippy --all-targets --all-features -- -D warnings` passing
- `cargo test` passing

Rationale:

- the repo has already regained a clean lint baseline, which is strategically useful
- the audit should improve policy quality without regressing validation discipline

## Risks / Trade-offs

- [Useful future-facing helper removed too aggressively] -> Require a concrete documented justification before keeping speculative surface; reintroducing a small helper later is cheaper than carrying dead code indefinitely.
- [Audit grows into broad refactor work] -> Keep the change focused on classification, removal, and justification of dead-code allowances rather than subsystem redesign.
- [Documented but unused public API gets removed accidentally] -> Check docs, doctests, and subsystem API modules before removing any public-facing item.
- [Compatibility cleanup breaks in-flight work] -> Treat active migration shims as a separate keep category and remove them only when the dependent path is gone.

## Migration Plan

1. Enumerate the recently added `#[allow(dead_code)]` annotations.
2. Classify each annotated item as `keep`, `remove now`, or `needs decision`.
3. Remove unused APIs and unnecessary allowances where classification is clear.
4. Record any true edge cases that require explicit architectural review.
5. Rerun `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test`.

Rollback strategy:

- if the audit removes an item that turns out to be required, restore that item with a narrower, explicitly justified allowance or with a concrete caller that makes the API live again
