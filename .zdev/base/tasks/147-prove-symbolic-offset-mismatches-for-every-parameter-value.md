+++
schema_version = 1
id = "base-147"
key = "lint-symbolic-offsets"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = ["base-146"]
+++
# Prove symbolic offset mismatches for every parameter value

## Outcome

index-set-mismatch and partial-expression report accesses whose index leaves the array's index set for every value of the unassigned parameters for which the access is evaluated.

## Context

Read the Lint review follow-up (2026-10-29) section of the brief and the semantic-extension rule in its Interpretation before policy section. Models usually declare sizes as unassigned data parameters. Today `int: n; array[1..n] of var 0..10: x; constraint forall (i in 1..n) (x[i] < x[i + 1]);` is silent. So is iterating `1..n` over an array indexed `0..n-1`. In the first case the last evaluated iteration, `i = n`, accesses `n + 1`, outside `1..n` for every n >= 1. When n < 1 the iteration is empty and nothing is evaluated. This task builds on the named-bound handling from the preceding task.

## Boundaries

- Support comparisons of bounds that are a single parameter or named size plus or minus an integer constant. Stay quiet on anything else, such as products, min/max or data-dependent sets.
- A finding requires a violating index for every parameter value that makes the access's iteration nonempty and leaves the access active. Guards, filters and inactive contexts protect accesses exactly as they do for literal bounds.
- Wording states that the traversal includes an index outside the array's index set whenever the traversal is active and nonempty. It must not say that every evaluated access is outside, and must not claim a runtime crash or infeasibility.

## Done when

- [ ] The two symbolic examples in Context report index-set-mismatch.
- [ ] `forall (i in 1..n - 1) (x[i] < x[i + 1])`, `forall (i in 1..n where i < n) (x[i] < x[i + 1])` and `forall (i in index_set(x)) (x[i] < y[i])` with matching declarations stay quiet.
- [ ] A symbolic access that only fails for some parameter values, such as `x[k]` with an unassigned `k`, stays quiet.
- [ ] The rule documentation in `docs/lint-arrays.md` describes symbolic offsets and the every-value condition.

## Validation

- Add the positive and quiet cases above to the existing index-set-mismatch tests, plus one partial-expression case.
- Run the brief's Rust validation commands and `zdev check base --format json`.
- Using a current release build, the existing corpus runner (`scripts/check-corpus.py`) or the ad hoc loop used by the lint review, report for the published Challenge corpus at `/Users/zayenz/gecode/dexter/problems/mzn-challenge-a8448864-clean` with the installed 2.10.1 stdlib and a 60 s per-file cutoff: analysed, timed-out and errored root totals, and the rule's finding count before and after the change. Read every finding the change introduces, or a sample of 20 if there are more, against the source. A confirmed false positive introduced by the change blocks completion; findings that existed before the change and unobserved roots are reported separately.
