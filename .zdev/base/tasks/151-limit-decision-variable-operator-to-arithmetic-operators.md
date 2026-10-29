+++
schema_version = 1
id = "base-151"
key = "narrow-decision-operator"
area = "base"
status = "open"
complexity = "routine"
afk = true
priority = "normal"
blocked_by = []
+++
# Limit decision-variable-operator to arithmetic operators

## Outcome

decision-variable-operator reports only `^`, `div`, `mod` and `/` with decision-dependent operands.

## Context

The rule produced 18,753 findings in the corpus review, mostly ordinary `\/`, `->` and `not`. The brief's Lint review follow-up narrows it. Start in `crates/zincite-lint/src/decision_use.rs`. Row 4.10 in `.zdev/base/background/thesis-rule-coverage.md` lists the old operator set.

## Boundaries

- Keep the rule ID and its membership in the thesis and flattening presets.

## Done when

- [ ] Decision-dependent `xor`, disjunction, implication in either direction, equivalence and `not` produce no finding.
- [ ] Decision-dependent `^`, `div`, `mod` and `/` are still reported.
- [ ] Row 4.10 of the thesis coverage background, the catalogue purpose/limitations and `docs/lint-flattening.md` describe the narrowed set and note that it deliberately departs from the thesis.

## Validation

- Update the existing decision-use tests.
- Run the brief's Rust validation commands and `zdev check base --format json`.
