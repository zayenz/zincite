+++
schema_version = 1
id = "base-148"
key = "search-coverage-explicit-search"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Apply search-coverage only to annotated searches

## Outcome

search-coverage reports nothing for a model whose solve item has no search annotation.

## Context

`docs/lint-search.md` says the rule looks for decisions that an explicit search does not name. Today `var 0..10: a; var 0..10: b; constraint a + b >= 8; solve satisfy;` reports both `a` and `b`; `solve minimize a;` does the same. Start in `crates/zincite-lint/src/search.rs` and `search_coverage.rs`.

## Boundaries

- Keep existing behavior for solve items that have a supported search annotation, including aliases and `seq_search`.

## Done when

- [x] Both examples in Context report no search-coverage finding.
- [x] The documented `int_search([morning_staff], ...)` example still reports `evening_staff`.
- [x] Solve items whose annotations contain no search report nothing and record no limitation for the missing search. An unknown annotation keeps its current limitation behavior.

## Validation

- Add the quiet unannotated case to the existing search tests.
- Run the brief's Rust validation commands and `zdev check base --format json`.

## Result

search-coverage reports nothing unless the solve item reaches a standard search annotation directly or through aliases or seq_search; unknown annotations keep their limitation.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace, zdev check: all pass
- Release CLI: unannotated satisfy/minimize models silent; documented int_search example still reports evening_staff; warm_start/constraint_name-only solves silent
- Independent Codex verifier: pass on snapshot W0e7fbf3a7b8bb3f5
