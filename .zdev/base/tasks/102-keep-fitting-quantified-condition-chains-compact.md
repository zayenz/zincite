+++
schema_version = 1
id = "base-102"
key = "format-quantified-condition-chain"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Keep fitting quantified condition chains compact

## Outcome

Short conjunctions of quantified conditions in if and elseif headers remain readable on one line when they fit.

## Context

Found with local zincite-fmt at f950541 while revising Gecode's gecode_nooverlap interface. With --indent-size 2 --max-line-length 120, `if forall (w in width) (is_fixed(w)) /\ forall (h in height) (is_fixed(h)) then` expands to seven lines, although the complete header fits. base-095 fixed a single direct quantified condition but the conjunction still expands. Start in crates/zincite-fmt/src/layout.rs and tests/formatting.rs; follow the brief's Formatting direction. This is a readability issue; parsing, token/comment preservation and idempotence pass.

## Boundaries

- Preserve generator bodies with comments or deliberate multiline layout, code tokens, condition order and precedence.

## Done when

- [x] A fitting if or elseif header containing a conjunction of short forall conditions stays compact.
- [x] Comments, genuinely multiline bodies and headers exceeding the configured width retain readable expanded layout.

## Validation

- Add focused two/four-space formatter examples at fitting and non-fitting widths; check parsing, token/comment preservation and second-pass stability.
- Run the existing formatter tests and required Cargo checks.

## Result

Fitting if/elseif conjunctions of short forall conditions stay compact with preserved grouping and complete-header width checks.

Validation:

- Independent verifier passed at We32044bc151cf9c7; two/four-space exact-width regression, parsing, token/comment preservation and idempotence passed.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace passed.
