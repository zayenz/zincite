+++
schema_version = 1
id = "base-005"
key = "array-forms"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-004"]
+++
# Add indexed arrays, matrix syntax and dependent indices

## Outcome

Indexed and two-dimensional literals and dependent collection declarations parse and format with their index and row structure intact.

## Context

Read the pinned indexed-array and type-inst productions and current ordinary collection parsing. Add index tuples, explicit indices, matrix row delimiters, set cardinalities and dependent array indices; expose row/index boundaries to later formatting.

## Boundaries

- Use valid stable row layout here; aligned columns and synchronized wrapping belong to matrix-layout.
- Index tuples are syntax, not evaluated values; no index/domain inference.

## Done when

- [ ] Indexed array literals, indexed/unindexed 2D literals and index tuples parse and format without changing indices, row order or cell order.
- [ ] Set-cardinality and index-dependent array type-inst forms parse and format with their dependent binding names/ranges available.
- [ ] Comments and explicit row structure survive, including empty forms where permitted by the pinned grammar.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use an indexed literal/matrix example and a dependent declaration; check row/index structure, source coverage and idempotence. Supplement valid examples with MiniZinc 2.10.1 acceptance.
