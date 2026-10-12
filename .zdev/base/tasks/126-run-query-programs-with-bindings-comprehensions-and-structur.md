+++
schema_version = 1
id = "base-126"
key = "read-only-query-programs"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-119", "base-125"]
+++
# Run query programs with bindings, comprehensions and structured reports

## Outcome

One query program produces several related results and structured feature reports while sharing its input and demanded semantic facts.

## Context

Shared execution is owned by the Zirql base task bundle: base-002 supplies the initial native pipeline adapter, base-004 supplies programs/bindings, and base-005/base-007 supply positional/keyed report comprehensions. Check the live integration and its pinned Zirql revision before implementation. The prior standalone evaluator plan is superseded; retain Zincite frontend, native result and acceptance obligations, and do not complete this task merely because a core feature exists.

Zirql base-004 now supplies the verified consumer program checkpoint: both CLI forms and Program::parse/evaluate/evaluate_with_model/evaluate_with_facts accept read-only source bindings, ordered statements, terminal emitters and native count/histogram/selection records. Shared execution uses Zirql 4e4baf38e7c8566781f069dccea110383d6c0d24 with exact registry fallback =0.1.0-alpha.3. The canonical source checkpoint is 48c3f6a1c5b211779a5af92dcc519a141cb9985d, from source transport 18095e960adbe1b861e298949c0b0008dbb53f2a. Existing Query/edit APIs, ModelFacts/source/proof ownership, exact-root/BOM and Unknown/negation behavior remain host-owned. Program limits add aggregate native/report admission; CLI output is buffered through evaluation and rendering. Base-126 remains open: the MiniZinc positional/keyed comprehension, generator and key obligations belong to later base-005/base-007 integration. Keep task outcome, boundaries and completion conditions unchanged; reconcile only superseded current-behavior context after acceptance.

Read crates/zincite-query/src/program.rs, language.rs, lib.rs, pipeline.rs, semantic.rs and cli.rs, docs/reference.md and Zirium docs/query-language.md as a design reference. Read the shared comprehension contract from literal-mapping-contract before the remaining integrations; report construction must use its MiniZinc-style generators and bounded operations. The implemented read-only program checkpoint does not complete those constructor obligations.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Keep existing single-pipeline behavior and library entry points compatible. No edit statements, arbitrary callbacks, recursive programs, plugin framework or MLIR machinery.
- Use the existing semantic owner from base-119; do not introduce a second inference implementation.
- Use the same bounded comprehension semantics as the data-construction consumer; do not introduce a parallel evaluator for reports.

## Done when

- [ ] Both CLI forms and a reusable library API accept program files, ordered read-only statements and immutable earlier-result bindings.
- [ ] Named count, histogram and selection results can form one structured JSON report; duplicate keys, undefined/forward bindings and incompatible stage types receive located errors.
- [ ] Report comprehensions construct collections from named query results using the shared contract, with explicit generator order, filtering, keys and Unknown/resource behavior.
- [ ] Programs reuse parsed input and demanded facts, have explicit aggregate work/result/nesting limits, and buffer output so a later failure emits no partial report.
- [ ] Existing pipelines and semantic Unknown/negation/limit behavior remain unchanged; documented examples define statement emission and structured result types.

## Validation

- Reuse query test helpers; add one multi-feature report control using a comprehension and focused later-error, binding and aggregate-limit refusals. Demonstrate shared fact demand with temporary instrumentation rather than permanent implementation-count tests.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace when Rust changes; always run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.
