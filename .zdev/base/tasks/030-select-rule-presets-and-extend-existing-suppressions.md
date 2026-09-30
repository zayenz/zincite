+++
schema_version = 1
id = "base-030"
key = "rule-selection"
area = "base"
status = "done"
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

- [x] Implement --rules default|thesis|all and explicit comma-separated IDs with the exact brief semantics, stable catalogue IDs, and rejection of unknown/repeated selection.
- [x] Preserve lint(&ParsedFile) defaults; use selected options through a library API. Every registered ID can be suppressed even when disabled, without affecting other rules.
- [x] Before each rule lands, selecting an unimplemented rule reports a clear unavailable-rule error; neither preset nor empty stub reports successful thesis coverage. Remove that temporary limitation as implementations land.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check unchanged default warnings/statuses, exact preset membership, explicit selection and disabled-rule suppressions with a few existing CLI fixtures.

## Result

Added stable rule catalogue and library/CLI selection with exact presets, independent suppressions and explicit unavailable-rule errors.

Validation:

- Workspace fmt, clippy with -D warnings, all 65 tests and diff check passed.
- Public library and CLI checks confirm exact two/fourteen/sixteen preset membership, unchanged defaults, explicit selections, duplicate-ID deduplication and malformed or repeated selection errors.
- All registered IDs can be suppressed independently when disabled; unavailable rules fail even for empty stdin or directory input, and directive placement errors remain intact.
- Independent whole-task PASS and exact snapshot comparison W4edd44e9fdb2c75c.
