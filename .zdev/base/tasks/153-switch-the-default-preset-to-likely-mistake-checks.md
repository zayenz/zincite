+++
schema_version = 1
id = "base-153"
key = "quieter-default-preset"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "high"
blocked_by = ["base-149"]
+++
# Switch the default preset to likely-mistake checks

## Outcome

Omitted selection, `default` and `preset:default` select the eight rules listed in the brief's Lint review follow-up, in that order. naming and missing-constraint-label run only when selected, for example through `preset:conventions`.

## Context

Read the Default preset paragraph of the brief's Lint review follow-up (2026-10-29). `Rule::DEFAULT` in `crates/zincite-lint/src/rules.rs` is the source of the preset. The previous default change was base-137; its touch points are a useful map: configuration defaults, library entry points, CLI help text, `docs/linting.md` and the Default columns in `docs/lint-*.md`. The staffing walkthrough in `docs/first-model.md` already runs `--isolated --rules naming,missing-constraint-label` explicitly; keep that command and update only its explanation of what an ordinary default run checks.

## Boundaries

- Do not change any rule's behavior, family or severity.
- Keep explicit `--rules`, configuration selection and suppressions working as before.

## Done when

- [ ] Omitted selection and both default selectors return the eight rules in the brief's order.
- [ ] A model with an unlabeled constraint and a camelCase name lints clean under the default and reports both under `--rules preset:conventions`.
- [ ] The CLI help, `docs/linting.md`, the group page Default columns and the first-model walkthrough's explanation describe the new default and show how to enable conventions, for example `select = ["default", "preset:conventions"]` in `zincite.toml`. `docs/linting.md` refers to every registered rule instead of a hard-coded rule count.
- [ ] CHANGELOG.md records the default change under unreleased changes.

## Validation

- Update the existing default-selection checks in `crates/zincite-lint/tests/linting.rs` and `tests/cli.rs`, and run the first-model walkthrough's documented lint command by hand to confirm its output still matches the page.
- Run the brief's Rust validation commands and `zdev check base --format json`.
