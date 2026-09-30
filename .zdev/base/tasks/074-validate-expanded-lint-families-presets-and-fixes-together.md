+++
schema_version = 1
id = "base-074"
key = "lint-expansion-acceptance"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-061", "base-062", "base-063", "base-064", "base-065", "base-066", "base-067", "base-068", "base-069", "base-070", "base-073", "base-049"]
+++
# Validate expanded lint families, presets and fixes together

## Outcome

The ten new rule families and workflow features have documented coverage, limitations and integrated validation.

## Context

Reuse the corpus runner and semantic lint baseline; base-050 remains the prior bundle's acceptance task. This gate owns the new rule/config/fix combination, not a second generic testing platform. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Account for every new stable ID, family, settings/preset path and fix producer through working CLI/library examples; reconcile skipped, inapplicable, limited and failed outcomes.
- [ ] Run selected rules over the existing available corpus, inspect representative findings for false positives, record unresolved correctness problems rather than claiming clean coverage.
- [ ] Exercise fixes only on staged public/synthetic copies with before/after parsing, preservation, second-pass stability and selected compiler acceptance; external originals remain unchanged.
- [ ] Measure added lint time/memory against the base-049 baseline using identical inputs and explicit comparable rule IDs before measuring the new selection; share facts among selected rules and update user docs with actual supported behaviour and limits.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Run the area checks, a compact preset/selection/fix integration scenario and the existing corpus/performance tools; use focused regressions only for concrete defects uncovered.
