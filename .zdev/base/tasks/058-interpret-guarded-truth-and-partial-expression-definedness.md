+++
schema_version = 1
id = "base-058"
key = "guarded-facts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-057"]
+++
# Interpret guarded truth and partial-expression definedness

## Outcome

Rules can distinguish proven, refuted and unknown conditions and definedness in their actual MiniZinc context.

## Context

Reuse the enforced-context walk of base-036 and numeric facts. Read MiniZinc partiality/conditional semantics and the brief; partial-expression, vacuity, input-precondition and fix tasks depend on this interpretation. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Track conditions in if branches, generators/filters and enforced Boolean contexts using MiniZinc relational semantics; implication/disjunction do not create unconditional global facts.
- [ ] Interpret array access, division/modulo, supported empty-aggregate preconditions and assertions as context-specific definedness obligations; retain unsatisfied/unknown obligations and ranges.
- [ ] Expose guarded truth/definedness to consumers without issuing lint warnings or proposing edits; opaque calls and unsupported forms remain explicit.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check guarded versus unguarded indexing/division, false branch partiality, reified context, a parameter assertion and an opaque call; compare representative cases with MiniZinc without solving.
