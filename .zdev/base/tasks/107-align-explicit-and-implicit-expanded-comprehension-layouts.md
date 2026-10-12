+++
schema_version = 1
id = "base-107"
key = "paired-comprehension-layout"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Align explicit and implicit expanded comprehension layouts

## Outcome

Expanded comprehensions differ only by an index prefix immediately after the opening bracket.

## Context

Read brief.md Formatting direction and its paired examples. Start at generator_expression_contents in crates/zincite-fmt/src/lib.rs and tests/formatting.rs. Current output places p: with the body and leaves the pipe after it; the approved shape is [ p: followed by the body, then a leading pipe beside the generators. Completed base-103 compact conditional behavior remains applicable.

## Boundaries

- Layout only: preserve expression meaning, index expressions, comments, directives and configured indentation; permit removal of a trailing generator comma. Keep short fitting comprehensions compact.
- Do not copy the external feedback document into the repository.

## Done when

- [x] Scalar and tuple prefixes stay on the opening bracket line; the same body and generator layout is used without a prefix.
- [x] A let-in body stays at the let indentation; the generator separator leads its line and has no trailing generator comma.
- [x] Focused paired examples are idempotent, preserve comments and honor spaces/tabs and width.

## Validation

- Add a small paired scalar/tuple and let-body formatter regression set, reusing indentation/comment cases where possible.
- Compile a few complete representative before/after models to FlatZinc with Gecode without solving; compare outputs ignoring only invocation-path comments.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.

## Result

Expanded explicit and implicit comprehensions share body and generator layouts, with opening-line scalar/tuple prefixes and leading generator pipes.

Validation:

- Independent verification passed against Wbf90a561cc64f438; both post-review and pre-completion comparisons were equal.
- cargo fmt, clippy, workspace tests, zdev check and four Gecode compile-only before/after comparisons passed.
- Updated zincite-fmt and zincite were installed locally; both match all four reviewed formatter controls.
