+++
schema_version = 1
id = "base-063"
key = "lint-partial-expression"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-059", "base-061"]
+++
# Report actionable partial-expression hazards

## Outcome

partial-expression reports concrete definedness hazards with their enclosing semantic context.

## Context

Consume numeric, guarded and option facts; do not recompute conditions in the rule. Intentional relational partiality remains a reason for precise advice and suppression. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Deliver partial-expression in family suspicious for supported zero-divisor, index, empty-aggregate and deopt obligations; distinguish proved failure from a demonstrated possible undefined value.
- [ ] Explain the relevant condition and Boolean context, respect guarding/assertions and avoid warnings based solely on unknown facts.
- [ ] Avoid duplicate findings with index-set-mismatch for the same access when both are selected; retain useful findings when either rule runs alone.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check one unguarded hazard per supported family, a protected conditional, intentional reification, unknown data and joint selection with the indexing rule.
