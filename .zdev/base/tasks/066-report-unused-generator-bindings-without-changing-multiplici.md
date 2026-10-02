+++
schema_version = 1
id = "base-066"
key = "lint-unused-generator"
area = "base"
status = "done"
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

- [x] Deliver unused-generator-binding in family suspicious for unused named bindings, excluding anonymous and intentional-unused names.
- [x] Differentiate repeated constraints from arithmetic aggregates; a sum's unused index does not make its iterations removable.
- [x] Recognize uses in subsequent domains/filters and retain partiality obligations in diagnostics and fix eligibility metadata.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a repeated constraint, repeated sum term, later-domain reference, an annotation reference and intentional-unused names.

## Result

Added opt-in unused-generator-binding advice over complete-chain resolved uses, distinguishing Boolean repetition from arithmetic multiplicity and retaining evaluation obligations and conservative prospective anonymization metadata.

Validation:

- Independent whole-task verification passed on W5f04b28d69bd46aa; root official comparison equal. Formatting, Clippy, all 157 workspace tests and zdev check pass. Public checks cover repeated constraints and sums, later domains and filters, annotations, intentional names, shadowed identities, partiality, selection and suppression without source edits.
