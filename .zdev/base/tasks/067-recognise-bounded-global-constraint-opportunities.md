+++
schema_version = 1
id = "base-067"
key = "lint-global-patterns"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-060"]
+++
# Recognise bounded global-constraint opportunities

## Outcome

global-constraint-opportunity gives precise advice for a small supported set of complete decompositions.

## Context

Use resolved standard callable identity and iteration coverage. Start with complete pairwise disequalities and repeated per-value occurrence bounds; extend neither into solver-based pattern search nor arbitrary scheduling recognition. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [x] Deliver global-constraint-opportunity in family modelling for a complete pairwise disequality pattern and a supported whole-array occurrence-count/cardinality pattern.
- [x] Document and check each pattern's equality/index/coverage/totality conditions; partial pairs, user overloads, filtered coverage and annotations cannot be silently discarded.
- [x] Explain the candidate global and excluded cases without claiming faster solving; keep initial suggestions diagnostic-only.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check both complete patterns and counterexamples for partial pairs, shifted indices, nonmatching occurrence coverage and an overloaded predicate.

## Result

Added diagnostic-only global-constraint-opportunity advice for complete pairwise disequalities and matching whole-array occurrence bounds, with resolved identities, actual indices, raw totality and conservative exclusions.

Validation:

- Independent whole-task PASS at W51413513a2042acc; formatting, workspace Clippy with warnings denied, all 162 workspace tests and zdev check passed. Installed MiniZinc standard-library replay produced both expected warnings without limitations or source changes; root post-verification snapshot comparison equal.
