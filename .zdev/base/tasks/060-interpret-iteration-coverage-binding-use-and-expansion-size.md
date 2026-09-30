+++
schema_version = 1
id = "base-060"
key = "iteration-facts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-032", "base-034", "base-035", "base-058"]
+++
# Interpret iteration coverage, binding use and expansion size

## Outcome

Consumers can inspect iteration index sets, emptiness, dependencies and candidate counts without performing a lint.

## Context

Extend existing symbolic index sets and resolved references. Index mismatches, vacuity, unused bindings, global patterns and expensive comprehensions need these facts; do not construct a generic dependence engine. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Track multidimensional and enum index-set identity, supported offsets, proven inclusion/disjointness and exact/unknown cardinality; equal cardinalities are not equal sets.
- [ ] Record binding uses in bodies, later generators, filters, annotations and partial domain expressions, plus invariant subexpressions and earliest valid filter dependencies.
- [ ] Expose proven empty/full/partial iteration coverage and checked exact or symbolic candidate-count upper bounds; preserve quantifier versus arithmetic multiplicity.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check same-sized offset arrays, enum domains, empty/unknown sets, a later generator referencing an earlier binding, and one large symbolic Cartesian product.
