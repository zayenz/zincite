+++
schema_version = 1
id = "checker-010"
key = "full-coverage"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = ["checker-009"]
+++
# Inventory complete standard and Gecode FlatZinc coverage

## Outcome

A source-backed coverage table makes the full-support target finite and assigns every mathematical operation to an imported task family.

## Context

Read the new Full FlatZinc and library support section of the brief and its exact upstream links. The current Predicate enum implements ten names; preparation admits one external declaration. Inspect MiniZinc flatzinc_builtins.mzn, versioned redefinitions and standard decompositions, plus pinned Gecode gecode.mzn, gecode_fzn.mzn, all overrides/redefinitions and registry.cpp. The registry has 315 textual registration matches for 311 unique names; three matches are commented out. Its 312 active registration statements name 308 unique strings, including aliases and feature-conditional repeats. Neither count establishes unique predicate/signature coverage.

## Boundaries

- Use the fixed upstream revisions in the brief; no automatic main tracking or general plugin/registration system.

## Done when

- [x] An indexed table records exact names/signatures and aliases from the pinned sources, current support, owning family/task, semantic source and useful deterministic outputs. Standard builtins, versioned/deprecated additions and Gecode set/float/MPFR-conditional registrations are included.
- [x] Every mathematical predicate is assigned to one task in this bundle, including predicates reachable through each library include/decomposition path. Source-only wrappers are distinguished from direct external calls; no TODO or catch-all omission is counted as support.
- [x] Text/JSON grammar, annotation classes, numeric/Gecode-universe boundaries and compiler-profile requirements are recorded compactly. Experimental blackbox/restart hooks are listed separately with the pure-checking exclusion and required diagnostic, not mislabeled as mathematical support.

## Validation

- Inspect source declarations and actual registry entries at the exact revisions; cross-check extracted names/signatures against both include trees and existing runtime cases.
- Run both zdev checks. A small extraction/check command may support the table; no new runtime, solver build or test framework is needed.

## Result

Created and independently verified the pinned standard/Gecode coverage inventory, source-check helper and documentation links. Every indexed operation has task ownership; current support, planned work and excluded hooks are distinguished.

Validation:

- Independent source comparisons matched 3168 declarations, 993 explicit include edges, 312 active registrations and 924 exact aliases across 743 standard and 102 Gecode files; default include closures resolve 605/604 files.
- Recorded eight standard and six Gecode-profile optional decreasing include gaps, seven malformed EOF hook prototypes, feature conditions, exact signatures and current ten-name admission limits against both fixed clean source revisions.
- Full review plus one correction pass and focused recheck passed against W15a85ce439be2fa5. Corrected float-extremum output positions and functional Boolean implication outputs without changing guarded forms.
- Normal inventory comparison, focused semantic/alias assertions, Ruff format/lint, ty, both zdev checks and git diff --check passed. No runtime changes or new runtime tests were required.
