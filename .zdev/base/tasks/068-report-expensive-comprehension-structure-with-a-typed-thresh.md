+++
schema_version = 1
id = "base-068"
key = "lint-expensive-comprehension"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-056", "base-060"]
+++
# Report expensive comprehension structure with a typed threshold

## Outcome

expensive-comprehension explains potential expansion and repeated invariant work without claiming a measured runtime.

## Context

Consume iteration candidate bounds and dependencies. The threshold is defined in the brief and configurable through a preset or per-rule option; compiler optimization remains relevant to the advice. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [x] Deliver expensive-comprehension in family performance for candidate upper bounds above max-candidates and supported repeated invariant aggregate/filter structures.
- [x] Explain concrete counts or symbolic dimensions and filter dependencies; distinguish potential expansion from measured enumeration.
- [x] Do not suggest moving partial expressions or filters across invalid scopes; test option overrides and suppression.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check threshold boundary and override, symbolic product, a safely earlier filter, repeated invariant aggregate and a partial expression that cannot be hoisted.

## Result

Add expensive comprehension diagnostics for checked candidate counts, symbolic products and safe earlier filter or invariant aggregate structure.

Validation:

- Independent PASS; cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace passed (166 tests). Focused threshold, option override, suppression, symbolic dimension, scope, guard, partiality, skipped evaluation, effect and annotation checks passed; verified snapshot unchanged.
