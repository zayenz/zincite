+++
schema_version = 1
id = "base-043"
key = "search-direct"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-036"]
+++
# Check search coverage through direct definitions

## Outcome

Thesis search-coverage identifies missing decisions for direct and nested standard search annotations.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement the direct-definition portion of section 4.9. Use dependency closure seeded by variables in solve search annotations; do not reproduce the thesis array/cycle shortcuts.

## Done when

- [x] Resolve standard typed searches, nested seq_search, array views including array1d and simple user annotation aliases; identify whole versus partial array search coverage.
- [x] Add variables only when all their required dependencies are covered; handle direct equalities, parameter definitions and full array definitions, leaving unseeded cycles and partial arrays uncovered.
- [x] Deliver conservative search-coverage advice and explicit unknown-annotation limitations; document that callable-definition propagation arrives in the next task.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check a direct dependency chain, no search annotation, seq_search/array1d, an unseeded mutual cycle, a seeded cycle and partially searched/defined arrays.

## Result

Implemented direct search coverage facts and separate advice with typed searches, nested aliases/views, all-dependency closure and conservative array/cycle/operator boundaries; independently verified W2ab2d08633a735ce.

Validation:

- Independent Cargo fmt, clippy with denied warnings, workspace tests, diff and zdev checks passed.
- Three pinned compiler/core CLI controls matched warnings 0/3/0 and limitations 1/1/2 with unchanged inputs; no solver or full-library claim.
- Whole-task PASS W2ab2d08633a735ce; exact candidate and six foreign paths preserved. Callable-output propagation remains explicitly Limited until base-044.
