## 1. Inventory current dead-code allowances

- [ ] 1.1 Enumerate the `#[allow(dead_code)]` annotations added during the recent clippy cleanup pass.
- [ ] 1.2 Group the findings by subsystem (`core`, `notifications`, `plugin`, `queue`, `scanner`) and by kind (`public boundary`, `compatibility shim`, `test helper`, `internal helper`).
- [ ] 1.3 Record whether each allowed item has current production use, test use, documentation use, or no clear consumer.

## 2. Classify each allowed item

- [ ] 2.1 Mark items with no concrete consumer or documented contract as `remove now`.
- [ ] 2.2 Mark items that are still serving an intentional compatibility or documented boundary role as `keep`.
- [ ] 2.3 Mark ambiguous items that affect stable boundary policy as `needs decision`.

## 3. Remove unjustified dead code

- [ ] 3.1 Remove unused APIs and helpers classified as `remove now`.
- [ ] 3.2 Remove corresponding `#[allow(dead_code)]` annotations that are no longer needed.
- [ ] 3.3 Narrow any remaining allowances so they apply only to the specific justified item.

## 4. Handle edge cases explicitly

- [ ] 4.1 For each `needs decision` item, either resolve the decision during the change or record the required follow-up artifact.
- [ ] 4.2 Confirm that any retained dead-code allowance has a clear reviewable justification.

## 5. Re-verify the baseline

- [ ] 5.1 Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] 5.2 Run `cargo test`.
- [ ] 5.3 Summarize which allowances were removed, which remained, and why.
