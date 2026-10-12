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

The initial read-only adapter lowers existing pipelines to Zirql and retains Zincite's native projections, semantic facts, rendering and edit route. Its source transport is c789d21cd41350eacd82037e3aa9c518a429dc2a, with Zirql pinned to 8a2c11a602affe41bcc87b70a8aaad5b33aa0d2c. Zirql is now public; the pinned Git revision remains the source dependency. Its yanked 0.0.0-prerelease only reserves the registry name and is not a usable dependency release. This is the completed adapter increment owned by Zirql base-002; programs, bindings and structured report comprehensions remain pending, and this task remains open.

Zincite currently accepts one fixed pipeline and rejects semicolon statements and bindings. Read crates/zincite-query/src/language.rs, lib.rs, semantic.rs and cli.rs, docs/reference.md and Zirium docs/query-language.md as a design reference. Read the shared comprehension contract from literal-mapping-contract; report construction must use its MiniZinc-style generators and bounded operations. This user-approved follow-up extends the initial single-pipeline boundary; update the brief and query reference with the implemented program contract.

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
