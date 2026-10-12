+++
schema_version = 1
id = "base-112"
key = "root-aware-compact-lint-report"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Show lint roots and summarize analysis limitations

## Outcome

Broad lint output identifies roots and presents compact limitations with opt-in located detail.

## Context

Read brief.md Adopted Atlas feedback and analysis outcome contract. Begin at crates/zincite-lint/src/analysis.rs write_analysis, model.rs, cli.rs and tests/cli.rs. Shared include diagnostics currently lack the analysis root, and every limitation is printed separately. Adopt simple root labeling, with a documented --show-limitations option for detailed locations.

## Boundaries

- Retain located library facts, errors, warnings and per-rule Limited outcomes. Presentation must not imply completed analysis.
- Do not build a cross-root deduplication/grouping system; keep differing root conclusions distinct.

## Done when

- [ ] Include diagnostics identify their root and source location.
- [ ] Default stderr summarizes limitations by root with counts and tells users how to request detail; --show-limitations prints located limitations.
- [ ] Warning/error exit behavior is unchanged and independent roots with different conclusions remain distinguishable.

## Validation

- Use a small two-root shared-include CLI fixture and default/detail checks; no new semantic analysis or corpus sweep.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
