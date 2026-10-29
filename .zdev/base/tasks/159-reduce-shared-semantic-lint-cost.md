+++
schema_version = 1
id = "base-159"
key = "default-lint-cost"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Reduce shared semantic lint cost

## Outcome

Default linting of moderately sized generated Challenge models completes in seconds instead of timing out, with unchanged results.

## Context

Read the Cost paragraph of the brief's Lint review follow-up and the performance background linked from the brief. Selecting index-set-mismatch, partial-expression, hidden-optionality, vacuous-constraint or unbounded-variable alone each took 38 to 53 s on the 93 KB `2014/liner-sf-repositioning/tp7_8.mzn` and over 60 s on `2021/connect/connect__0018.mzn`. prefer-array-comprehension, suspicious-shadowing and unused-declaration took about 1 s on both. The hypothesis is a shared prerequisite of the slow rules, possibly guarded/iteration fact production (`guarded.rs`, `iteration.rs`, `domains.rs`), but numeric and callable facts are also shared; let the profile identify the owning module. Both files are under `/Users/zayenz/gecode/dexter/problems/mzn-challenge-a8448864-clean`. Use a release build and the installed 2.10.1 stdlib.

## Boundaries

- Profile before changing code and fix the dominant cost found. Do not change semantic results or add caching layers that the profile does not justify.
- Use only a release build of current sources and the two named models for timing; a broad benchmark campaign is out of scope.

## Done when

- [x] `zincite lint --isolated --rules index-set-mismatch,partial-expression,hidden-optionality,vacuous-constraint,unbounded-variable` completes on each named model in under 10 s on the user's M1 Max.
- [x] Exit status and complete stderr with `--show-limitations` for that selection are byte-identical before and after the change on both models, and the repository's existing lint tests pass unchanged.
- [x] The handoff reports the profile evidence, the cause and before/after timings.

## Validation

- Run the brief's Rust validation commands and `zdev check base --format json`.
- Compare exit status and `--show-limitations` stderr before and after on both named models.

## Result

Replaced two linear reference scans and one linear expression-type scan in opaque-source checking with indexed lookups. The five shared default rules now take about 1.9 s on tp7_8.mzn (was 59 s) and about 8.3 s on connect__0018.mzn (was 127 s), with byte-identical output.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace (679 passed), zdev check: all pass
- Release five-rule selection: tp7_8 61.9 s -> 2.35 s, connect__0018 130 s -> 8.68 s (verifier reproduction); stderr byte-identical and exit codes unchanged
- Independent Codex verifier: pass on snapshot W2d8164ed3cbba6bf
