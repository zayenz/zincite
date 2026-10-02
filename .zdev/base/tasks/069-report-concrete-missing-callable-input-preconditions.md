+++
schema_version = 1
id = "base-069"
key = "lint-input-preconditions"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-060", "base-063"]
+++
# Report concrete missing callable input preconditions

## Outcome

missing-input-precondition explains a callable's specific unvalidated input assumption.

## Context

Use guarded obligations and index/emptiness facts in user callable bodies. Begin with matching array indices, nonempty aggregates and nonzero/positive numeric inputs required by a supported operation. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [x] Deliver missing-input-precondition in family modelling when a supported operation requires a condition neither declared nor established by a guard/assertion.
- [x] Recognize declared domains, explicit assertions and safe conditional handling; unresolved operation meaning does not establish a missing contract.
- [x] Describe the obligation at the callable boundary and deduplicate against partial-expression when both select the same underlying obligation; do not insert assertions automatically.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check mismatched-array assumptions, nonempty aggregation, numeric preconditions, matching declared/guarded/asserted contracts and joint selection with partial-expression.

## Result

Added opt-in missing-input-precondition advice for concrete callable input assumptions, with declared and scoped protection and exact obligation deduplication against partial-expression.

Validation:

- Independent whole-task review passed with no findings; formatting, workspace Clippy and 170 workspace tests passed. Focused public and CLI checks cover array identities, nonempty and numeric conditions, guards/assertions, suppression and source preservation; installed-library replay passed. Verification snapshot Wf712787aa0530982 remained equal.
