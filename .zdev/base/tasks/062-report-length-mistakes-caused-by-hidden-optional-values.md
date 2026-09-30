+++
schema_version = 1
id = "base-062"
key = "lint-hidden-optionality"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-059"]
+++
# Report length mistakes caused by hidden optional values

## Outcome

hidden-optionality explains when a comprehension length cannot count selected or present elements.

## Context

Consume optional cardinality facts. The handbook variable-set length example motivates a focused suspicious rule rather than warning about every decision filter. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Deliver hidden-optionality in family suspicious for length/count-intent patterns over option-producing comprehensions, with a concrete explanation of capacity versus presence.
- [ ] Keep ordinary optional aggregates and parameter-filtered comprehensions quiet unless the selected operation has a demonstrated mismatch.
- [ ] Integrate selection, exact locations and suppressions; avoid suggesting a replacement count unless its semantics are established.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a variable-set length constraint, a decision-filtered length, a parameter filter and valid optional sum/forall uses.
