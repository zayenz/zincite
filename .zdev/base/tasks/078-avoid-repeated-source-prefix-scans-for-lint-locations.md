+++
schema_version = 1
id = "base-078"
key = "source-location-prefix-cost"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Avoid repeated source-prefix scans for lint locations

## Outcome

Native diagnostic and semantic-location workloads avoid the confirmed repeated source-prefix cost while preserving exact diagnostics and current selected-analysis outcomes.

## Context

Read brief Performance and format-on-save, background/performance.md and scripts/batch-performance.md. At the base049 baseline, 10000 warnings take0.999/0.997 seconds and25000 take5.236/5.308 seconds; the20-shared-root all-preset control takes23.077/23.054 seconds. SourceLocation::new walks source[..range.start] for every location. A selected two-second diagnostic interval has1445 top-stack samples there; the semantic interval has991 of1439. These are interval observations, not whole-command percentages. Start at crates/zincite-lint/src/model.rs location construction and its actual analysis callers. Retained baseline executables, exact controls and raw statuses are under target/benchmarks/base049/baseline-binaries/ and the linked reports. Existing075 include-loading and076 callable limitations stay independent; keep050 blocked until its full current acceptance evidence exists.

## Boundaries

- Use a bounded local source-location improvement, with any needed position data owned only by the current source/analysis scope. Preserve public API behavior, source byte ranges, BOM offsets, CRLF/CR handling, Unicode columns, paths, diagnostic order/text, rule outcomes, errors and limitations.
- Do not add cross-root caching, parallelism, a daemon, new crate, semantic support or a location framework. Other sampled functions are observations rather than optimization scope. Do not hide limited/rejected/unobserved work or edit external sources.
- Reuse public behavior checks and the existing probe; add no timing/layout/reporting tests. Preserve all noisy/failed baseline rows and separate instrumented profiles from native comparisons.

## Done when

- [ ] Remove repeated prefix scanning from the measured lint location workload using the smallest concrete consumer change; preserve byte-for-byte native diagnostics/status and the complete current per-rule findings/errors/limitations on matched controls.
- [ ] Meet matched fresh native targets of at most2.5 seconds for25000 warnings,0.6 seconds for10000 warnings and12 seconds for20 shared semantic roots; demonstrate the diagnostic growth no longer follows the measured repeated-prefix pattern. Record child CPU, allocation traffic/live/peak and RSS, investigate any regression and preserve exact scope release.
- [ ] Replay full current native default/thesis/all commands and structured coverage with finite adequate deadlines after the change, without dropping roots; keep incomplete counts Unobserved and final acceptance blocked by remaining syntax/loading/semantic/performance gaps.

## Validation

- Run required workspace fmt/clippy/tests; reuse focused public location/diagnostic behavior checks for BOM, CRLF, Unicode and exact ranges. Add a behavior regression only if a concrete behavior defect is exposed.
- Retain exact before/after release binaries and inputs; run at least two fresh matched repeats for the100/1000/10000/25000 warning family and1/3/10/20 shared roots with installed2.10.1 core and all preset. Keep complete file-backed diagnostics, statuses, child CPU/RSS and native/probe outcome reconciliation.
- Use separate existing allocation/drop attribution to check per-root results/context/position data release; preserve standalone SourceLocation caller behavior and default/thesis/all shared/missing/rejected/BOM controls.
- Run full native default/thesis/all over the same three known roots, re-discover the actual denominator, rehash originals, and report attempted throughput separately from completed semantic coverage. Preserve finite cutoffs and existing075/076/077 blockers; do not claim full semantic acceptance from this performance repair.
