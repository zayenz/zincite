+++
schema_version = 1
id = "base-126"
key = "read-only-query-programs"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-119", "base-125"]
+++
# Run query programs with bindings, comprehensions and structured reports

## Outcome

One query program produces several related results and structured feature reports while sharing its input and demanded semantic facts.

## Context

Zirql supplies shared pipeline execution, programs/bindings and positional/keyed report
evaluation through its base-002/base-004/base-005/base-007 work. The complete consumer
implementation retains Zincite frontend, native results, public ModelFacts ownership,
rendering and source/edit safety. Consumer completion rests on its own full library/CLI
requirements and independent canonical verification, not on a core feature alone.

The accepted source checkpoint is 4c9da0ea89ad7e348a3951aa0a68d8afb8ef47d2, from
reviewed transport 2a959d9a17a084141b149dd7f45aa01da3dc2818 plus the canonical
query-guide correction. Both CLI forms and Program library APIs support ordered
read-only statements, immutable source/let bindings, literal-source helpers and bounded
positional/keyed/multidimensional comprehensions. Native count/histogram/selection
reports preserve their existing shapes; constructed reports retain tagged members, sets,
tuples, records, axes and keys. Exact declared domains and explicit enum-member
identity/order come from existing public facts and retained source. Contiguous axes,
rectangular coverage and established empty-axis information are validated before
intermediate values can be consumed. Unsupported or uncertain identity/shape receives a
located refusal. Parsing, evaluation, projection, copying, shape checks and rendering
share finite request allowances; later failures leave CLI stdout empty. Model facts are
reused within the request; original input/model/BOM locations, exact .mzn root guard and
Unknown/negation behavior remain intact.

Shared evaluation pins reviewed Zirql Git revision
9c3ce81dca3d35cde55b2fba77567d73e431ae54. There is no alpha.5 registry fallback because
that published version lacks the host-validation hook. Development uses exact Git pins;
publication and registry packaging wait for an explicit release milestone. The existing
missing sibling registry-version precondition remains release distribution work; this
task does not claim a normalized package archive.

Read crates/zincite-query/src/program.rs, comprehension.rs, language.rs, lib.rs,
pipeline.rs, structured.rs, semantic.rs and cli.rs, docs/reference.md and
docs/query-comprehension-contract.md for the implemented report contracts. Later data
construction must reuse this shared evaluator and resolve the existing attended base-127
grammar/operation/comment/key gate before edits. No data-write approval is supplied by
this report task.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Keep existing single-pipeline behavior and library entry points compatible. No edit statements, arbitrary callbacks, recursive programs, plugin framework or MLIR machinery.
- Use the existing semantic owner from base-119; do not introduce a second inference implementation.
- Use the same bounded comprehension semantics as the data-construction consumer; do not introduce a parallel evaluator for reports.

## Done when

- [x] Both CLI forms and a reusable library API accept program files, ordered read-only statements and immutable earlier-result bindings.
- [x] Named count, histogram and selection results can form one structured JSON report; duplicate keys, undefined/forward bindings and incompatible stage types receive located errors.
- [x] Report comprehensions construct collections from named query results using the shared contract, with explicit generator order, filtering, keys and Unknown/resource behavior.
- [x] Programs reuse parsed input and demanded facts, have explicit aggregate work/result/nesting limits, and buffer output so a later failure emits no partial report.
- [x] Existing pipelines and semantic Unknown/negation/limit behavior remain unchanged; documented examples define statement emission and structured result types.

## Validation

- Reuse query test helpers; add one multi-feature report control using a comprehension and focused later-error, binding and aggregate-limit refusals. Demonstrate shared fact demand with temporary instrumentation rather than permanent implementation-count tests.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace when Rust changes; always run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.

## Result

Implemented read-only query programs with immutable bindings, native feature reports and bounded positional, keyed and multidimensional comprehensions through shared Zirql evaluation. Preserved public facts, nominal identity, exact axes, native output, located limitations and aggregate budgets; later failures emit no partial output. Canonical source is 4c9da0e with exact reviewed Git dependency 9c3ce81; data edits remain attended.

Validation:

- Independent full canonical base-126 source PASS at Wab173b57fa9f5932 with unchanged comparison; fmt, warning-free Clippy, full workspace tests and Rust 1.88 controls pass. All eleven rebuilt CLI pairs retain exact outputs, diagnostics and statuses; temporary pure/native fact-reuse evidence checked.
- All four commands installed from committed source with exact release-byte identity; eleven installed CLI pairs match reviewed controls, including complete/incomplete JSON and empty stdout for errors. Independent record/eligibility PASS at Wb581491c07b788e3 with unchanged comparison; diff and zdev checks pass. Registry packaging remains explicit release work; no publication or data-write approval.
