+++
schema_version = 1
id = "base-104"
key = "indent-expanded-if-conditions"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Indent expanded conditional headers consistently

## Outcome

Split if and elseif conditions align continuation lines with the first condition after the keyword.

## Context

Found in Gecode PR253 after Zincite formatting at f950541. In gecode/flatzinc/mznlib/fzn_if_then_else_bool.mzn, the condition combines length(c) = 2, index_sets_agree(c, x), and (if is_fixed(c[max(index_set(c))]) then fix(c[max(index_set(c))]) else false endif). Zincite prints the first condition after if, but the remaining conjuncts and nested else/endif at the indentation of the outer if keyword. This obscures which conditional owns them. All eight typed conditional hooks showed this layout. It reproduces with --indent-size 2 --max-line-length 120; parsing and token/comment preservation pass. Start in crates/zincite-fmt/src/layout.rs and tests/formatting.rs; follow the brief Formatting direction. The user clarified the desired layout: keep the first condition after the keyword and align continued operands with its start, three columns after `if` and seven after `elseif`. Nested conditional expressions retain their relative indentation; do not move the first condition onto a new line.

## Boundaries

- Preserve condition tokens, grouping, conjunction order and comments.

## Done when

- [x] A split if/elseif condition keeps its first operand after the keyword and aligns continued operands with that operand, including nested conditional expressions (three columns after if, seven after elseif).
- [x] The then delimiter and branch body make the boundary between condition and result clear; fitting simple conditions remain compact.

## Validation

- Add a focused regression based on this conjunction with a nested if, at two/four-space indentation and fitting/non-fitting widths. Check reparsing, tokens/comments and second-pass stability.
- Run the existing formatter tests and required Cargo checks.

## Result

Expanded if/elseif headers align continued operands at the keyword condition column, preserve nested block indentation and comments, and retain fitting compact headers. Independent verifier passed unchanged snapshot Wb58f125f57aa2390.

Validation:

- Independent cargo fmt check, workspace clippy with denied warnings, and workspace tests passed; focused regression and retained disjunction/header-comment probes preserve tokens, tree structure and second-pass stability.
- Existing six-case save recheck: 600 expected outcomes, maximum p95 34.155 ms and exact-child RSS 8.016 MiB; no corpus sweep.
