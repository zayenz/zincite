+++
schema_version = 1
id = "base-137"
key = "broader-default-lints"
area = "base"
status = "done"
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

- [x] Omitted selection and preset:default/default select the same six rules in deterministic order, preserving the original three first.
- [x] CLI/config overrides and suppressions remain effective; default analysis retains explicit limitations and never enables automatic writes.
- [x] Document the changed default and its semantic/include requirements; synchronize earlier brief default statements as implemented behavior.

## Validation

- Update the existing default-selection control and exercise a small public model with one new finding and a quiet counterpart through both command forms. Run existing settings and suppression tests.
- Run the brief's required Rust validation for code changes; use existing focused controls and only the few behavior checks specified here.
- Run zdev check base --format json for changed records.

## Result

Default now selects six focused rules in order; catalogue, configuration, suppressions and limitations are preserved, with synchronized docs and CLI help.

Validation:

- Full independent review and focused help recheck passed. Workspace rustfmt, Clippy with warnings denied and 459 tests passed; both CLI positive/quiet and CRLF-preservation controls passed.
