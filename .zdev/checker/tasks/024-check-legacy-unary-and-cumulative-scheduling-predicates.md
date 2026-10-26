+++
schema_version = 1
id = "checker-024"
key = "legacy-scheduling"
area = "checker"
status = "open"
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

## Done when

- [ ] All assigned legacy unary/disjunctive, optional unary, cumulative and multi-machine cumulatives forms check durations, resource usage, capacity and presence under their exact signature.
- [ ] Zero-duration tasks, absent tasks, upper/lower resource bounds, negative usages and legacy capacity behavior match upstream semantics. Validate fixed versus variable arguments and integer limits.
- [ ] Typed resource/conflict findings coexist with other findings; event-boundary evaluation avoids iteration over a potentially enormous time horizon. No starts are guessed and incomplete inputs stay reports.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
