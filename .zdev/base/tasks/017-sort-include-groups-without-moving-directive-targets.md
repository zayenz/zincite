+++
schema_version = 1
id = "base-017"
key = "include-sorting"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-010", "base-016"]
+++
# Sort include groups without moving directive targets

## Outcome

Existing include groups sort predictably with their attached comments and protected boundaries.

## Context

Read brief Remaining layout defaults and existing include/item spans. Use the formatting directive barriers from format-directives and recognize lint suppression comments as barriers without requiring the linter executable.

## Boundaries

- No new includes, include traversal or reordering of other items.
- Do not cross blank-line/section boundaries, formatting directives, lint directives or skipped spans.

## Done when

- [x] Eligible adjacent includes sort by decoded path with globals.mzn first and stable equal-path ordering.
- [x] Directly attached comments move with their include; separated comments remain section boundaries.
- [x] Formatting twice preserves the result and no sorting move changes which item any directive controls.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use one mixed include group with attached/section comments and duplicate paths, plus a directive/skipped barrier example. Check item order, comment text and stable second output.

## Result

Sorted eligible include groups by decoded paths with stable duplicates, attached comments and directive/protected barriers.

Validation:

- Independent PASS on Wbdfcc5c971561dd7; comments, path escapes, section/directive boundaries and idempotence verified.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all pass (44 tests).
