+++
schema_version = 1
id = "base-154"
key = "pairwise-self-comparison"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-153", "base-152"]
+++
# Add pairwise-self-comparison for unsatisfiable equal pairs

## Outcome

A new default correctness rule reports enforced irreflexive comparisons between two generator bindings over the same source that never exclude the equal pair.

## Context

Read the New rules table in the brief's Lint review follow-up (2026-10-29). `constraint forall (i, j in S) (x[i] != x[j]);` cannot be satisfied when S is nonempty because the pair `i = j` requires `x[i] != x[i]`. The same holds for `<` and `>`, and for `forall (i in S, j in S)`. global-constraint-opportunity already recognises complete pairwise disequality with `i < j`; reuse its enforced-context and generator facts where they fit rather than adding parallel machinery. New rules register in `Rule`, `Rule::all()`, the catalogue match, presets and `Rule::DEFAULT`, and need a docs entry in their group page following existing rule entries.

## Boundaries

- Report only when substituting equal bindings makes both operands denote the same value, established from resolved identities: for example both sides index the same resolved array with the two bindings in the same positions. `x[i] != y[j]` over different arrays stays quiet.
- Report only enforced contexts: top-level constraints, conjunctions and `forall` bodies. A filter, guard or condition that excludes `i = j` makes the comparison quiet, as does any comparison the rule cannot prove irreflexive for the compared types.
- When the source is known to be nonempty, say the constraint cannot be satisfied. When nonemptiness is unknown, say it cannot be satisfied whenever the source is nonempty. A known-empty source stays quiet.
- No fix.

## Done when

- [x] The two forms in Context report one finding each at the comparison, naming both bindings.
- [x] Forms filtered by `i < j`, `i != j` or `j in i + 1 .. n`, comparisons inside a disjunction, comparisons of different arrays and a known-empty source stay quiet.
- [x] The rule is correctness family, warning severity, appended to the end of the default preset, and a member of the formulation preset.
- [x] `docs/lint-formulation.md` documents the rule with an example. The preset table and default description in `docs/linting.md`, and the CLI help listing of the default, include it.

## Validation

- Add a focused test file with the positive and quiet cases above.
- Run the brief's Rust validation commands and `zdev check base --format json`.
- Using a current release build, the existing corpus runner (`scripts/check-corpus.py`) or the ad hoc loop used by the lint review, report for the published Challenge corpus at `/Users/zayenz/gecode/dexter/problems/mzn-challenge-a8448864-clean` with the installed 2.10.1 stdlib and a 60 s per-file cutoff: analysed, timed-out and errored root totals, and the rule's finding count before and after the change. Read every finding the change introduces, or a sample of 20 if there are more, against the source. A confirmed false positive introduced by the change blocks completion; findings that existed before the change and unobserved roots are reported separately.

## Result

Added pairwise-self-comparison, a default correctness rule that reports an enforced !=, < or > between two bindings over one source that never excludes the equal pair. Known-empty sources, filters excluding the pair, conditional contexts, different arrays and optional elements stay quiet.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace, zdev check: all pass
- Release CLI: docs example exits 1 with the warning; empty-enum and index_set forms exit 0 with no limitation; Challenge corpus 413 analysed, 0 findings
- Independent Codex verifier: rework (empty-enum false positive), one correction, focused recheck pass on snapshot Wa1f1f874453b889d
