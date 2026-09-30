+++
schema_version = 1
id = "base-061"
key = "lint-index-mismatch"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-060"]
+++
# Report index-set mismatches and off-by-one accesses

## Outcome

index-set-mismatch explains accesses whose surrounding iteration does not match the indexed array.

## Context

Consume iteration identity and guarded access obligations. This is a correctness/suspicious check, separate from the thesis preference for one-based numeric arrays. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Deliver index-set-mismatch in family correctness for proved incompatible accesses and ranges, including neighbour offsets and multiple dimensions.
- [ ] Show the access and relevant index-set declaration; account for guards, non-one-based and enum indices and unknown data.
- [ ] Support library/CLI selection and suppressions; unresolved evidence produces a limitation or no candidate according to the brief, never an unconditional rewrite.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check offset arrays of equal length, a guarded neighbour access, distinct enum/index identities, and an unresolved parameter relation.
