+++
schema_version = 1
id = "base-100"
key = "semantic-query-model-admission"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Admit normal-library semantic queries with distinct finite scan and result bounds

## Outcome

Small declaration/use/type queries with the normal MiniZinc 2.10.1 library succeed through both query command forms under finite resource protection, without charging all model source tokens as query result entries.

## Context

Read brief Query and data transformation expansion and Testing, scripts/query-acceptance.md, and crates/zincite-query/src/semantic.rs, lib.rs and existing semantic/limit tests. SemanticResult::fact_scan counts all loaded source tokens/nodes against Limits.collection before ensure_bindings constructs facts; this causes the retained ordinary-library failure independently of optional callable-body expansion. Defaults are work 1,000,000 and collection 100,000 at the proposal baseline. Reuse existing Work and Limits where their contracts fit: model admission/work protects fact construction; collection protects produced results and bounded expansion. Define/document what each guard bounds and choose any necessary finite default adjustment from measured normal-library input. This task is independent of support-contract changes and callable refactors.

## Boundaries

- Keep a pre-allocation model/work admission guard and finite result/expansion bounds. Do not delete guards or merely raise a collection number. Count admitted model work consistently rather than repeatedly treating the same source as a fresh result collection; preserve actual query-stage work accounting. No general budget subsystem, incremental semantic database or partial include loader.
- Preserve query order, exact identities/ranges, unknown predicates, located limitations, status 0/1/2, no partial stdout on errors and identical standalone/umbrella semantics. No enum-expression evaluation, wedding completion claim, reduction safety weakening or unrelated formatter/lint changes. External model/library files stay read-only.
- Scope to the retained real navigation failure and finite refusal behavior. A resource cutoff is not a semantic exclusion and cannot become a complete result. Retain the current API or document a narrowly necessary compatibility decision rather than silently adding a new broad configuration surface.

## Done when

- [ ] The model scan and result/expansion guards have distinct documented meanings and finite enforcement. Normal library source size no longer exhausts the result collection allowance before a small semantic result is computed.
- [ ] One small declaration/use/type inspection with the installed normal 2.10.1 library passes in both command forms, with identical complete results and recorded library/version/settings. An oversized model admission and oversized query result each fail explicitly, retain located errors and emit no partial stdout.
- [ ] Existing low-budget/unknown-predicate and reduction refusal behavior remains intact. A representative command's fresh-process latency and peak RSS are recorded without claiming a broad performance or corpus result. Base-092 remains responsible for its separate final acceptance checks.

## Validation

- Reuse current semantic and budget public tests; add only focused regressions for the demonstrated model/result conflation and finite rejection. Run the small real-library queries in both command forms and check matching output/status. Exercise an explicit small budget for failure rather than building a huge fixture. Keep compiler/solver execution out of ordinary query validation.
- Record the representative release command, library/version/input context, latency/RSS and terminal status; no timing assertions or broad benchmark campaign. Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace and git diff --check.
