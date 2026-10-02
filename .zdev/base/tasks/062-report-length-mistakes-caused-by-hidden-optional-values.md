+++
schema_version = 1
id = "base-062"
key = "lint-hidden-optionality"
area = "base"
status = "done"
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

- [x] Deliver hidden-optionality in family suspicious for length/count-intent patterns over option-producing comprehensions, with a concrete explanation of capacity versus presence.
- [x] Keep ordinary optional aggregates and parameter-filtered comprehensions quiet unless the selected operation has a demonstrated mismatch.
- [x] Integrate selection, exact locations and suppressions; avoid suggesting a replacement count unless its semantics are established.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a variable-set length constraint, a decision-filtered length, a parameter filter and valid optional sum/forall uses.

## Result

Added the opt-in suspicious hidden-optionality lint for direct length comparisons of decision-produced optional comprehensions and supported aliases, with conditional capacity/presence explanations, consuming-item suppression and no replacement advice. Refined callable specificity for standard comparisons while retaining user-operation ambiguity.

Validation:

- Independent verifier PASS at W5c944e07aed5f3f5; coordinator official context comparison equal.
- cargo fmt --all -- --check; cargo clippy --offline --workspace --all-targets -- -D warnings; cargo test --offline --workspace: all 140 tests passed independently.
- Installed MiniZinc library CLI controls returned 1/1/0 for decision membership, decision filter and parameter filter; MiniZinc 2.10.1 compile-only controls accepted capacity semantics without a solver run.
- Focused public checks cover source ranges, aliases, quiet aggregates and capacity contexts, item suppression, joint rule selection, local limits and CLI status precedence; unchanged effective-zero-one regression passes.
