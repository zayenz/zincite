+++
schema_version = 1
id = "base-066"
key = "lint-unused-generator"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-060"]
+++
# Report unused generator bindings without changing multiplicity

## Outcome

unused-generator-binding identifies unused iteration names and explains repetition where it can be established.

## Context

Consume resolved binding-use facts across the entire remaining generator chain, body and annotations. The later fix task may anonymize a binding but must preserve iteration. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Deliver unused-generator-binding in family suspicious for unused named bindings, excluding anonymous and intentional-unused names.
- [ ] Differentiate repeated constraints from arithmetic aggregates; a sum's unused index does not make its iterations removable.
- [ ] Recognize uses in subsequent domains/filters and retain partiality obligations in diagnostics and fix eligibility metadata.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a repeated constraint, repeated sum term, later-domain reference, an annotation reference and intentional-unused names.
