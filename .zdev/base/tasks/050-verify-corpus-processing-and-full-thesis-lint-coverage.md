+++
schema_version = 1
id = "base-050"
key = "corpus-acceptance"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "low"
blocked_by = ["base-026", "base-028", "base-029", "base-045", "base-048", "base-049", "base-075", "base-076", "base-078", "base-079", "base-080", "base-081", "base-082", "base-083"]
+++
# Verify corpus processing and full thesis lint coverage

## Outcome

A reproducible acceptance report demonstrates the user goal across the available published Challenge corpus and local models.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. This is an acceptance task after the implementation slices, not permission to hide remaining failures or claim that all warnings should disappear. The adopted bounded policy permits named semantic exclusions at acceptance while retaining their Limited outcomes; it completes no task. Preserve base-083 diagnostic-scaling and confirmed measured-cost obligations and all existing final integration/compiler/source-preservation checks.

## Done when

- [x] Run the complete corpus command on the recorded archive revision and local tree; reconcile every file, year, model root and known model/data pairing with a result or explicit nonvalid/unavailable classification.
- [x] Demonstrate no unresolved Zincite processing failures on valid target inputs, source preservation and formatter stability, and complete per-root/per-rule execution accounting on complete resolved models under docs/semantic-processing-plan.md. Retain exact selected IDs, source/dependency/availability classifications, Completed/Limited/Inapplicable/NotRun partitions and explicit invalid/unavailable/Unobserved rows. Resolve missing/incorrect promised semantic support; accepted documented exclusions stay located and Limited, not Completed. Resource cutoffs, crashes, missing observations and invalid target syntax cannot become exclusions. Unavailable published data/dependencies prevent a full-coverage claim.
- [x] Document reproducible usage, actual supported extensions, rule descriptions and analysis limits in project docs, with local/private content omitted. If acceptance fails, leave this task open and create bounded follow-ups.
- [x] Exercise public directory inputs against the same roots and verify all follow-ups raised by the integration checkpoints are resolved; never mark acceptance complete while those blockers remain.
- [x] Repeat the brief release performance checks after the final syntax/layout/semantic changes: every representative save case meets latency and memory budgets, batch growth has no unexplained superlinear or unbounded behavior, and all required performance follow-ups are resolved.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run the full corpus and known no-solve compiler checks plus normal Cargo checks; no new tests are expected solely for the acceptance report.
- Report end-to-end per-case p50/p95 and peak child RSS plus separate batch throughput and first-use observations, with hardware/build/input context and no timing assertions in unit tests.

## Result

Recorded current corpus fidelity, bounded semantic support and final compiler, directory and release acceptance evidence with explicit coverage qualifications.

Validation:

- Independent whole-task verifier PASS at W79980afa21ef54e5: all five completion conditions supported under the adopted bounded contract.
- All 6417 unchanged inputs accounted: 6401 preservation/reparse/stability successes, 16 previously assessed negatives, zero timeout or Unobserved fidelity rows.
- Three original/formatted no-solve Gecode controls pass; directory controls retain 28 passing comparisons and exact source discovery with explicit editor-lock and VCS qualifications.
- All 1400 fresh saves meet applicable budgets; 40 bounded native runs complete, stream hashes and pins match, and current 381-test fmt/clippy receipts remain applicable to documentation-only changes.
