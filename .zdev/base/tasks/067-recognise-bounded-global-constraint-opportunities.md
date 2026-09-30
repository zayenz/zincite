+++
schema_version = 1
id = "base-067"
key = "lint-global-patterns"
area = "base"
status = "open"
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

- [ ] Deliver global-constraint-opportunity in family modelling for a complete pairwise disequality pattern and a supported whole-array occurrence-count/cardinality pattern.
- [ ] Document and check each pattern's equality/index/coverage/totality conditions; partial pairs, user overloads, filtered coverage and annotations cannot be silently discarded.
- [ ] Explain the candidate global and excluded cases without claiming faster solving; keep initial suggestions diagnostic-only.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check both complete patterns and counterexamples for partial pairs, shifted indices, nonmatching occurrence coverage and an overloaded predicate.
