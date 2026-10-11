+++
schema_version = 1
id = "base-105"
key = "prefer-array-comprehension"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Prefer direct array comprehensions with preserved indices

## Outcome

Default linting recommends replacing proven standard array1d/array2d/array3d wrappers around matching comprehensions with direct comprehensions and offers safe explicit fixes preserving every index axis.

## Context

The user requested plain comprehensions when the default 1..n index set suffices and indexed comprehensions otherwise, chose an explicit lint fix rather than automatic formatting, and requested inclusion in the base/default lint set. MiniZinc 2.10.1 simple comprehensions use 1..n; indexed comprehensions retain supplied keys. Begin with rules.rs, settings.rs, analysis.rs and existing unused_generator.rs fix production; reuse existing binding, callable and domain facts and atomic source edits. Root retained three compiler-accepted illustrative pairs in /tmp/zincite-comprehension-feasibility; these are supplementary evidence, not a semantic proof. The user approved this task with 2D and 3D cases using tuples before the colon. Simple comprehensions are always one-dimensional: tuple indices remain mandatory for 2D/3D even when axes start at 1. Multidimensional compiler pairs and an empty-axis shape-loss counterexample are retained beside the original examples.

## Boundaries

- Candidates are standard array1d/array2d/array3d calls with matching one/two/three independent named membership bindings in the simple-comprehension argument, no where clause, and fixed set domains matching the corresponding index arguments by established binding/expression identity. Preserve Cartesian generator and axis order; support grouped bindings only when the same proof holds.
- Resolve the standard callable and fixed integer/enum sets using existing facts. Leave ambiguous, shadowed, unsupported, filtered, dependent/extra-generator, array-domain and annotated cases unchanged; do not expand callable semantic analysis or resume base-083 repairs/corpus sweeps.
- Preset membership must not determine requires_model: this new default rule requires model facts. When selected without body-aware consumers, use the existing source/signature callable route; mixed selections retain their existing body-aware route. Do not expand selected bodies just to recognize this wrapper.
- Domain identity proof also establishes safe removal of its first-argument evaluation: accept resolved parameter-set/enum references or already-supported pure fixed set expressions, never syntax equality alone for arbitrary calls, asserts or tracing. Retain body text and evaluation; no general body-totality machinery is required.
- Use a plain comprehension only when existing facts prove a contiguous integer set starting at 1, including its empty case; otherwise preserve keys with the generator binding. Do not infer index identity from cardinality alone.
- In 1D, use a plain comprehension only when existing facts prove a contiguous integer set starting at 1, including its empty case; otherwise preserve the generator key. In 2D/3D retain the complete ordered index tuple even for one-based axes, and require every axis to be proven nonempty; empty or possibly empty axes can lose other axes and stay unchanged. Never infer index identity from cardinality alone.

- Preserve body evaluation, comments, source spelling and annotations. Withhold a fix when removed wrapper text contains comments or annotations that cannot be retained exactly. Formatting remains token preserving.

## Done when

- [x] The style rule prefer-array-comprehension is registered, documented and enabled in default/base lint selection; existing explicit selections and suppressions continue to work. Semantic requirements are independent of default membership; default invocation performs the bounded model-backed analysis, and standalone selection uses source/signature facts while mixed selections retain existing consumers.
- [x] Applicable one-based 1D wrappers fix to [body | s in S]; other supported 1D wrappers fix to [s: body | s in S]. Supported 2D/3D Cartesian wrappers fix to [(i,j): body | i in S, j in T] or [(i,j,k): body | i in S, j in T, k in U], preserving dimensionality, index identity/order, values and evaluation.
- [x] Unproved or excluded forms are unchanged and receive no safe fix; repeated explicit fixing produces no further edits.
- [x] Both zincite lint and zincite-lint expose the rule through the existing diagnostic, --diff and --fix paths with fix selection, atomic source validation and comments preserved.

## Validation

- Add a few focused public behavior checks for one-based, shifted and enum sets, safe fixes, default/explicit selection and suppression; include counterexamples for shadowing, unsupported index knowledge, filtering and comments in removed wrapper text. Reuse existing tests rather than a broad matrix. Include an actual wrapper through omitted --rules/default and an excluded effectful first-argument expression; check existing metadata and bounded callable routing using established public evidence. Add focused 2D/3D tuple cases, including axis-order discrimination and at least one enum axis, plus a possibly empty-axis and a dependent-generator exclusion.
- Compile a few complete before/after examples with MiniZinc 2.10.1 and Gecode to FlatZinc without solving; compare index sets/values directly and FlatZinc payloads, excluding only invocation-path comments. Compiler acceptance alone does not prove equivalence.
- Run required Cargo formatting, workspace clippy with denied warnings, workspace tests, and zdev record validation. After independent verification/commit, install and smoke-check local zincite and zincite-fmt as requested.

## Result

Default prefer-array-comprehension style advice offers safe explicit array1d/array2d/array3d fixes: plain proven one-based 1D, keys otherwise, and ordered multidimensional tuples with proven nonempty axes. Existing source/signature facts and atomic source copies preserve identity, order, evaluation and comments; excluded or unknown cases withhold fixes. Independent verifier passed unchanged Wefd2d48827d0c48e.

Validation:

- Independent Cargo formatting, workspace clippy with denied warnings, workspace tests, zdev and diff checks passed. Focused public tests cover default/explicit selection, both CLI forms, diff/fix/suppression/exclusion/idempotence and 1D/2D/3D boundaries.
- Five generated MiniZinc 2.10.1/Gecode pairs compile without solving and match FlatZinc payloads excluding invocation comments; direct index/value/axis controls and symbolic count 0/3 controls pass. Public routing confirms source-only versus mixed body-aware paths.
- Configured-library default release smoke approximately 54-57 ms versus former syntax-only 4-9 ms; this is smoke evidence, not a performance bound. Local installation and post-commit smoke are coordinator-owned.
