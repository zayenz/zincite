+++
schema_version = 1
id = "base-103"
key = "format-short-conditional-comprehensions"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Keep fitting conditional array comprehensions compact

## Outcome

Short conditional array comprehensions stay readable on one line when the complete containing line fits.

## Context

Found with local zincite-fmt at f950541 while formatting Gecode's optional scheduling hooks. With --indent-size 2 --max-line-length 120, `array[int] of var int: start = [if is_fixed(si) /\ absent(fix(si)) then 0 else deopt(si) endif | si in s];` expands to five lines although the complete statement fits. The else/endif lines align with the declaration while the conditional begins after the opening array bracket, making the array expression hard to scan. Token/comment checks, parsing and idempotence pass. Start in crates/zincite-fmt/src/layout.rs and tests/formatting.rs; follow the area brief's Formatting direction.

## Boundaries

- Preserve comments, tokens, comprehension generators, expression grouping and explicit multiline bodies.

## Done when

- [x] A short conditional array comprehension may remain compact when its complete containing line fits the configured width.
- [x] Non-fitting and deliberately multiline conditional comprehensions retain clear indentation within their array brackets.

## Validation

- Add a few focused formatting examples at two/four-space indentation and fitting/non-fitting widths; check reparsing, token/comment preservation and second-pass stability.
- Run existing formatter tests and required Cargo checks.

## Result

Fitting short conditional array comprehensions stay compact; expanded heads and generators align clearly within brackets.

Validation:

- Independent verifier passed at W2370f38b344caba4; complete-line width, two/four-space indentation, comments/tokens, reparsing, grouping and second-pass stability passed.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace passed, including 29 formatter tests.
