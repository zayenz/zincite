+++
schema_version = 1
id = "base-093"
key = "callable-readability"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Refactor callable definitions into readable internal modules

## Outcome

Callable-definition analysis is organized into understandable internal responsibilities while retaining its existing public behavior and safety boundaries.

## Context

Read brief Implementation direction, Semantic analysis and Testing, the current base-083 task, and the latest semantic-corpus checkpoint. callable_definitions.rs currently contains about 29,000 lines: public fact APIs, callable discovery and fixed-point output inference, model-local inspection, standard-body checks, and source safety/dependency traversal share one Producer. Start with resolve_callable_definitions, Producer, direct_safety, dependencies and their consumers in search_coverage, domains and guarded analysis. Base-083 will be parked behind the subsequent callable-planning task. This task makes the existing implementation readable before deciding its future purpose.

## Boundaries

- Preserve fact APIs, declaration/range identity, ordered diagnostics and limitations, Supported/Unknown/Unsupported distinctions, output guarantees, coverage and refusal behavior. Preserve existing lookup reuse and observable performance; do not add semantic support or repair unrelated failures.
- Use ordinary private Rust modules and named functions aligned with actual responsibilities. Moving one oversized impl into several files is insufficient if the controlling path and dependencies remain difficult to understand. Keep tightly coupled invariants together; choose boundaries from the code rather than an arbitrary line-count limit.
- No new crate, generic analysis framework, evaluator, semantic cache or public API redesign. Future semantic policy and architecture decisions belong to the following planning task.
- Begin from a clean integrated checkpoint after active callable edits finish; retain private unfinished candidates as unmerged evidence. Keep independent command/query and formatter work outside this allocation.

## Done when

- [x] Public fact APIs and controlling analysis flow can be understood from their modules and named collaborators; discovery/output inference, source inspection and strict dependency responsibilities are identifiable without reading the entire former file.
- [x] A short module map explains ownership, dependencies and preserved safety invariants, including why inspection alone supplies no output or search-coverage proof.
- [x] Existing public tests pass before and after; matched focused semantic captures retain the same complete facts, ordered diagnostics, limitations and outcomes, with no unexplained material performance regression.

## Validation

- Run the required workspace fmt/clippy/tests before and after the refactor. Reuse existing tests; no new tests are expected for mechanical restructuring.
- Reuse a small retained comparison set covering callable outputs, local source inspection, cycles, errors and Unknown/refusal cases. Include a representative real model and lookup-growth control; compare full results and bounded release cost. Do not start a new full-corpus campaign solely for the refactor.
- Independently review the refactored responsibilities, callers and preserved contracts; record any unexplained result or cost change as unfinished work.

## Result

Separated callable analysis into explicit stages and readable private responsibilities while preserving public behavior and proof boundaries.

Validation:

- Independent review Wbd02766cd1b47f3f PASS; fmt, Clippy, all 340 workspace tests and diff checks passed.
- Twelve frozen cases: all 36 capture pairs and 72 streams match exactly; identities, ordered facts, diagnostics, limitations and input bytes preserved.
- Bounded serial release Workforce and 1000/10000-reference controls show no unexplained material wall-time/RSS regression; no allocation-counter claim.
