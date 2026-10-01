+++
schema_version = 1
id = "base-079"
key = "matrix-width-row-classification-cost"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Avoid repeated row classification while measuring matrix widths

## Outcome

Matrix width calculation avoids the confirmed repeated whole-row scans while preserving source-preserving layout and exposing any remaining full-batch formatter cost.

## Context

Read brief Performance and format-on-save and scripts/batch-performance.md. Three fixed rows with5000/15000/30000 columns take about0.27/2.35/10.8 seconds of native formatter wall and matching child CPU, while allocations grow roughly with columns. Public2019/groupsplitter/u7g2pref1.dzn (885733 bytes) takes9.478/9.455 seconds at129 MiB. A selected two-second extended interval has1389 stacks in matrix_layout width closure/iterator, reached via prefix_exceeds_width; its40-second cutoff and prior failed attachment are retained. width_at scans child rows for each column and matrix_row_has_index rescans row tokens. This is a confirmed local cause, not proof of the sole cause of full formatter cutoffs. Start at crates/zincite-fmt/src/lib.rs matrix_layout/width_at/matrix_row_has_index. Retained binaries and exact controls are under target/benchmarks/base049/. This seam is distinct from existing077 child-buffer/rendered-token construction; keep that task and the unchanged save budgets.

## Boundaries

- Compute row classification once within the current matrix layout scope, or an equally small measured local change. Preserve indexed/unindexed rows, column indices, nested values, comments, protected bytes, spelling, indentation, alignment, width decisions and format stability. Keep the existing parser/CST architecture and independently usable formatting APIs.
- No new crate, unsafe packing, arena, layout engine, persistent cache, parallelism or daemon. Do not expand into a broad formatter tuning campaign or infer the current full-batch executing root from stderr tails.
- Use existing source-preservation and layout behavior checks; add no timing/layout-size/reporting-only tests. Retain noisy/failed rows and finite deadline evidence. No formal save-budget relaxation or hard-case exclusion.

## Done when

- [ ] Avoid repeated whole-row classification scans during per-column width calculation and retain byte-identical matched formatting outputs/statuses; public wide and synthetic30000 remain clean parse/reparse, coverage/spelling/structure/protected-byte/idempotence controls.
- [ ] Meet matched native --check targets of at most0.5 seconds on30000-column/three-row data and1 second on the public wide matrix, with fixed-row CPU growth roughly proportional to columns. These are bounded optimization targets; independently measure the unchanged formal save budgets and retain any miss as a final acceptance blocker.
- [ ] Replay both fresh full native formatter checks over every currently discovered root under finite adequate deadlines, reporting RSS and actual completion without assuming the local fix explains all prior300-second cutoffs. Full native replay is required acceptance evidence; preserve any residual cutoff/high-water gap and existing077/050 gating.

## Validation

- Run required workspace fmt/clippy/tests and existing matrix/comment/protected-range/layout public checks; add a focused behavior regression only for an actual exposed behavior defect.
- Retain before/after release binaries and exact input bytes; run at least two fresh matched native --check repeats for5000/15000/30000 columns, public wide, small public table and established27.8MB table with file-backed statuses/diagnostics/output fidelity. Profile separately if the local change fails to remove the confirmed CPU growth.
- Reuse finite adequate existing fidelity/drop checks for public wide and synthetic30000, and verify ParsedFile remains usable after formatting. Compare requested/live/peak allocations and native per-child RSS; do not treat released scopes as a general allocator plateau.
- Use the existing50-fresh-sample save protocol for the up-to1MiB wide shape with default/nested EditorConfig, separately retaining first use and native RSS; preserve unchanged100ms/64MiB acceptance criteria and any concrete miss. Do not infer p95 from the two native checks.
- Run two full native formatter --check repeats over the same three known roots with actual discovery and unchanged original hashes. Run the existing fresh syntax/format corpus without --rules after layout changes, retaining rejects/timeouts and adequate wide/table supplemental checks; no new full semantic campaign is needed for this formatter repair.
