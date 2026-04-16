## 1. Inventory current dead-code allowances

- [x] 1.1 Enumerate the `#[allow(dead_code)]` annotations added during the recent clippy cleanup pass.
- [x] 1.2 Group the findings by subsystem (`core`, `notifications`, `plugin`, `queue`, `scanner`) and by kind (`public boundary`, `compatibility shim`, `test helper`, `internal helper`).
- [x] 1.3 Record whether each allowed item has current production use, test use, documentation use, or no clear consumer.

## 2. Classify each allowed item

- [x] 2.1 Mark items with no concrete consumer or documented contract as `remove now`.
- [x] 2.2 Mark items that are still serving an intentional compatibility or documented boundary role as `keep`.
- [x] 2.3 Mark ambiguous items that affect stable boundary policy as `needs decision`.
  Remaining scanner/check-out-manager test-support cases were identified as boundary decisions rather than routine dead-code cleanup.

## 3. Remove unjustified dead code

- [x] 3.1 Remove unused APIs and helpers classified as `remove now`.
- [x] 3.2 Remove corresponding `#[allow(dead_code)]` annotations that are no longer needed.
- [x] 3.3 Narrow any remaining allowances so they apply only to the specific justified item.
  The remaining allowances are limited to scanner/check-out-manager test-support helpers and a single styles macro case.

## 4. Handle edge cases explicitly

- [x] 4.1 For each `needs decision` item, either resolve the decision during the change or record the required follow-up artifact.
  Follow-up change created: `restrict-scanner-test-api`.
- [x] 4.2 Confirm that any retained dead-code allowance has a clear reviewable justification.
  Scanner and checkout-manager cases remain only as temporary, explicit exceptions pending the follow-up boundary cleanup.

## 5. Re-verify the baseline

- [x] 5.1 Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] 5.2 Run `cargo test`.
- [x] 5.3 Summarize which allowances were removed, which remained, and why.
  Most allowances across `core`, `notifications`, `plugin`, and `queue` were removed. The remaining scanner/check-out-manager cases were isolated as a separate test-boundary follow-up, and one `core::styles` allowance remains narrowly scoped to the generated enum variants.
