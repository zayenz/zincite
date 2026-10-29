+++
schema_version = 1
id = "base-152"
key = "lint-families-and-severity"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Record default severity per rule and reassign style families

## Outcome

Each catalogue entry records its default severity. The listed advisory rules report informational findings, and the four reassigned rules belong to the style family.

## Context

Read the Families and Default severity paragraphs of the brief's Lint review follow-up (2026-10-29). Today almost every finding is `Severity::Warning`, so advice fails runs. `RuleMetadata` in `crates/zincite-lint/src/rules.rs` is the single catalogue entry used by `--list-rules` and `--explain`. Rule modules set `severity` on each finding; `expensive_comprehension.rs` already emits both severities. The corpus tooling also consumes severity: `crates/zincite-fmt/examples/check-corpus-file.rs` writes `result.findings.len()` as `warning_count`, and `semantic_report_valid` in `scripts/check-corpus.py` then requires exit status 1 whenever that count is positive, so an informational-only root would fail report validation.

## Boundaries

- Severity is not configurable.
- expensive-comprehension keeps its current split.
- Exit status policy is unchanged: unsuppressed warnings fail; informational findings alone exit 0.

## Done when

- [ ] Every finding from the eleven informational rules has informational severity, and every other rule's findings are unchanged.
- [ ] `--list-rules` and `--explain RULE` show each rule's default severity alongside its family.
- [ ] array-index-start, global-variable-in-function, element-predicate and unused-generator-binding report family `style`, and `family:style` selects them.
- [ ] `docs/linting.md` and the group pages under `docs/lint-*.md` describe the severity split and the new family membership.
- [ ] The corpus helper reports warning and informational totals separately, and `scripts/check-corpus.py` derives the expected status from warnings only. An informational-only root passes report validation.

## Validation

- Update the existing catalogue/family tests and one CLI check showing a model with only informational findings exiting 0. Run the corpus helper on one informational-only model and confirm its report validates.
- Run the brief's Rust validation commands and `zdev check base --format json`.
