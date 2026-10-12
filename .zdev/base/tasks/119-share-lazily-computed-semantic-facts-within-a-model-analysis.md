+++
schema_version = 1
id = "base-119"
key = "demand-fact-session"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-115", "base-116", "base-117", "base-118"]
+++
# Share lazily computed semantic facts within a model analysis

## Outcome

Lint and semantic-query orchestration obtain needed fact families through a small reusable per-model owner, with one computation per requested fact family and explicit dependencies.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Begin with crates/zincite-lint/src/analysis.rs repeated rule-list scheduling and prepared_definitions handling, public fact producers in lib.rs, and crates/zincite-query/src/semantic.rs Option-backed ensure methods. The intended stages are source identity/signatures, expression type/call selection, requested local abstract facts, optional definition/search proofs, then diagnostic policy. Existing producers and domain types remain the implementation; this task changes ownership and scheduling, not semantic precision.

## Boundaries

- Use ordinary Rust functions and a context tied to immutable ModelContext data. Reuse an existing owner if adequate; a new crate, generic registry, user query engine, per-token query graph, persistent cache and Salsa are outside this task.
- Keep one owner per cached result and dependency order explicit. Cache context-sensitive evaluations only with their actual substitutions/scopes included, or leave them uncached; never use source range alone for instantiated/guarded facts.
- Preserve documented public producer entry points with thin compatibility calls where necessary, without maintaining duplicate inference implementations. Keep mutable diagnostics and rule settings out of semantic truth.

## Done when

- [ ] Rules and query stages request named fact families; their prerequisites are computed on demand and reused within that model analysis. The same selected family is not rebuilt for each consumer, and unused deep families are not constructed.
- [ ] Source facts can be shared with optional body facts without recomputing the whole structural layer or mutating facts already observed by another consumer. The design explains how mixed selections and prepared definitions remain coherent.
- [ ] Numeric, guarded, optional and iteration families retain their own compositional algorithms and explicit dependency order. Existing finite definition/search closures remain owned algorithms, not mutual lazy-call cycles.
- [ ] Query result lifetimes and public fact access remain usable; semantic scan/result limits still bound actual work. Each independent root/session releases its cached facts when no longer needed.
- [ ] Orchestration contains request/aggregation logic rather than semantic classification or growing duplicated lists of rule prerequisites. No facts or diagnostics depend on request order.

## Validation

- Compare relevant full public facts and findings from standalone versus mixed selections and repeated requests, including type queries followed by deep lint analysis and two independent model contexts.
- Use focused temporary instrumentation or an existing profiler to verify a shared prerequisite is built once and an unrequested deep family is absent; do not add permanent implementation-count tests solely to mirror scheduling.
- Measure one fresh small-model run and one installed-library mixed-rule run for wall time and peak RSS against the pre-change baseline; investigate material regressions and avoid unmeasured speed claims.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.
