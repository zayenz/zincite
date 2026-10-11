+++
schema_version = 1
id = "base-096"
key = "format-conjunction-chains"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Fix formatter indentation and wrapping of conjunction chains

## Outcome

Expanded conjunctions align their operands and wrap nested calls using the space available on their own lines.

## Context

Observed while formatting the Gecode MiniZinc registry PR with the local Zincite source checkout at 6edaf0a. Reproduce with zincite-fmt --indent-size 2 --max-line-length 120. Read the area brief Formatting direction and start in crates/zincite-fmt/src/layout.rs and tests/formatting.rs. The output reparses and is idempotent; this is a readability problem, not evidence of changed model behavior.

Input:

```minizinc
predicate regular(array[int] of var bool: xs, int: Q, set of int: S,
    array[int,int] of int: d, int: q0, set of int: F) =
  assert(Q > 0, "regular: 'Q' must be greater than zero") /\
  assert(card(S) > 0, "regular: 'S' must be non empty") /\
  assert(index_set_1of2(d) = 1..Q /\ index_set_2of2(d) = S,
    "regular: the transition function 'd' must be [1..Q,S]") /\
  assert(forall(v in d)(v in 0..Q),
    "regular: transition function 'd' points to states outside 0..Q") /\
  assert(q0 in 1..Q, "regular: start state 'q0' not in 1..Q") /\
  assert(F subset 1..Q, "regular: final states in 'F' contain states outside 1..Q") /\
  let {var bool: result;} in result;
```

Current output places two assertions after the signature, then starts one assert at two spaces, a later assert at column zero, and the following let at column zero. In the Gecode regular_nfa overload, preceding assertions also cause length(x) to expand over three lines although it fits by itself. Each top-level conjunct should have consistent indentation, with /\ at the end of the preceding conjunct and nested call arguments indented beneath their own call.

## Boundaries

- Keep operator order, precedence, parentheses, comments, and literal spelling. Keep the existing trailing-conjunction convention and avoid synthetic true operands.

## Done when

- [x] An expanded assertion/let/conditional conjunction uses one meaningful conjunct per line or block, aligned consistently relative to its enclosing expression.
- [x] Nested assertions, generator calls, and conditional operands keep their local indentation.
- [x] A short call such as length(x) stays compact when it fits on its current line; the width of an earlier conjunct does not force it to expand.

## Validation

- Add a small regression set based on the assertion chain and regular_nfa short-call case; check the two- and four-space layouts.
- Check reparsing, token/comment preservation, configured width, and second-pass idempotence; run the existing area Cargo checks.

## Result

Aligned expanded conjunction operands and preserved nested expression layout so preceding conjuncts do not force short calls to expand.

Validation:

- Independent whole-task review passed; snapshot Wc5965de141910d43 comparison equal true.
- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace passed independently.
- Regression and Gecode regular/regular_nfa probes passed at two/four spaces with token/comment/tree preservation, width checks and second-pass idempotence; base-095 checks remain passing.
