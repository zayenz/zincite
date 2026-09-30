+++
schema_version = 1
id = "base-050"
key = "corpus-acceptance"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "low"
blocked_by = ["base-028", "base-045", "base-026", "base-029", "base-048", "base-049"]
+++
# Verify corpus processing and full thesis lint coverage

## Outcome

A reproducible acceptance report demonstrates the user goal across the available published Challenge corpus and local models.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. This is an acceptance task after the implementation slices, not permission to hide remaining failures or claim that all warnings should disappear.

## Done when

- [ ] Run the complete corpus command on the recorded archive revision and local tree; reconcile every file, year, model root and known model/data pairing with a result or explicit nonvalid/unavailable classification.
- [ ] Demonstrate no unresolved Zincite processing failures on valid target inputs, source preservation and formatter stability, and all applicable thesis rules executed on complete resolved models; unavailable published data/dependencies prevent a full-coverage claim.
- [ ] Document reproducible usage, actual supported extensions, rule descriptions and analysis limits in project docs, with local/private content omitted. If acceptance fails, leave this task open and create bounded follow-ups.
- [ ] Exercise public directory inputs against the same roots and verify all follow-ups raised by the integration checkpoints are resolved; never mark acceptance complete while those blockers remain.
- [ ] Repeat the brief release performance checks after the final syntax/layout/semantic changes: every representative save case meets latency and memory budgets, batch growth has no unexplained superlinear or unbounded behavior, and all required performance follow-ups are resolved.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run the full corpus and known no-solve compiler checks plus normal Cargo checks; no new tests are expected solely for the acceptance report.
- Report end-to-end per-case p50/p95 and peak child RSS plus separate batch throughput and first-use observations, with hardware/build/input context and no timing assertions in unit tests.
