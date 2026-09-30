+++
schema_version = 1
id = "base-027"
key = "stable-expressions"
area = "base"
status = "done"
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

- [x] Fix first/second-pass decisions for binary continuations, nested generator calls, conditions and expanded lists while preserving existing intentional multiline policy.
- [x] Minimise the observed failures into a few regressions and rerun all previously parseable corpus inputs; report any remaining failures by distinct cause.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check idempotence plus token/comment preservation on the reduced examples and full available corpus; a stable result that drops comments or changes grouping does not pass.

## Result

Stabilised ordinary expression wrapping and continuation indentation while preserving operator/comment order and explicit multiline layouts; independently verified.

Validation:

- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace passed (61 tests).
- All 6,417 corpus paths and original hashes reconciled. All 6,345 completed syntax-clean checks passed preservation, reparsing and idempotence; 27 known large-input timeouts and existing syntax/encoding exclusions remain explicit.
- Seven focused synthetic cases, five named public models and original/formatted MiniZinc 2.10.1 model-check-only probes passed.
- Unchanged save driver: 26 case/settings pairs with 50 fresh-process samples each, all 1,300 statuses accepted. Small-case budgets passed; separate native RSS and existing medium/large performance misses remain documented in ignored benchmark reports for tasks 046-048.
