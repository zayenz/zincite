+++
schema_version = 1
id = "base-060"
key = "iteration-facts"
area = "base"
status = "done"
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

- [x] Track multidimensional and enum index-set identity, supported offsets, proven inclusion/disjointness and exact/unknown cardinality; equal cardinalities are not equal sets.
- [x] Record binding uses in bodies, later generators, filters, annotations and partial domain expressions, plus invariant subexpressions and earliest valid filter dependencies.
- [x] Expose proven empty/full/partial iteration coverage and checked exact or symbolic candidate-count upper bounds; preserve quantifier versus arithmetic multiplicity.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check same-sized offset arrays, enum domains, empty/unknown sets, a later generator referencing an earlier binding, and one large symbolic Cartesian product.

## Result

Added independently usable iteration facts for ordered index-set identity and membership relations, lexical binding uses and earliest filter scopes, guarded partiality, coverage, and checked exact or symbolic candidate products. Anonymous written binder slots now contribute their Cartesian multiplicity to optional counts.

Validation:

- Independent whole-task verifier PASS at W521f07aa426e8f08; root official comparison equal with six owned and six foreign files unchanged.
- Public checks cover equal-sized offset arrays, ordered multidimensional and enum identities, closed membership relations and replaceable-default uncertainty.
- Public checks cover later generator sources, assignment and nested bindings, filters, annotations, partial domain obligations and relative earliest scopes.
- Public checks cover empty and unknown iteration spaces, proper-partial coverage, repeated symbolic factors, anonymous products, dependent uniform bounds, decision universes, checked overflow and quantifier versus arithmetic multiplicity.
- Independent cargo fmt --all -- --check, workspace Clippy with all targets and -D warnings, all 133 workspace tests, and zdev check passed.
- Two independently repeated MiniZinc 2.10.1 controls compiled with explicit standard library without solving; source and FlatZinc bodies matched.
