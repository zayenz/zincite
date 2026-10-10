+++
schema_version = 1
id = "base-087"
key = "structured-data-operations"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-086"]
+++
# Inspect and filter structured literal data while preserving keys and records

## Outcome

Queries expose literal fields/elements/keys and can explicitly filter supported data collections without evaluating arbitrary MiniZinc expressions.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Data edits and structured values. The syntax crate already retains RecordLiteral/RecordLiteralField, TupleLiteral, IndexedArrayEntry, ArrayLiteral, MatrixLiteral/MatrixRow and set nodes. The local seating data has enum-keyed guest records with nested interest arrays. Extend the query's structured views and edit path, not the source parser or a second language interpreter.

## Boundaries

- Support literal scalar/member values, sets, records, tuples and nested arrays with retained ranges/spelling; computed values stay unsupported with located explanations.
- Record field labels and strings are distinct from enum references; retained enum keys must not be renumbered by ordinary collection filtering.

## Done when

- [x] Public library and CLI projections expose record fields, collection elements and array keys with sufficient structure to retain nested values and source locations.
- [x] Explicit equality/numeric element filters, including filter_elements(gt(0)), produce complete candidate data through the edit/output path.
- [x] Filtering compacts ordinary list indices only under explicit transformation; keyed arrays preserve surviving identities and incompatible or nonrectangular candidates are rejected.
- [x] Unsupported computed values, invalid comparison types and numeric range failures produce useful diagnostics without guessed values or partial writes.
- [x] The structured result/projection syntax and supported value forms are documented in the query guide.

## Validation

- Use a few synthetic public examples covering a keyed record with nested array/set, ordinary numeric filtering, empty results, retained keys/comments and an unsupported computed expression.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Result

Implemented and independently reviewed recursive literal inspection, fields/elements/keys projections and explicit equality/numeric collection filtering. Surviving keys/comments/source bytes are retained; collection and enclosing shapes are validated through existing atomic edit and file-safety paths.

Validation:

- Independent review PASS at W9ab9483e754e2bee; comparison equal before completion.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all passed, including 14 query library and 6 query CLI tests.
- Focused numeric/comment/directive/batch checks and four documented root-command examples passed on synthetic data; no corpus/private-input acceptance or semantic-validity claim.
