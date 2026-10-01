+++
schema_version = 1
id = "base-047"
key = "syntax-memory-performance"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-022"]
+++
# Reduce the live syntax and formatting working set

## Outcome

Token-dense inputs consume substantially less peak memory while preserving source ownership and useful traversal.

## Context

Read brief Performance and format-on-save and ../background/performance.md, including the Ruff, uv and ty references. SyntaxElement and SyntaxNode currently occupy 48 bytes each on arm64, Token occupies 24, and apply_layout re-lexes formatted output while original syntax remains live. These identify measurement candidates, not a required replacement design. Start with syntax storage, vector capacity/lifetimes, formatter temporary buffers and final layout conversion.

## Boundaries

- Do not replace the parser/CST architecture, add unsafe packing or reduce source fidelity solely to reach a number. Do not add tests asserting a particular struct size or allocation layout.

## Done when

- [x] Use allocation evidence to select a small change to token/CST storage, excess capacity, temporary strings or lifetime overlap; demonstrate reduced peak RSS on dense data and confirm the brief memory budgets for the covered save cases.
- [x] Preserve exact token coverage, source ranges, recovery, protected comments/literals and public library usability; update real formatter/linter consumers if the internal storage representation changes.
- [x] Measure release latency and RSS across the small real/synthetic size families. Keep remaining memory misses as concrete acceptance blockers, with bounded follow-ups rather than an open-ended representation rewrite.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Compare before/after child RSS and retained/allocation evidence with consistent native units, run existing syntax/format/lint behavior tests and corpus preservation checks, and verify that independent-file processing releases per-file work.

## Result

Completed and validated the bounded syntax-memory optimization checkpoint; dense parse retained allocation fell 140464461 to 66132733 bytes, native RSS fell about 143 to 91 MiB, and remaining dense save budgets require a direct follow-up before base-048 acceptance.

Validation:

- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace all pass; exact commands/statuses/logs: /Users/zayenz/projects/zincite/target/benchmarks/base047/validation.json.
- All 26 case/settings pairs retain 50 fresh samples per version, identical inputs/output bytes/statuses and normalized invalid diagnostics; full noisy final run, one bounded full repeat, affected-case paired controls and first-use observations retained in /Users/zayenz/projects/zincite/target/benchmarks/base047/.
- Five interleaved public-table before/final pairs emit 78976026 identical bytes; median wall 4131.09 to 3846.63 ms and RSS 3919-3932 to 2805-2808 MiB; evidence: /Users/zayenz/projects/zincite/target/benchmarks/base047/large-table-layout/paired.json.
- Dense and ordinary independent parse/format/lint scopes release live allocations to exact baseline while ParsedFile remains usable after formatting; separate distinct-path 1/3/10/20 native high-water comparison and explicit limitations retained in independent-file-growth.json.
- Fresh syntax/format corpus has all 6417 unchanged inputs, 6349 completed preservation/reparse/idempotence checks, 23 timeouts, 36 syntax assessments, 4 data/model cases and 5 invalid UTF-8 inputs; public table passes a finite 30-second supplemental checker. Evidence: /Users/zayenz/projects/zincite/target/corpus/base047-final/reconciliation.json and large-table-layout-check.json.
- Default/thesis/all shared-include replay preserves 18 before/final reports, findings, rule outcomes, errors and limitations; foreign six-file bytes/modes remain exact and index empty. Evidence: /Users/zayenz/projects/zincite/target/benchmarks/base047/shared-includes/replay.json and ownership.json.
- Dedicated checkpoint and remaining acceptance blocker: /Users/zayenz/projects/zincite/scripts/syntax-memory-performance.md. Complete validation/evidence index: /private/tmp/zincite-base047-validation.json.
