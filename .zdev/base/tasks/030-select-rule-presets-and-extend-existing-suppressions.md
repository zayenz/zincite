+++
schema_version = 1
id = "base-030"
key = "rule-selection"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Select rule presets and extend existing suppressions

## Outcome

The library and CLI can select rule sets, retaining current defaults and next-item suppression behavior.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Start with Rule, lint, item_suppressions in zincite-lint/src/lib.rs and CLI argument parsing. The user chose opt-in thesis rules; no plugin/configuration architecture is needed.

## Done when

- [ ] Implement --rules default|thesis|all and explicit comma-separated IDs with the exact brief semantics, stable catalogue IDs, and rejection of unknown/repeated selection.
- [ ] Preserve lint(&ParsedFile) defaults; use selected options through a library API. Every registered ID can be suppressed even when disabled, without affecting other rules.
- [ ] Before each rule lands, selecting an unimplemented rule reports a clear unavailable-rule error; neither preset nor empty stub reports successful thesis coverage. Remove that temporary limitation as implementations land.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check unchanged default warnings/statuses, exact preset membership, explicit selection and disabled-rule suppressions with a few existing CLI fixtures.
