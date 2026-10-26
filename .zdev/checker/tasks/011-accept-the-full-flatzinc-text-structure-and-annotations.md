+++
schema_version = 1
id = "checker-011"
key = "text-grammar"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-010"]
+++
# Accept the full FlatZinc text structure and annotations

## Outcome

Standard FlatZinc declarations and compiler metadata prepare through the existing CST with precise validation.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. The current lowerer rejects almost all predicate declarations and many harmless annotations. Read the 2.10.1 FlatZinc grammar and Gecode metadata declarations before extending Source and Lowerer.

Handle the known semantic `computed_domain` annotation under the brief's Checking
contract; its convenience domain is neither a problem restriction nor a source
of singleton defaults. This comes from MiniZinc's pinned `stdlib_ann.mzn`
declaration, unlike feasibility-neutral search/propagation metadata. Preparation
may express the distinction using existing typed unbounded domains if that fits
the executable; preserve logical alias restrictions and validated loading.

## Boundaries

- Preserve source spelling/ranges in shared syntax. Do not execute search annotations, external callbacks or high-level MiniZinc code.

## Done when

- [x] All non-float text forms in the target grammar are admitted with correct literal, parameter-reference, scalar/array alias, index and declaration-order rules; the float seam is covered by float-core. Arbitrary MiniZinc expressions, predicate bodies and invalid reference/type/shape forms still diagnose.
- [x] Known external declarations validate argument types, roles and shapes without requiring upstream formal parameter spelling. Unknown/mismatched declarations and undeclared external calls are diagnosed; annotations on declarations follow the target grammar.
- [x] Standard/Gecode search, propagation, output and introduced-variable annotations are parsed and classified without changing feasibility. Application metadata and source ranges survive; unknown semantic annotations and experimental external/search-state hooks receive located diagnostics.

## Validation

- Exercise compact grammar/admission cases and actual emitted declarations/annotations from both pinned libraries, including renamed formal parameters, nested annotation arrays and malformed signatures.
- Run Rust workspace checks and both zdev checks; confirm formatter/linter/query source-preservation behavior if shared syntax changes.

## Result

Implemented and independently verified non-float FlatZinc text admission, structural predicate declarations and contextual metadata validation. Computed domains no longer create constraints or singleton defaults; aliases and fixed initializers preserve their logical restrictions.

Validation:

- Workspace formatting, Clippy with warnings denied and 549 workspace tests passed, including shared syntax source preservation and formatter/linter/query suites. Focused grammar, signature, metadata, domain and serialized round-trip cases passed.
- Pinned-source compile-only standard and selected Gecode annotation specimens prepared successfully. Parser-free release Wasm build excludes syntax; native/Wasm smoke agreed across 18 cases and eight additional computed-domain/emitted-metadata comparisons.
- Full independent review and one correction/focused recheck passed at unchanged snapshot Wf6b12f228247b1d4. output_only is rejected with locations across scalar/array aliases and parameter contexts; valid output markers and computed_domain passed.
- Both zdev checks, normal pinned-library inventory comparison and diff checks passed. Mathematical predicate scope stays at ten names; literal array access is documented as a checked Gecode-compatible reference extension.
