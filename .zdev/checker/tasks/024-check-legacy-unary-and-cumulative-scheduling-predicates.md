+++
schema_version = 1
id = "checker-024"
key = "legacy-scheduling"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-012", "checker-014"]
+++
# Check legacy unary and cumulative scheduling predicates

## Outcome

The legacy scheduling interfaces emitted by standard and Gecode libraries are fully checked.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Inspect installed-example strict semantics, pinned cumulative overrides and registry handlers for schedule_unary, cumulative_optional and legacy cumulatives. Do not apply the later explicit-end contracts to legacy signatures.

The user selected concrete candidate checking for four-argument `cumulatives`
and its aliases. Apply the brief's explicit exception to domain-dependent
Gecode posting behavior and document the concrete relation. The other legacy
interfaces follow their pinned upstream mathematical semantics.

## Done when

- [x] All assigned legacy unary/disjunctive, optional unary, cumulative and multi-machine cumulatives forms check durations, resource usage, capacity and presence under their exact signature.
- [x] Zero-duration tasks, absent tasks, upper/lower resource bounds, negative usages and legacy capacity behavior match upstream semantics, with the brief's concrete-value exception for four-argument `cumulatives`. Validate fixed versus variable arguments and integer limits.
- [x] Typed resource/conflict findings coexist with other findings; event-boundary evaluation avoids iteration over a potentially enormous time horizon. No starts are guessed and incomplete inputs stay reports.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added legacy unary, optional cumulative and four/six-argument cumulatives checks with typed findings, variable-duration strict support and concrete-value legacy semantics. Scheduling uses task events, selects no outputs and preserves fixed strict artifacts.

Validation:

- Independent full review W60bd19d733155010 passed with no required corrections; snapshot comparison equal.
- Workspace formatting, Clippy with warnings denied and 631 tests passed; parser-free check/build and 15 tests passed.
- Locked release Wasm and Node/Web bindings passed; syntax-free dependency trees and 108 native/Wasm comparisons passed across 13 artifacts and 54 candidates.
- Both controls compiled under both pinned library profiles; 80 source/load and both-CLI comparisons passed, with control formatting and default lint.
- Coverage check reports 791 admissions; Ruff, ty, zdev base/checker checks and diff whitespace checks passed.
