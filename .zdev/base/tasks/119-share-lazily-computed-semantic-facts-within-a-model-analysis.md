+++
schema_version = 1
id = "base-119"
key = "demand-fact-session"
area = "base"
status = "done"
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

- [x] Rules and query stages request named fact families; their prerequisites are computed on demand and reused within that model analysis. The same selected family is not rebuilt for each consumer, and unused deep families are not constructed.
- [x] Source facts can be shared with optional body facts without recomputing the whole structural layer or mutating facts already observed by another consumer. The design explains how mixed selections and prepared definitions remain coherent.
- [x] Numeric, guarded, optional and iteration families retain their own compositional algorithms and explicit dependency order. Existing finite definition/search closures remain owned algorithms, not mutual lazy-call cycles.
- [x] Query result lifetimes and public fact access remain usable; semantic scan/result limits still bound actual work. Each independent root/session releases its cached facts when no longer needed.
- [x] Orchestration contains request/aggregation logic rather than semantic classification or growing duplicated lists of rule prerequisites. No facts or diagnostics depend on request order.

## Validation

- Compare relevant full public facts and findings from standalone versus mixed selections and repeated requests, including type queries followed by deep lint analysis and two independent model contexts.
- Use focused temporary instrumentation or an existing profiler to verify a shared prerequisite is built once and an unrequested deep family is absent; do not add permanent implementation-count tests solely to mirror scheduling.
- Measure one fresh small-model run and one installed-library mixed-rule run for wall time and peak RSS against the pre-change baseline; investigate material regressions and avoid unmeasured speed claims.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.

## Result

Added root-local lazy ModelFacts shared by lint and semantic queries, with explicit dependencies, immutable source/proof observations and thin public compatibility entry points.

Validation:

- Independent review passed all five done conditions, public lifetime/limit contracts and ownership; cargo fmt check, Clippy workspace/all-targets with denied warnings, 399 workspace tests and git diff check pass.
- Full temporary original-algorithm fact and ordered-policy parity, request-order/two-context/shared-query controls and producer instrumentation pass; all temporary source instrumentation/tests removed.
- Retained six release samples: small median 0.1188 s vs 0.1071 s and peak RSS 36.4 vs 35.0 MB; mixed 0.3843 s vs 0.2466 s and 52.4 vs 51.9 MB. Material mixed wall variation investigated with three alternating pairs: median wall baseline/candidate 0.2543/0.2035 s, user CPU 0.22/0.17 s, identical outputs; no general speed claim.
- Review snapshot We1df7121f1a53ad8 compares equal; parity, instrumentation, original measurement and diagnostic evidence retained under /tmp/zincite119-*.
