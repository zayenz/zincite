+++
schema_version = 1
id = "base-136"
key = "semantic-lint-presets"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Expose semantic lint groups as built-in presets

## Outcome

Users can select coherent MiniZinc concerns through existing preset selectors in the library, settings and both CLIs.

## Context

The current28-rule catalogue has primary families and default/thesis/all presets, but lacks domain-oriented choices. Start with crates/zincite-lint/src/rules.rs, config selection and docs/linting.md. Implement this exact group mapping: conventions: naming, missing-constraint-label, array-index-start; scope: suspicious-shadowing, unused-declaration, unused-generator-binding, global-variable-in-function; arrays: index-set-mismatch, hidden-optionality, prefer-array-comprehension; domains: unbounded-variable, suspicious-domain, partial-expression, missing-input-precondition; formulation: constant-variable, compact-if, effective-zero-one, element-predicate, global-constraint-opportunity, prefer-enum, vacuous-constraint; flattening: expensive-comprehension, decision-variable-operator, decision-variable-generator, decision-variable-condition, reified-global; search: search-coverage, unmarked-symmetry-breaking. Read the approved usability follow-up in the brief.

## Boundaries

- Reuse existing preset machinery; no second taxonomy engine or new semantic analysis. Keep primary family, severity, advisory wording and fix metadata independent.

## Done when

- [ ] Each named preset expands deterministically through preset:NAME in CLI and configuration; all28 current rules are represented.
- [ ] Existing family selectors, personal presets, exclusions and explicit IDs retain their contracts; built-in/personal name conflicts get a clear error rather than shadowing.
- [ ] Keep thesis membership unchanged and expose group memberships through rule listing/help and user documentation.
- [ ] Correct the docs classification of unused-generator-binding to match its actual suspicious family.

## Validation

- Use one focused catalogue/selection control for exact group mapping, deduplication/exclusion and built-in/personal collision, with a representative configuration and both CLI forms.
- Run the brief's required Rust validation for code changes; use existing focused controls and only the few behavior checks specified here.
- Run zdev check base --format json for changed records.
