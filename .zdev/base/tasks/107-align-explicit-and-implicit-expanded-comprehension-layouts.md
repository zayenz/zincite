+++
schema_version = 1
id = "base-107"
key = "paired-comprehension-layout"
area = "base"
status = "open"
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

- [ ] Scalar and tuple prefixes stay on the opening bracket line; the same body and generator layout is used without a prefix.
- [ ] A let-in body stays at the let indentation; the generator separator leads its line and has no trailing generator comma.
- [ ] Focused paired examples are idempotent, preserve comments and honor spaces/tabs and width.

## Validation

- Add a small paired scalar/tuple and let-body formatter regression set, reusing indentation/comment cases where possible.
- Compile a few complete representative before/after models to FlatZinc with Gecode without solving; compare outputs ignoring only invocation-path comments.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
