+++
schema_version = 1
id = "base-069"
key = "lint-input-preconditions"
area = "base"
status = "open"
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

- [ ] Deliver missing-input-precondition in family modelling when a supported operation requires a condition neither declared nor established by a guard/assertion.
- [ ] Recognize declared domains, explicit assertions and safe conditional handling; unresolved operation meaning does not establish a missing contract.
- [ ] Describe the obligation at the callable boundary and deduplicate against partial-expression when both select the same underlying obligation; do not insert assertions automatically.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check mismatched-array assumptions, nonempty aggregation, numeric preconditions, matching declared/guarded/asserted contracts and joint selection with partial-expression.
