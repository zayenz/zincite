+++
schema_version = 1
id = "base-043"
key = "search-direct"
area = "base"
status = "open"
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

- [ ] Resolve standard typed searches, nested seq_search, array views including array1d and simple user annotation aliases; identify whole versus partial array search coverage.
- [ ] Add variables only when all their required dependencies are covered; handle direct equalities, parameter definitions and full array definitions, leaving unseeded cycles and partial arrays uncovered.
- [ ] Deliver conservative search-coverage advice and explicit unknown-annotation limitations; document that callable-definition propagation arrives in the next task.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check a direct dependency chain, no search annotation, seq_search/array1d, an unseeded mutual cycle, a seeded cycle and partially searched/defined arrays.
