+++
schema_version = 1
id = "base-047"
key = "syntax-memory-performance"
area = "base"
status = "open"
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

- [ ] Use allocation evidence to select a small change to token/CST storage, excess capacity, temporary strings or lifetime overlap; demonstrate reduced peak RSS on dense data and confirm the brief memory budgets for the covered save cases.
- [ ] Preserve exact token coverage, source ranges, recovery, protected comments/literals and public library usability; update real formatter/linter consumers if the internal storage representation changes.
- [ ] Measure release latency and RSS across the small real/synthetic size families. Keep remaining memory misses as concrete acceptance blockers, with bounded follow-ups rather than an open-ended representation rewrite.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Compare before/after child RSS and retained/allocation evidence with consistent native units, run existing syntax/format/lint behavior tests and corpus preservation checks, and verify that independent-file processing releases per-file work.
