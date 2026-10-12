+++
schema_version = 1
id = "base-117"
key = "opaque-search-proofs"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-116"]
+++
# Separate possible dependencies from proven definitions in search coverage

## Outcome

Search coverage preserves useful finite proofs while opaque relations prevent unsupported claims that a decision is uncovered or independent.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Start at crates/zincite-lint/src/search.rs, search_coverage.rs, callable_definitions/model_roots.rs, inference.rs and dependencies, and tests/search.rs. Existing CallableDefinitionFacts combine outputs, definitions, unavailable targets and inspected locals. Search currently marks affected targets unknown and uses complete-root gates. Base-110's complete enum2int projections are retained. Apply the task-1 catalogue of retained summaries.

## Boundaries

- Maintain actual searched-decision identity, whole-array coverage, enforcement, optionality, index correspondence, sibling-failure and recursion barriers. Do not infer outputs by argument order or invert arbitrary relations.
- Possible reference/dependency edges may block a negative finding but never establish a functional output. Retain conservative wider uncertainty when target attribution is incomplete.
- No new search family, solver reasoning, recursive execution or arbitrary symbolic dependency analysis.

## Done when

- [x] Opaque relations and unavailable output proofs have explicit conservative effects on potentially affected decisions without requiring comprehensive selected-body inspection.
- [x] An absent output proof alone never becomes Uncovered or PartialArray advice. Unrelated known search coverage remains usable; inability to safely localize uncertainty may still limit the wider result.
- [x] Direct definitions, supported complete forall definitions, transparent wrappers, finite callable summaries, aliases/views and complete enum2int search projections retain their established positive coverage behavior.
- [x] Search and definition publication retain known errors and sibling failures and do not publish outputs from incomplete owners. Recursive or changing-instantiation calls terminate with uncertainty rather than guessed summaries.
- [x] Admission-only inspected-local handling is removed from search where it establishes no retained guarantee; public findings and RuleOutcome classifications match the revised contract.

## Validation

- Reuse search tests for direct/whole-array definitions, partial and filtered traversals, cycles, seq_search, aliases and enum2int projections.
- Use a compact opaque p(x,y) case with x searched: y must not be certified covered or warned as definitely independent merely because p is opaque. Check a separate provably uncovered simple decision still receives scoped advice when localization is safe.
- Compare standalone search and combined constant/unbounded/search selections on the same models; findings and relevant proof facts must agree.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.

## Result

Preserve finite search coverage proofs and use mapped possible references to veto unsupported negative advice; remove admission-only local inspection.

Validation:

- Independent PASS at W120eb28be7a692fa; 398 workspace tests, fmt, clippy and diff checks passed. Installed root/standalone lint parity checks passed; binary hashes match.
