+++
schema_version = 1
id = "base-085"
key = "item-queries"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-084"]
+++
# Deliver source-preserving item queries through library and both commands

## Outcome

Users can select model items and data assignments by kind/name, filter their ordered selection and emit original source or a count.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Commands and ownership and Initial query language. Start with ParsedFile/SyntaxNode/NodeKind in crates/zincite-syntax/src/lib.rs, existing source locations and inputs APIs, and the umbrella CLI entry points. Inspect Zirium's named local lexer/parser references before adapting small useful portions. Build the reusable zincite-query library and thin zincite-query binary alongside zincite query.

## Boundaries

- Implement the first fixed-stage pipeline; no whole Zirium dependency, arbitrary callbacks, plugin/backend interfaces, permanent mutable document or general MiniZinc evaluation.
- Limit the CLI to one query and one input file/stdin; syntax queries do not load includes or need an installed standard library.

## Done when

- [x] items, filter(kind/name with parenthesized not/and/or), head, count and source emit execute from the reusable library and identically through zincite-query and zincite query.
- [x] The agreed initial/default selection, ordering, identifier identity, selected comments and implicit source emission are documented and observable for .mzn and assignment-only .dzn.
- [x] Query/input errors have useful located diagnostics and status 2 without partial stdout; syntax-only operation remains independent of semantic context.
- [x] Nested predicates and evaluation expansion obey explicit documented bounds, and exceeded limits fail without silent truncation.
- [x] Help and local installation examples advertise only implemented query stages.

## Validation

- Use a few public model/data queries covering retained comments/quoted names, order, empty selections, Boolean precedence and malformed/over-nested queries. Check source/token preservation through the public result, not merely saved input equality.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Result

Added reusable bounded source-preserving item queries through zincite-query and zincite query, with native selections/counts and located errors.

Validation:

- Independent review confirms source/token ranges, quoted names, predicate precedence, comment/directive behavior, BOM/opaque bytes and assignment-only data.
- Both command forms match in targeted public comparisons; malformed and limit errors produce no partial stdout.
- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace pass.
