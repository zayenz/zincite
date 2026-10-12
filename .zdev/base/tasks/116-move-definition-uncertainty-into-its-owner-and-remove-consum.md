+++
schema_version = 1
id = "base-116"
key = "opaque-definition-facts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-115"]
+++
# Move definition uncertainty into its owner and remove consumer exceptions

## Outcome

Definition and domain facts classify unproved initializers directly, and constant/unbounded rules consume those facts without diagnostic-string or source-shape repair logic.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Start at crates/zincite-lint/src/analysis.rs definition.safety rewriting, unbounded_variable.rs inspected_initializer handling, definitions.rs uninspected_union/inspect_union_domain, value_safety.rs, and callable_definitions.rs inspected_locals. The current bridges turn particular Unsupported messages into Unknown only after whole-owner inspection. Read tests/definitions.rs, tests/unbounded.rs, tests/domains.rs and tests/fixes.rs. Base-109's explicit per-element range support is retained.

## Boundaries

- Keep enforcement, coverage, value safety and dependency cycles separate. A local initializer, partial array selection or conditional equality cannot become a complete definition through opacity.
- Retain known relevant sibling failures and explicit error observations; removing an inspection requirement is not evidence that an owning let or call is safe.
- Use existing domain result types where sufficient and small typed distinctions only where a real consumer needs them. Public fact changes must migrate all in-repository callers together; no parallel certainty framework.

## Done when

- [ ] Supported-but-unproved local integer selections, array comprehensions and union-derived domains receive conservative facts from their semantic owner without a second body interpreter certifying admission.
- [ ] analysis.rs no longer recognizes the control/iteration-safety message or reconstructs initializer shapes to change semantic facts; unbounded_variable.rs no longer recognizes the index-membership message; domain admission no longer depends on the UNINSPECTED_UNION_COMPREHENSION diagnostic text.
- [ ] Constant-variable advice and semantic fixes still require supported safety and complete enforced definitions. Unknown initializer facts withhold stronger conclusions.
- [ ] Unbounded advice retains positive simple explicit-domain cases and existing direct/per-element bound suppression. A relevant opaque initializer or relation that might supply a bound/definition prevents advice based solely on failure to prove one; target attribution uses conservative references, not arbitrary relation inversion. If affected targets cannot be bounded safely, withhold the dependent rule result at the necessary scope.
- [ ] Whole-owner and inspected-local mechanisms remain only where they establish an actual retained guarantee. Remove fields/helpers made unused, or document their remaining concrete proof consumer without retaining message-based compatibility logic.
- [ ] Update rule/support documentation for intentional scope changes and preserve meaningful source locations and limitation/error distinctions.

## Validation

- Use existing definition/unbounded controls for scalar and array locals, union domains, partial and conditional definitions, anchored/unanchored cycles, explicit scalar/per-element constraints and sibling failures.
- Add only a missing public opaque-initializer/relation veto with a nearby independent positive declaration to check that uncertainty is localized when evidence permits.
- Run constant-variable and fix controls to prove that replacing Unsupported with supported uncertainty does not upgrade fix eligibility.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.
