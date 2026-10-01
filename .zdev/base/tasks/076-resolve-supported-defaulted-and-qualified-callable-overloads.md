+++
schema_version = 1
id = "base-076"
key = "semantic-call-ranking"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-075"]
+++
# Resolve supported defaulted and qualified callable overloads

## Outcome

CallableFacts resolves the compiler-supported defaulted and qualifier-specific calls that currently become invented ambiguities in the corpus consumers.

## Context

Read brief Interpretation before policy and scripts/semantic-corpus-checkpoint.md. Compiler-accepted choose_present and optional-guard reductions reproduce Ambiguous outcomes; actual all_different defaults and optional overloads reproduce the first cause. The current matcher appends omitted default targets before its equal-length minimum test and allows generic pattern order to veto strictly narrower instantiation/optionality. Raw public facts and independent checks are retained under target/corpus/base045/probes.

## Boundaries

- Keep the existing direct component-coercion matcher, exact declaration identities, actual/default formal mapping and instantiated body contracts. Do not introduce scores, name-based selection, a general type engine or policy-specific guesses.
- Unknown possible candidates must still prevent a confident selection. Preserve ambiguity for real competing implementations and the existing no-default prototype/unique-implementation contract.

## Done when

- [ ] Compare applicable candidates on the correct supplied-argument correspondence while retaining all instantiated formal types and default dependencies in the resolved result. Resolve the supported defaulted present-array versus optional-array control and the original all_different call after loader repair.
- [ ] Respect strict instantiation/optionality coercion when comparing supported generic and concrete candidates. Resolve absent(opt int) to its parameter operation and parameter integer <= to a parameter operation without erasing genuine generic or incomparable ambiguity.
- [ ] Check actual consumer consequences: the optional guard has a Parameter fact and its decision-variable-condition analysis completes; parameter <= retains its already-Parameter guard result; element/global/unused consumers no longer receive the demonstrated invented ambiguities. Retain unrelated unsupported conditional, partiality or symbolic limits.

## Validation

- Use a few combined public CallableFacts/selected-analysis regressions including an applicable user overload and unknown-candidate control; preserve source ranges and default scope.
- Run pinned MiniZinc 2.10.1 compiler-only controls, required workspace fmt/clippy/tests and selected installed-core/global replays. No solver, broad type matrix or full-corpus campaign is needed for this repair.
