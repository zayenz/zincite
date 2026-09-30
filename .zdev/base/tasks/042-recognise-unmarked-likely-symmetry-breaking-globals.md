+++
schema_version = 1
id = "base-042"
key = "symmetry-advice"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-033"]
+++
# Recognise unmarked likely symmetry-breaking globals

## Outcome

Thesis rule unmarked-symmetry-breaking covers the full named family as explicit advice.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.11 and every function name in its coverage row. Resolve both standard predicates and symmetry_breaking_constraint rather than trusting spelling.

## Done when

- [ ] Report the complete documented predicate family when outside a resolved symmetry wrapper, with a message that asks whether the constraint is intended to break symmetry.
- [ ] Respect nested wrappers and user shadowing; never claim the predicate proves symmetry or that removing it preserves solutions.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use a small table-driven check of the finite predicate list and focused wrapped/shadowed/required-constraint wording examples.
