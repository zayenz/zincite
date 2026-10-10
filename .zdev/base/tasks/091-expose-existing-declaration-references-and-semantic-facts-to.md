+++
schema_version = 1
id = "base-091"
key = "semantic-queries"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-090", "base-089"]
+++
# Expose existing declaration references and semantic facts to model queries

## Outcome

Queries can navigate supported model declarations and references and inspect available type/instantiation facts without running lint policy.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), the final semantic-query paragraph, and the existing Semantic analysis contract. Public APIs already export load_model/resolve_bindings/resolve_callables/resolve_instantiations and located unknown/unsupported outcomes. Extend the query consumer using these APIs; base-083 remains responsible for its existing thesis/corpus repairs and is not a blanket prerequisite for querying supported facts.

## Boundaries

- No replacement name resolver/type checker, forced shared semantic crate, SSA traversal or inferred flattening dependencies.
- Compute only facts requested by a query and keep normal syntax queries, formatter save behavior and lint defaults independent of this machinery.

## Done when

- [x] Supplied model/include context enables reference-to-declaration navigation, declaration uses, bounded transitive reference traversal and available type/par-var inspection through native library results and both query commands.
- [x] File identities, lexical shadowing, callable overload resolution and standard/user source distinctions agree with the existing fact APIs.
- [x] Unknown/ambiguous/unsupported predicates and missing dependency errors are explicit; not unknown cannot become a positive match and incomplete reports cannot claim complete analysis.
- [x] Traversal terminates on recursive references within its limits and documents static written-reference semantics, ordering and deduplication.

## Validation

- Use a few public examples for nested shadowing, included declarations, overloaded calls, a recursive reference and unknown facts; compare with the existing public semantic results and supplement complete examples with compiler checks.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Result

Added existing-fact semantic navigation and type/instantiation inspection with exact retained-root matching, actual cross-file source owners, bounded traversal and persistent located uncertainty.

Validation:

- Independent whole-task review passed; snapshot W66d5238012933339 comparison equal true.
- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace and git diff --check passed independently.
- Native identity/source/unknown/demand checks and both commands passed; three complete public controls passed MiniZinc 2.10.1 model-check-only without solving. BOM-bearing include is a separate compiler limitation covered by native preservation checks.
