+++
schema_version = 1
id = "base-122"
key = "salsa-decision"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-121"]
+++
# Evaluate Salsa against the simplified demand-driven analysis

## Outcome

A bounded evidence-based decision explains whether Salsa would improve Zincite now and defines concrete conditions for revisiting it without adding a production dependency.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Read docs/semantic-processing-plan.md and .zdev/base/background/performance.md after the migration. Research current primary sources: Salsa overview (https://salsa-rs.github.io/salsa/overview.html), cycle handling (https://salsa-rs.github.io/salsa/cycles.html), tuning (https://salsa-rs.github.io/salsa/tuning.html), ty inference architecture (https://github.com/astral-sh/ruff/blob/main/crates/ty_python_semantic/src/types/infer.rs), and rustc queries (https://rustc-dev-guide.rust-lang.org/query.html). The approved target is shared demand-driven facts; Salsa is one implementation option, not the semantic scope decision. Use actual post-refactor workload evidence, not ty benchmark ratios.

## Boundaries

- Production Salsa adoption, a persistent service/editor feature, disk caches and a generic fixed-point engine are outside this task. A recommendation to adopt later is a completed decision, not permission to migrate now.
- Keep the investigation to existing fresh CLI, repeated in-process requests, and a representative shared-library case. Do not create a benchmark platform or assume repeated CLI processes retain database state.
- Use a small disposable prototype only if a specific unresolved question about reuse, lifetime, cycle behavior or cost cannot be answered from the implementation and primary documentation. A prototype must stay out of production code and dependency manifests.

## Done when

- [ ] Update the existing semantic plan with a dated decision and direct primary-source links, separating demand, within-session memoization and cross-revision incrementality.
- [ ] Compare ordinary per-model caching with Salsa for the actual fact families, input identity, include/configuration dependencies, context-sensitive keys, diagnostic collection, cycle handling, invalidation and retained memory. State what Salsa would and would not simplify.
- [ ] Measure fresh analysis and repeated requests with the existing small cases; report actual fact reuse, wall time and peak memory and any measurement limits. Do not infer a Salsa speedup without a comparable prototype measurement.
- [ ] If a prototype is warranted, restrict it to one representative source-fact query and one source/configuration mutation, verify affected results change while unaffected results remain reusable, and check cycle policy and memory lifetime. Otherwise record the concrete reason no prototype is needed.
- [ ] Recommend defer or a separately scoped adoption, with observable reconsideration triggers and implementation cost. Either result leaves the production semantic contract and completed simplification intact.

## Validation

- No new product tests are expected. Verify factual claims against current primary documentation and local code; retain the exact commands and small measurement results in the task result or existing performance note.
- Run zdev check base --format json and git diff --check. Run the brief's Rust checks only if repository Rust code changes, which this investigation should not require.
