+++
schema_version = 1
id = "base-146"
key = "lint-named-bounds"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = []
+++
# Resolve known named bounds in index, definedness and vacuity checks

## Outcome

index-set-mismatch, partial-expression and vacuous-constraint report their existing findings when an index set, iteration range or accessed index is written through parameters or named sets with known values.

## Context

Read the Lint review follow-up (2026-10-29) section of the brief first. In a corpus run, these rules fired only on literal ranges. With the installed 2.10.1 stdlib, `int: n = 3; array[1..n] of var 0..10: x; constraint x[4] = 1;` and `set of int: I = 1..3; array[I] of var 0..10: x; constraint x[4] = 1;` produce no index-set-mismatch finding and no limitation. The literal form `array[1..3]` reports `index space {4} is wholly outside`. partial-expression is silent on `int: n = 7; array[1..n] of int: required; int: next_week = required[n + 1];`. vacuous-constraint is silent on its documented example when `1 .. 7` is replaced by `set of int: Days = 1 .. 7`. The same gap appears with the test stub stdlib from `crates/zincite-lint/tests/index_set_mismatch.rs`, so the real stdlib is not the cause. array-index-start already resolves `set of int: I = 0..n` with `int: n = 3`, so domain facts exist. The gap is probably where index spaces and accessed indices are interpreted for these rules: look at `index_set_mismatch.rs`, the guarded/iteration facts in `guarded.rs` and `iteration.rs`, and `domains.rs`. Diagnose before changing anything.

## Boundaries

- Use only values established by unconditional initializers of parameters and named sets, including arithmetic over such values. Unassigned parameters stay unknown; symbolic reasoning belongs to a later task.
- Unknown values stay quiet, as they do now. Do not weaken guard, inactive-evaluation or empty-iteration protections.
- Do not change rule selection, severity or families.

## Done when

- [x] The named-parameter and named-set examples in Context each produce the same index-set-mismatch or partial-expression finding as their literal counterparts.
- [x] vacuous-constraint reports the documented impossible `where` filter when the iteration source is a named set with a known value.
- [x] Corrected counterparts, such as an in-range access or an index guarded by a known-valued condition, stay quiet.
- [x] Existing tests for the three rules still pass without changing their expected findings, except where a test asserted the old silence and the brief now requires a finding.

## Validation

- Add a positive and a quiet case for each of the three rules to their existing test files, using named bounds.
- Run the brief's Rust validation commands and `zdev check base --format json`.
- Using a current release build, the existing corpus runner (`scripts/check-corpus.py`) or the ad hoc loop used by the lint review, report for the published Challenge corpus at `/Users/zayenz/gecode/dexter/problems/mzn-challenge-a8448864-clean` with the installed 2.10.1 stdlib and a 60 s per-file cutoff: analysed, timed-out and errored root totals, and the rule's finding count before and after the change. Read every finding the change introduces, or a sample of 20 if there are more, against the source. A confirmed false positive introduced by the change blocks completion; findings that existed before the change and unobserved roots are reported separately.

## Result

Index, definedness and vacuity checks now use values from unconditional initializers of parameters and named sets, including declared-range parameters and div/mod arithmetic. Invariant/advice consumers are unchanged.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace (674 passed), zdev check: all pass
- Challenge corpus (3 rules): vacuous-constraint +9 true positives (constant config switches, one empty forall), index-set-mismatch/partial-expression 0 -> 0, no other added findings or limitations
- Independent Codex verifier: rework (2 findings, both fixed) then focused recheck pass; snapshot Wc35e1323ed0e9458
