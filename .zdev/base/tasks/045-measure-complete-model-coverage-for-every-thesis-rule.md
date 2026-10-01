+++
schema_version = 1
id = "base-045"
key = "semantic-corpus-close"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-037", "base-038", "base-039", "base-040", "base-042", "base-044", "base-041"]
+++
# Measure complete-model coverage for every thesis rule

## Outcome

The existing runner measures semantic coverage across real model roots and yields bounded follow-ups for remaining gaps.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Use the full corpus runner with --rules thesis and --rules all. Start from actual unresolved bindings, overloads, type facts and annotations in complete valid models; do not broaden the semantic engine without a failing consumer.

## Boundaries

- Implement only the runner/report integration here. Unknown semantic repair families become bounded follow-ups rather than an unbounded engine rewrite.

## Done when

- [x] Extend the existing small corpus runner for --rules thesis/all, complete-root/dependency outcomes and per-rule executed/inapplicable/limited results from the selected-analysis API. Distinguish warnings, execution failures and analysis limitations.
- [x] Run every applicable rule on complete Challenge/local model roots and review representative findings for all fourteen IDs. Reconcile unresolved bindings, overloads, type facts and annotations with concrete causes; do not treat zero warnings as proof of coverage.
- [x] Propose bounded implementation follow-ups for confirmed semantic gaps or false claims, recording them as blockers of corpus-acceptance. Separate unavailable external dependencies from Zincite defects and keep every skipped rule visible.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run the full semantic corpus and hand-check executed, inapplicable and limited outcomes plus one finding per implemented rule. Reuse existing rule fixtures; defect regressions belong with their fixes.

## Result

The semantic-only corpus runner measures both presets over every inventoried input and records public API outcomes, process failures and exact counts. Compiler-accepted reductions confirm two bounded repair families; the source integration is submitted for independent verification before graph changes.

Validation:

- Required Cargo fmt/clippy/workspace tests and git diff --check passed; raw commands and logs are retained in zincite-base045-required.json.
- Both fixed-binary semantic campaigns finished with 6,417 unique rows per preset. Every selected rule partition and finding count reconciles; target/corpus/base045/reconciliation.json independently matches inventories, summaries and all original hashes after the probes and both runs.
- Fourteen representative finding locations and advisory meanings were reviewed; thirteen are public corpus cases and effective-zero-one uses a separately labelled existing public fixture. target/corpus/base045/finding-reviews.json retains source ranges and reviews.
- Pinned MiniZinc 2.10.1 model-check-only accepts the all_different/default-overload/optional-guard controls and rejects the separate self-include/historical-string controls. Current public facts reproduce the exact loader and callable ranking causes. Independent replay evidence is retained under target/corpus/base045/probes.
- scripts/semantic-corpus-checkpoint.md records exact API outcomes, unobserved failures, unsupported boundaries and the base-050 acceptance boundary. Publication of these two drafts remains coordinator-owned after independent source verification.
