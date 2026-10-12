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

Shared execution is owned by the Zirql base task bundle: base-002 supplies the native
pipeline adapter, base-004 supplies programs/bindings, and base-005 supplies positional
report comprehensions. Keyed/enum/multidimensional report obligations remain for
base-007 integration. Check the live integration and pinned Zirql revision before
implementation. Retain Zincite frontend, native result and acceptance obligations; a
core feature alone does not complete this task.

The verified positional consumer checkpoint accepts read-only source bindings, source
literal helpers, immutable let bindings and bounded MiniZinc positional comprehensions
through both CLI forms and Program library APIs. Zincite lowers its CST and projects
supported native values; Zirql owns checked scalar expressions, scope and generator
evaluation. Pure reports use tagged scalar/set/tuple/record/positional-array output;
native reports preserve their existing shape. Shared execution uses Zirql
be8bd8b07d5e7e4fb148f8f2e032ffe0f5e226b9 with exact registry fallback =0.1.0-alpha.4.
The canonical source checkpoint is c4a3e568eaca852d2df4e7cae3bd7033218438e6, from source
transport 02ca8d7e881b64666daa8845306f9372cb7bbf73. Existing Query/edit APIs,
ModelFacts/source/proof ownership, original input/BOM locations and Unknown/negation
behavior remain host-owned. Parsing, evaluation, projection, copying and rendering use
aggregate allowances; CLI output remains buffered. Base-126 stays open for the remaining
base-007 report obligations. Keep outcome, boundaries, completion conditions and
validation unchanged.

Read crates/zincite-query/src/program.rs, comprehension.rs, language.rs, lib.rs,
pipeline.rs, semantic.rs and cli.rs, docs/reference.md and Zirium docs/query-language.md
as a design reference. Read the shared comprehension contract from
literal-mapping-contract before the remaining integrations; report construction must use
its MiniZinc-style generators and bounded operations. The accepted positional checkpoint
does not complete the remaining keyed constructor obligations.

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
