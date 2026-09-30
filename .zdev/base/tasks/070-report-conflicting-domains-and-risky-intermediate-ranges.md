+++
schema_version = 1
id = "base-070"
key = "lint-domain-bounds"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054", "base-057"]
+++
# Report conflicting domains and risky intermediate ranges

## Outcome

suspicious-domain identifies proved domain conflicts and explains significant intermediate bounds precisely.

## Context

Consume numeric interpretation from the separate task. Missing domains already belong to unbounded-variable; this rule must add evidence rather than duplicate it. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Deliver suspicious-domain in family suspicious for a known definition disjoint from its declared domain, and intermediate bounds incompatible with an explicit destination/domain contract.
- [ ] Report the derived and required ranges with the relevant expressions; distinguish a proven contradiction from a conservative possible conflict.
- [ ] Unknown values and arithmetic overflow in the analyser become limitations; do not invent universal backend integer limits or arbitrary large-domain warnings.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check an initializer/domain contradiction, a supported intermediate/destination mismatch, a broad but valid domain, unknown parameter and checked-arithmetic failure.
