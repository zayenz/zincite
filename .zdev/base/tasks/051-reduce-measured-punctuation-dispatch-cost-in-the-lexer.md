+++
schema_version = 1
id = "base-051"
key = "lexer-symbol-dispatch-performance"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Reduce measured punctuation dispatch cost in the lexer

## Outcome

Token-dense formatter inputs spend less CPU matching punctuation while retaining exact token spelling, coverage, ranges and diagnostics.

## Context

Read brief Performance and format-on-save and scripts/README.md. Start in crates/zincite-syntax/src/lexer.rs and reuse scripts/bench-save.py and the developer phase probe. Native sampling found 1042 of 1450 parse-phase samples in lexer::scan, primarily prefix comparison/memcmp; format-only sampling found 933 of 1453 samples re-lexing through apply_layout. The lexer currently tests the complete longest-first SYMBOLS list for each punctuation token, with comma late in that list. This bounded CPU path is outside base-046 fit measurement and base-047 working-set reduction.

## Boundaries

- Limit implementation to measured lexer symbol matching and necessary focused behavior checks; do not add a scanner framework, cache or broad parser/CST rewrite.
- Preserve longest-match behavior, Unicode operators, source spelling, precise ranges, malformed-input recovery and existing public library contracts.

## Done when

- [x] Use the smallest measured change to select symbol candidates without repeatedly comparing unrelated spellings on dense punctuation.
- [x] Demonstrate before/after release p50/p95 and CPU attribution on dense size growth using the existing save driver, retaining failures, RSS and first-use observations.
- [x] Preserve existing syntax/formatter/linter behavior and report remaining save-budget misses for the already planned fit, memory and save acceptance work.

## Validation

- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace.
- Compare at least 50 fresh-process samples per small case with the tracked base-022 baseline, using identical inputs and configuration variants; recheck dense growth, representative real models and separate native child RSS.
- Run existing lossless lexing, source-range, recovery, formatting stability/comment and lint behavior checks; add only a focused regression needed for changed symbol matching.

## Result

Grouped lexer symbol candidates by UTF-8 leading byte and measured substantially lower dense-input save CPU and latency.

Validation:

- Independent whole-task PASS against unchanged Wb705294a3942188f; all 60 symbol mappings and longest-match order retained.
- 26 paired case/settings distributions each use 50 fresh processes, matching input/output hashes and statuses; dense p95 roughly 300-314 to 125-127 ms at 966,669 bytes, with CPU attribution and native RSS retained.
- Cargo fmt, clippy and all 54 workspace tests pass; first-use uncertainty and remaining fit, memory and save-budget misses documented for base-046, base-047 and base-048.
