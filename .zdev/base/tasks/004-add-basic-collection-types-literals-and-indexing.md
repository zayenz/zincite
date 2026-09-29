+++
schema_version = 1
id = "base-004"
key = "collections"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-003"]
+++
# Add basic collection types, literals and indexing

## Outcome

Array/set declarations and literal/indexing expressions work through the parse/format path.

## Context

Read the pinned type-inst and collection grammar and current expression/type parsers. Add ordinary sets, arrays and lists, optional types and one-dimensional unindexed literals. Reuse scalar-expressions for domain expressions and indexing; array-forms owns indexed/matrix syntax and dependent-index declarations.

## Boundaries

- Comprehensions and generator calls belong to generators; structured types/access belong to structured-values.
- Indexed literals, matrix row syntax, set cardinalities and index-dependent array declarations belong to array-forms.

## Done when

- [ ] Ordinary set, array and list type-inst forms, optional scalar types/absent values, domain expressions, unindexed set/array literals and array indexing parse and format.
- [ ] Compact lists remain compact when appropriate; explicitly multiline comma-separated lists remain expanded with one item per line; comments and literal spelling survive.
- [ ] Typed traversal exposes collection entries, index expressions and collection declarations without introducing a second syntax tree.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a few ordinary collection declarations/literals, an optional value and comments in an expanded list; verify tree shape, exact coverage and idempotence.
- Check representative complete collection models with MiniZinc 2.10.1.
