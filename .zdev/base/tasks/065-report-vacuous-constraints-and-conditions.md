+++
schema_version = 1
id = "base-065"
key = "lint-vacuity"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-060", "base-059"]
+++
# Report vacuous constraints and conditions

## Outcome

vacuous-constraint identifies statically established empty or ineffective constraint contexts.

## Context

Consume guarded truth and iteration emptiness. Parameter-dependent conditions are unknown unless justified; do not guess the modeller's intended constraint. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [x] Deliver vacuous-constraint in family suspicious for proven empty quantifiers, impossible filters, tautological constraints and contradictory conditions.
- [x] Describe the exact proven fact and preserve unknown parameter distinctions; optional/partial expressions require the shared semantic interpretation.
- [x] Keep warnings independently selectable/suppressible and do not delete constraints or conditions automatically.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check an empty forall, impossible filter, self-comparison with total versus partial operands, contradiction and a symbolic range.

## Result

Added opt-in vacuous-constraint advice for proved empty quantifiers, rejecting filters, true constraints and false conditions, with explicit condition context and conservative optional/partial/unknown handling.

Validation:

- Independent verification passed on W7bd4b138b9805f64; root official comparison equal. Formatting, Clippy and all 154 workspace tests pass; native controls match statuses 1/1 and exact streams. Context-attribution regression fails before correction and passes afterward.
