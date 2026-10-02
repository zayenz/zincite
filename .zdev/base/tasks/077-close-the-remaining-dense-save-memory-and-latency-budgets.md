+++
schema_version = 1
id = "base-077"
key = "dense-save-budget-follow-up"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Close the remaining dense-save memory and latency budgets

## Outcome

The representative dense save up to 1 MiB meets the unchanged 100 ms and 64 MiB budgets through bounded measured local improvements, preserving source ownership and formatting behavior.

## Context

Read brief Performance and format-on-save, background/performance.md and scripts/syntax-memory-performance.md. Base-047 reduced dense parse retained allocation to 66132733 bytes, but parse construction still peaks at 81409013 bytes and formatting adds 29360128 bytes while the CST is live. Final paired dense save p95 remains 112.36 ms and native RSS approximately 91 MiB. The next concrete seams are large collection child-buffer construction/finishing and the final-layout rendered token buffer. Token trimming alone was measured and rejected because it adds copying with no useful dense peak-RSS reduction. Use allocation evidence to choose the next small change at these seams, rather than prescribing a replacement representation. Recheck actual fresh-save latency, first-use observations and independent-file release; keep base-048 acceptance blocked while budgets miss.

## Boundaries

- Keep the existing parser/CST architecture, owned source, usize ranges, token indices, node distinctions, borrowed traversal, recovery and protected comments/literals. No unsafe packing, new crate, daemon, persistent cache, arena or open-ended representation rewrite.
- Retain only a demonstrated bounded local improvement. Do not weaken budgets, omit dense or public large-table shapes, add timing/layout-size tests or run a new full semantic corpus campaign.

## Done when

- [x] Use allocation/native RSS evidence to select and validate a bounded local change to the measured construction or rendered-token seams, with no unexplained regression on smaller real/synthetic save cases.
- [x] Meet 100 ms p95 and 64 MiB native child RSS on the representative 966669-byte dense save with both default and nested EditorConfig settings; retain concrete misses as acceptance blockers if the bounded change cannot meet them.
- [x] Preserve exact measured outputs/statuses and supported corpus behavior, borrowed ParsedFile reuse and independent-file allocation release; document native high-water retention limitations separately from application live allocations.

## Validation

- Run the area Cargo checks and reuse existing public syntax/format/lint behavior tests; add a regression only if an actual behavior defect is exposed.
- Retain fresh release baseline/after binaries and exact inputs; use at least 50 fresh save samples per small case/settings variant, separate first-use and per-child Darwin RSS bytes, plus at least five matched interleaved public-table comparisons with complete output/status evidence.
- Use the existing phase/allocation probe and bounded distinct-file native checks; preserve token/leaf coverage, ranges, recovery, protected bytes and retained ParsedFile usability after formatting.
- Run a fresh all-6417 syntax/format corpus branch without --rules, preserving rejection/timeout rows, plus a finite adequate public-table supplemental replay and representative default/thesis/all shared-include replay. Report unchanged budgets and any remaining blockers explicitly.

## Result

Bounded scanner, parser capacity and formatter changes meet dense-save budgets in both settings while preserving source and output behavior; native batch high-water growth remains documented.

Validation:

- Independent verifier PASS at Wa4e49b323b5e4976; final snapshot comparisons equal, required Cargo checks and all 189 tests pass.
- Fresh 26 variants x 50 saves, 400 matched controls and five public-table pairs preserve outputs/statuses; dense p95 90.513/91.347 ms and native RSS 63.9375/63.984375 MiB meet unchanged budgets.
- All 6417 syntax/format inputs accounted for with 6373 completions and 44 retained gaps; supplemental table/shared-include replays, exact allocation drops and foreign/input preservation checked.
