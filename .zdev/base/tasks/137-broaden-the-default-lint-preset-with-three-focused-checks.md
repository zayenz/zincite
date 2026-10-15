+++
schema_version = 1
id = "base-137"
key = "broader-default-lints"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-136"]
+++
# Broaden the default lint preset with three focused checks

## Outcome

The default preset retains existing advice and adds a modest set of actionable checks.

## Context

The existing default has naming, missing-constraint-label and prefer-array-comprehension only. User requests a slightly broader default along with semantic groups. Add exactly index-set-mismatch, suspicious-shadowing and partial-expression. Start with Rule::DEFAULT, configuration defaults, library entry points and docs. Semantic preset mappings belong to the preceding task.

## Boundaries

- Keep all other rules opt-in; do not change severity or fix safety.

## Done when

- [ ] Omitted selection and preset:default/default select the same six rules in deterministic order, preserving the original three first.
- [ ] CLI/config overrides and suppressions remain effective; default analysis retains explicit limitations and never enables automatic writes.
- [ ] Document the changed default and its semantic/include requirements; synchronize earlier brief default statements as implemented behavior.

## Validation

- Update the existing default-selection control and exercise a small public model with one new finding and a quiet counterpart through both command forms. Run existing settings and suppression tests.
- Run the brief's required Rust validation for code changes; use existing focused controls and only the few behavior checks specified here.
- Run zdev check base --format json for changed records.
