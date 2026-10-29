+++
schema_version = 1
id = "base-157"
key = "ambiguous-precedence"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "low"
blocked_by = ["base-152"]
+++
# Add ambiguous-precedence for easily misread Boolean operators

## Outcome

A new opt-in source-only rule reports two unparenthesized Boolean operator patterns whose MiniZinc 2.10.1 grouping is easy to misread.

## Context

The 2.10.1 specification's operator table (https://docs.minizinc.dev/en/2.10.1/spec.html#operators) is the parse authority: `/\` binds tighter than `\/` and `xor`, and `->` and `<-` share one precedence level and are left-associative. A MiniZinc 2.10.1 run confirmed `false -> false -> false` evaluates to false, the grouping `(false -> false) -> false`, and `true \/ false /\ false` evaluates to true, the grouping `true \/ (false /\ false)`. The rule reports exactly these patterns:

1. `/\` and `\/` (or `xor`) at one level without parentheses. Positive: `a \/ b /\ c`, grouped `a \/ (b /\ c)`. Quiet: `a \/ (b /\ c)`.
2. Two or more `->`/`<-` operators in one unparenthesized chain. Positive: `a -> b -> c`, grouped `(a -> b) -> c`. Quiet: `a -> (b -> c)` and `(a -> b) -> c`.

`a /\ b -> c` stays quiet. Like naming and missing-constraint-label, the rule needs no model context; the formatter's precedence handling in `crates/zincite-fmt` shows how the CST represents these operators.

## Boundaries

- Source-only: `requires_model` is false and no stdlib is needed.
- Report only the two patterns in Context; `not`, `<->` chains and arithmetic are out of scope.
- Opt-in only. No fix; the formatter never adds parentheses.

## Done when

- [x] Each positive example in Context reports one finding at the operator, naming its actual grouping.
- [x] Each quiet example in Context, including `a /\ b -> c`, reports nothing.
- [x] The rule is suspicious family, warning severity, not in the default, and a member of the conventions preset.
- [x] `docs/lint-conventions.md` documents the rule, its two patterns and the specification basis.
- [x] The conventions row of the preset table in `docs/linting.md` lists the rule.

## Validation

- Add a focused test with each positive and quiet example in Context.
- Run the brief's Rust validation commands and `zdev check base --format json`.

## Result

Added ambiguous-precedence, an opt-in source-only suspicious rule in the conventions preset that reports an unparenthesized /\ inside \/ or xor and chains of two or more ->/<-, naming the actual grouping.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace (681 passed), zdev check: all pass
- Release CLI: both patterns report with grouping messages; parenthesized forms, a /\ b -> c, <-> chains and backquoted calls stay quiet; groupings confirmed against the MiniZinc 2.10.1 compiler
- Independent Codex verifier: pass on snapshot Wa3bad4154058017b
