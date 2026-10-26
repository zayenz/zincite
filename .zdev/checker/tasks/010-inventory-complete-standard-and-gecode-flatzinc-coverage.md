+++
schema_version = 1
id = "checker-010"
key = "full-coverage"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = ["checker-009"]
+++
# Inventory complete standard and Gecode FlatZinc coverage

## Outcome

A source-backed coverage table makes the full-support target finite and assigns every mathematical operation to an imported task family.

## Context

Read the new Full FlatZinc and library support section of the brief and its exact upstream links. The current Predicate enum implements ten names; preparation admits one external declaration. Inspect MiniZinc flatzinc_builtins.mzn, versioned redefinitions and standard decompositions, plus pinned Gecode gecode.mzn, gecode_fzn.mzn, all overrides/redefinitions and registry.cpp. The observed registry has 315 registration statements for 311 unique names, including aliases and feature-conditional repeats; do not equate that with unique predicate/signature coverage.

## Boundaries

- Use the fixed upstream revisions in the brief; no automatic main tracking or general plugin/registration system.

## Done when

- [ ] An indexed table records exact names/signatures and aliases from the pinned sources, current support, owning family/task, semantic source and useful deterministic outputs. Standard builtins, versioned/deprecated additions and Gecode set/float/MPFR-conditional registrations are included.
- [ ] Every mathematical predicate is assigned to one task in this bundle, including predicates reachable through each library include/decomposition path. Source-only wrappers are distinguished from direct external calls; no TODO or catch-all omission is counted as support.
- [ ] Text/JSON grammar, annotation classes, numeric/Gecode-universe boundaries and compiler-profile requirements are recorded compactly. Experimental blackbox/restart hooks are listed separately with the pure-checking exclusion and required diagnostic, not mislabeled as mathematical support.

## Validation

- Inspect source declarations and actual registry entries at the exact revisions; cross-check extracted names/signatures against both include trees and existing runtime cases.
- Run both zdev checks. A small extraction/check command may support the table; no new runtime, solver build or test framework is needed.
