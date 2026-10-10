+++
schema_version = 1
id = "base-090"
key = "syntax-inspection"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-085"]
+++
# Query nested expressions and emit structured source inspection reports

## Outcome

Users can inspect selected model expressions, calls and annotations with exact locations and small JSON/count reports.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Initial query language. The existing CST exposes nested expression/call/annotation/solve nodes and retained ranges. Extend the item query consumer directly; mzn-analyse expression/AST/solve-annotation passes are operation references, while Zirium provides navigation and projection patterns.

## Boundaries

- Counts describe source constructs, not flattened constraints or execution frequency; expression fragments are not promised to be standalone models.
- No fixed-point programs, report-building language, Markdown emitter, objective-term expansion or compiler AST import.

## Done when

- [x] Expression selection, children/subtree navigation and input byte-range selection expose original node kinds/ranges/text, including expressions in selected constraints and solve annotations.
- [x] names, call_names, annotation_names, text, unique and tally produce useful native results with documented source-order/duplicate behavior.
- [x] json emits the brief's node inspection fields and corresponding projection/count/histogram shapes with correct escaping and file identity.
- [x] The library and both commands distinguish malformed ranges/stage combinations from successful empty results and preserve evaluation bounds/no-partial-output behavior.

## Validation

- Use a few public nested-constraint/solve queries covering overlapping descendants, explicit unique, range selection, call counts and JSON escaping. Avoid mirroring private traversal helpers.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Result

Implemented and independently reviewed bounded expression/navigation/range queries, native name/text/count/histogram results, explicit uniqueness and JSON inspection with original file identity, ranges and text. Existing assignment-edit safeguards remain intact.

Validation:

- Independent review PASS at W70319f8f7d0727e3; comparison equal before completion.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all passed, 318 tests across 49 groups.
- Focused public checks cover nested constraints/solve annotations, duplicate subtrees/unique, call tallies, BOM ranges, JSON escaping/file identity, empty results, invalid ranges/types and limits; independent special call/annotation-head checks passed.
