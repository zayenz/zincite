+++
schema_version = 1
id = "base-094"
key = "callable-purpose-plan"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-093"]
+++
# Replan callable analysis responsibilities and semantic support

## Outcome

An advanced, source-backed plan defines what the refactored callable analysis is for, which guarantees it should provide, how each consumer should use it, and the bounded work needed before base-083 resumes.

## Context

Read the completed callable-readability task and its module map, brief Semantic analysis and Testing, base-083, background/thesis-rule-coverage.md, and the current semantic-corpus checkpoint. Inspect actual consumers of CallableDefinitionFacts and source-safety adapters, including search coverage, unbounded-variable advice, domains, guarded analysis and iteration. Repeated corpus repairs have mixed prerequisite inspection with inference of values, definitions, outputs and coverage. Use the clearer implementation to assess that responsibility split; do not assume that every compiler-valid expression must become provable by a linter. On import, add this planning task as a blocker of base-083 while retaining base-083's existing blockers. Leave base-081, base-084 and the query/data-reduction chain independent of this work.

## Boundaries

- Require the ordinary advanced-task planning step before execution. This task itself performs a deep design investigation and produces a plan; it does not implement another semantic rewrite.
- Respect the brief's required coverage and explicit Unknown/Unsupported behavior. Separate supported symbolic uncertainty, missing required support and unavailable external prerequisites. Any proposal to change product scope or completion criteria must be identified for user approval rather than applied silently.
- Prefer existing direct analysis modules and the smallest contracts needed by concrete consumers. Assess alternatives and tradeoffs before proposing abstractions; do not presume a new framework, compiler or evaluator.
- Keep source inspection, value/extent/totality proofs, callable output guarantees and search prerequisites distinct. An inspected source must not accidentally grant a stronger certificate.
- Do not modify unrelated product work, mark base-083 complete, or launch an exhaustive corpus sweep during this planning task.

## Done when

- [ ] The saved plan identifies each current responsibility and consumer, the guarantees and prerequisites of each fact/API, and checked examples of where the current structure creates duplication, excessive specialization or unclear ownership.
- [ ] The plan compares a small set of realistic approaches and recommends a bounded design with explicit handling of Unknown and unsupported cases, including what should remain outside callable analysis.
- [ ] Concrete follow-up task drafts specify observable outcomes, ownership, dependencies and proportionate validation. The plan states the conditions for resuming base-083 and explicitly flags user decisions or proposed brief changes.
- [ ] Independent advanced-design review challenges the recommendation and its consumer contracts; material objections are resolved or retained as explicit decisions without claiming implementation or final corpus acceptance.

## Validation

- Use source inspection and existing public behavior/corpus evidence to support the plan. Distinguish observed failures from hypotheses; use small read-only or private bounded controls only when an uncertainty requires them.
- No new tests or Rust implementation are expected. Review the plan against every affected consumer and the current brief; run zdev check after any authorized record changes.
- Proposed implementation and task changes remain separate from this task's planning result and follow the normal review/approval workflow.
