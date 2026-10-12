+++
schema_version = 1
id = "base-110"
key = "enum-projection-search-coverage"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = []
+++
# Recognize complete enum2int search projections

## Outcome

A trusted complete enum-to-integer search projection covers the source decision array.

## Context

Read brief.md Adopted Atlas feedback and existing search contracts. Start at crates/zincite-lint/src/search.rs, search_coverage.rs and tests/search.rs. Reported int_search([enum2int(selected[p]) | p in Position], first_fail, indomain_min, complete) warns about selected despite full index traversal. Construct complete small root/include models rather than relying on the unavailable original Atlas models.

## Boundaries

- Trust declaration identity and established standard-call facts, not the name alone. No optional, filtered, partial or unsupported projection is assumed to cover the array.
- No broader projection framework, callable interpretation expansion or corpus sweep.

## Done when

- [ ] The supported complete projection has search-coverage outcome Completed, no relevant limitation and no false search-coverage warning.
- [ ] Shadowed/user overloads and partial or filtered projections retain conservative coverage; unsupported required facts produce a located limitation and selected-rule outcome Limited.
- [ ] Independent root conclusions stay distinct, including a root with no search and roots with different search annotations.

## Validation

- Add one complete projected-search regression with focused shadowing/partial counterexamples and root checks using existing fixtures.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
