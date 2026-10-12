+++
schema_version = 1
id = "base-120"
key = "demand-callable-summaries"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-119"]
+++
# Limit selected-body interpretation to requested retained proofs

## Outcome

Optional body analysis follows the specific retained proof requested by its consumer instead of eagerly inspecting all selected standard implementations.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Start with callables.rs inspect_selected_bodies traversal and callable_definitions.rs discovery -> infer_outputs -> infer_boundaries -> model_roots, plus interpretation and source adapters. Task source-facts-consumers removes consumers that never needed proofs, and demand-fact-session owns shared prerequisites. Here narrow remaining work to the requested bounds/definedness/definition/search operation and its actual selected declaration, concrete formal types, lexical context and reachable summary dependencies.

## Boundaries

- Use the retained summary catalogue established by the contract task. Keep positive proof families; an unknown fallback is allowed for excluded computation but cannot silently replace a promised useful proof.
- Preserve local staged fixed points over a finite discovered set, duplicate elimination, recursion/changing-instantiation barriers and complete-owner/sibling checks. No general interprocedural evaluator or relation inversion.
- Coarse per-declaration or proof-family requests are acceptable when they give the same guarantees with less machinery than per-expression queries. Do not build a fine-grained database to satisfy a naming preference.

## Done when

- [x] A consumer requesting a selected-call property names the property and the selected call context. Unrelated selected library bodies are not expanded solely because some deep rule is enabled.
- [x] Summary dependencies reuse the source layer and existing summaries within the model session. Different concrete substitutions or relevant lexical/guard contexts never share an invalid cached result.
- [x] Retained numeric/definedness and output/search guarantees have identifiable owners and explicit prerequisites; unsupported or recursive requests terminate conservatively.
- [x] Known relevant source/argument errors and sibling failures still veto dependent publication. Opacity does not hide errors already discovered by required source inspection, but there is no promise to discover arbitrary errors in an uninspected excluded library body.
- [x] Public body-aware producer compatibility is documented and implemented through the new machinery. Any explicit bulk compatibility entry point may request all its documented facts, but normal lint/query paths no longer invoke bulk expansion incidentally.
- [x] All retained positive summary controls agree with the prior task baseline; intended reductions are limited to admission-only inspection and documented opacity.

## Validation

- Use one mixed model with a retained transparent-wrapper/definition proof and an unrelated opaque library relation to demonstrate demand separation and retained outputs.
- Reuse distinct-instantiation, recursive-call, sibling-failure and whole-array proof controls. Compare relevant facts and diagnostics across request orders and standalone/mixed rule sets.
- Use temporary tracing or profiling to confirm the unrelated body is not expanded. Reuse the small performance cases from the fact-session task and check peak RSS as well as time.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.

## Result

Selected-call proof consumers request retained body properties and reuse concrete typing within the model session; unrelated library bodies remain opaque. Retained ordinary forall whole-array proofs and explicit bulk compatibility are preserved.

Validation:

- Independent whole-task PASS at Wdad91b7b84d3bfb1 with equal snapshot comparison; all six completion conditions checked.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; git diff --check and git diff --cached --check pass. Search 75/75; public whole-array control fails before and passes after correction.
- 932-row retained parity, mixed demand trace, contextual and sibling veto controls checked; three paired samples per retained small case show lower observed CPU/RSS, without a general speedup claim.
