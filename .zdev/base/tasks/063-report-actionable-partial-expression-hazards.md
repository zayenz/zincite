+++
schema_version = 1
id = "base-063"
key = "lint-partial-expression"
area = "base"
status = "done"
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

- [x] Deliver partial-expression in family suspicious for supported zero-divisor, index, empty-aggregate and deopt obligations; distinguish proved failure from a demonstrated possible undefined value.
- [x] Explain the relevant condition and Boolean context, respect guarding/assertions and avoid warnings based solely on unknown facts.
- [x] Avoid duplicate findings with index-set-mismatch for the same access when both are selected; retain useful findings when either rule runs alone.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check one unguarded hazard per supported family, a protected conditional, intentional reification, unknown data and joint selection with the indexing rule.

## Result

Added the opt-in suspicious partial-expression lint for scoped zero-divisor, index, empty-aggregate and deopt requirements. Advice distinguishes failed preconditions from incompatible index candidates and explains guards, default capture and Boolean context; joint selection deduplicates emitted indexing findings.

Validation:

- Independent whole-task verifier PASS at W0fe0ea1a541bf345; coordinator official work-context comparison equal.
- Independent cargo fmt --all -- --check, cargo clippy --offline --workspace --all-targets -- -D warnings and cargo test --offline --workspace passed; all 145 tests passed.
- Focused public checks cover four hazard families, exact ranges/source preservation, protected conditionals/assertions/defaults, relational context, unknown data, empty-body/source boundaries, standalone and joint selection, exact suppression and CLI status precedence.
- Four installed-stdlib controls reproduced expected statuses 1/0/1/1 and diagnostic streams, including typed empty min/max; untyped empty arguments and user-operation ambiguity remain explicit limits.
- MiniZinc 2.10.1 compile-only controls accepted reification and guard/default models and retained the expected aborting assertion error; no solver run or totality, feasibility or equivalence claim.
