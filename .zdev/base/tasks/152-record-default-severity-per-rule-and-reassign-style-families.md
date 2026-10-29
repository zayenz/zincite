+++
schema_version = 1
id = "base-152"
key = "lint-families-and-severity"
area = "base"
status = "done"
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

- [x] Every finding from the eleven informational rules has informational severity, and every other rule's findings are unchanged.
- [x] `--list-rules` and `--explain RULE` show each rule's default severity alongside its family.
- [x] array-index-start, global-variable-in-function, element-predicate and unused-generator-binding report family `style`, and `family:style` selects them.
- [x] `docs/linting.md` and the group pages under `docs/lint-*.md` describe the severity split and the new family membership.
- [x] The corpus helper reports warning and informational totals separately, and `scripts/check-corpus.py` derives the expected status from warnings only. An informational-only root passes report validation.

## Validation

- Update the existing catalogue/family tests and one CLI check showing a model with only informational findings exiting 0. Run the corpus helper on one informational-only model and confirm its report validates.
- Run the brief's Rust validation commands and `zdev check base --format json`.

## Result

Each catalogue entry records its default severity; the eleven advisory rules report informational findings and four rules moved to the style family. --list-rules and --explain show severity, the corpus tooling counts warnings and informational findings separately, and the docs-site example renderer accepts informational findings.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace, zdev check, site npm run build: all pass
- Release CLI: informational-only search-coverage model exits 0; --rules family:style selects the reassigned rules; check-corpus.py validates an informational-only root
- Independent Codex verifier: rework (site example renderer required warnings), one correction, focused recheck pass on snapshot We9b7548a50bda54b
