+++
schema_version = 1
id = "base-099"
key = "simple-consumer-signature-demand"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-097"]
+++
# Separate source call facts from optional body expansion for simple lints

## Outcome

Decision operator, generator and condition analyses can consume supported source signatures/types/calls and instantiations without expanding selected standard bodies for optional proofs, while mixed/deep selections preserve their existing body-aware facts and findings.

## Context

Read the adopted support contract and current callables.rs, instantiations.rs and analysis.rs. resolve_callables builds declarations/signatures and source expression facts, then follows selected standard-library bodies. The three simple decision-use consumers need instantiation facts at written source locations, not callable output/totality proofs. Preserve written user-body inspection and typing needed to establish signatures. Start with this real consumer boundary; do not extract a new general frontend. Both routes should use existing CallableFacts and common implementation rather than parallel type engines. The existing public resolve_callables behavior remains available to deep consumers.

## Boundaries

- Retain common types, actual overload selection/coercion/substitutions, unknown-candidate veto, declaration/file/range ownership, optionality, user-body source facts, diagnostics and suppression. A selected opaque signature gives types and formal/actual identity, not bounds, totality, definitions or search outputs. Known errors remain visible to dependent proof paths.
- Use ordinary functions and existing fact structures. For a mixed selection compute one appropriate body-aware result, not both copies. Preserve deep consumers' current output/definition/search barriers, scope/view/lifetime/index choices and ordered results. No new syntax admission, evaluator, HIR, registry, cache, certainty framework, crate or blanket trust of builtin names/bodyless calls.
- Do not change MiniZinc relational partiality into Python-style short-circuit flow. Do not broaden proof-sensitive fix or constant-variable advice. If focused inspection demonstrates the simple path already meets the boundary, retain it and complete with checked evidence instead of adding an abstraction. No performance improvement is claimed without matched measurements.

## Done when

- [x] The simple-only selection reaches the supported source-call/instantiation facts without selected-library-body expansion for optional proofs; the controlling functions and caller demand are explicit. If no implementation change is necessary, evidence establishes that behavior for the named consumers.
- [x] A typed opaque call, shadowed/competing overload, optional result and decision expressions in condition versus branch retain correct selected declarations, user-source types/instantiations, findings and limitations. Unknown selection cannot fabricate a definite match.
- [x] Mixed simple/search and representative existing deep selections preserve complete body-aware facts, ordered findings/limitations and proof refusals. Existing public resolve_callables behavior remains compatible. The simple path does not publish stronger definition/output/coverage facts or hide dependent errors.

## Validation

- Establish a small before/after set from existing public callable and decision-use tests: typed opaque call, overload/shadowing, optional result and condition/branch distinction. Add only a focused public regression where the changed boundary lacks an existing meaningful witness. Compare source-location facts and rule outputs, not private helper calls.
- Compare full existing body-aware results for one mixed simple/search case and representative deep controls, including a refusal/sibling-failure control. Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace and git diff --check. No corpus sweep or solver run.

## Result

Separated source callable facts from optional selected standard-body expansion; simple decision rules use source facts and mixed/deep selections reuse the full producer.

Validation:

- Independent whole-task verifier PASS at Wf9d173e4ba786e10 with equal snapshot comparison.
- fmt, clippy, workspace tests and diff checks passed; ten full public before/after captures match; eight source/full equivalence cases and mixed success/refusal controls passed.
