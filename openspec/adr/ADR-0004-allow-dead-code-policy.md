# ADR-0004: `#[allow(dead_code)]` Policy

## Status

Accepted

## Date

2026-04-16

## Context

`repostats` uses `cargo clippy --all-targets --all-features -- -D warnings` as a meaningful quality gate. That only remains valuable if dead-code warnings continue to signal real design drift instead of being routinely suppressed.

Recent cleanup work restored a clippy-clean baseline, but part of that effort used targeted `#[allow(dead_code)]` annotations on APIs and helpers that were treated as compatibility surface, documented boundary, or future-facing helper surface. That created an immediate question:

- when is `#[allow(dead_code)]` a legitimate narrow exception?
- when is it just a way of preserving speculative or obsolete code?

The project already prefers:

- minimum-first public surfaces
- strong encapsulation
- small, reviewable changes
- YAGNI over speculative completeness

Those preferences need an explicit rule for dead-code suppression so future cleanup does not drift into “lint-clean by exception” rather than “lint-clean by design”.

## Decision

`#[allow(dead_code)]` should be treated as an exception, not a normal maintenance tool.

The default policy is:

- if an item is unused and has no concrete current justification, remove it
- do not keep unused code only because it might be useful later
- reintroducing a small API later is preferable to carrying speculative dead code indefinitely

`#[allow(dead_code)]` is acceptable only for narrowly scoped items that fit one of these categories:

- active compatibility shims that still serve an in-flight migration path
- documented public boundary items that the project intentionally supports even if this repository does not currently exercise them through production code
- narrowly scoped test-support helpers where the helper surface is itself intentional and reviewable

For retained allowances:

- scope the allowance to the smallest practical item
- do not apply it broadly to a module unless the entire module is intentionally a dormant extension contract
- be able to explain the reason in review in concrete terms

If an unused item cannot be clearly classified as “keep” or “remove,” treat it as an explicit architectural decision point rather than silently retaining it behind `#[allow(dead_code)]`.

## Rationale

- Dead-code warnings are useful pressure against stale, speculative, or abandoned surface area.
- A minimum-first API policy is undermined if unused APIs survive by default behind lint suppression.
- Broad or casual use of `#[allow(dead_code)]` hides design debt and makes later cleanup harder.
- Keeping the allowance narrow preserves lint signal while still allowing a few intentional exceptions.
- Requiring concrete justification keeps the codebase aligned with YAGNI and encapsulation goals.

## Alternatives Considered

### Allow `#[allow(dead_code)]` freely during cleanup as long as clippy stays green

Rejected.

This optimizes for short-term lint success at the cost of long-term boundary quality. It would make clippy less useful as a design signal.

### Preserve unused APIs by default until a future consumer appears

Rejected.

This is speculative design. Most such APIs never become real dependencies, and the cost of carrying them is higher than the cost of reintroducing a small item later.

### Ban `#[allow(dead_code)]` entirely

Rejected.

There are legitimate cases for narrow suppression, especially around compatibility edges, documented boundary items, and a few intentional test helpers. The problem is not the existence of the attribute, but the lack of a policy for when it is justified.

## Consequences

### Positive

- Dead-code warnings remain meaningful.
- Public and internal surfaces are more likely to shrink toward real usage.
- Reviews have a clearer standard for accepting or rejecting dead-code suppression.
- Future lint cleanup can stay disciplined without repeatedly re-arguing the policy.

### Negative

- Some cleanup work becomes slightly more demanding because removal is preferred over suppression.
- A few APIs may need to be reintroduced later if a real caller emerges.
- Reviewers must exercise judgment for boundary items that are intentionally supported but not currently exercised by runtime code.

## Guidance For Future Changes

Before adding `#[allow(dead_code)]`, ask:

1. Is there a concrete current consumer?
2. If not, is this item part of an intentionally supported documented contract?
3. If not, is it an active compatibility shim for work already in progress?
4. If not, should it simply be deleted and reintroduced later if needed?

If the answer does not clearly justify retention, delete the item instead of suppressing the warning.

When a retained allowance is architecturally important, prefer also recording that reasoning in the active OpenSpec change or a more specific ADR.

## Follow-Up

- Use this policy when auditing the recent dead-code allowances added during lint cleanup.
- Prefer removing speculative unused APIs in future changes rather than retaining them behind suppression.
- Revisit retained allowances periodically when migration paths close or boundary decisions become clearer.
