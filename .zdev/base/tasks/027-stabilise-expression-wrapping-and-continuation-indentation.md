+++
schema_version = 1
id = "base-027"
key = "stable-expressions"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-023"]
+++
# Stabilise expression wrapping and continuation indentation

## Outcome

Formatter output for ordinary expressions reaches a stable layout in one pass.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Planning diffs in ATSP and carpet-cutting show generator/binary continuations and comments moving between passes; mapping data shows nested conditional and comprehension wrapping changes. Start in zincite-fmt layout.rs and tests/formatting.rs.

## Done when

- [ ] Fix first/second-pass decisions for binary continuations, nested generator calls, conditions and expanded lists while preserving existing intentional multiline policy.
- [ ] Minimise the observed failures into a few regressions and rerun all previously parseable corpus inputs; report any remaining failures by distinct cause.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check idempotence plus token/comment preservation on the reduced examples and full available corpus; a stable result that drops comments or changes grouping does not pass.
