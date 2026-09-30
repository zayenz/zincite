+++
schema_version = 1
id = "base-053"
key = "parser-corpus-compatibility"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Accept the four remaining compiler-supported item and callable families

## Outcome

Deprecated omitted-function declarations, anonymous enum constructors, standalone equality items and callable annotation capture expose meaningful lossless syntax and work through formatter/lint traversal.

## Context

Read the area brief and scripts/syntax-corpus-checkpoint.md. scripts/syntax-gap-reproducers.json retains omitted_function, enum_constructor, standalone_equality and annotation_capture reductions, all accepted by MiniZinc 2.10.1. These families account for 19 rejected corpus instances. Reuse declaration, callable, assignment and expression parsing rather than opaque acceptance.

## Boundaries

- Support only the four demonstrated families, retaining written spelling, precise ranges, useful nodes and malformed-input recovery.
- Preserve default data-mode rejection of model declarations; anonymous enum constructor assignments must work in data mode.
- Do not admit the compiler-rejected int(...) call, malformed interpolation or corrupted source; do not add semantic rewriting or a parsing framework.
- Initial task-owned path allocation: ["README.md","crates/zincite-syntax/src/parser.rs","crates/zincite-syntax/src/lib.rs","crates/zincite-syntax/tests/parsing.rs","crates/zincite-fmt/src/lib.rs","crates/zincite-fmt/tests/formatting.rs","crates/zincite-lint/src/lib.rs","crates/zincite-lint/tests/linting.rs"]. Coordination may extend it only after checking retained parent edits and sibling assignments.

## Done when

- [ ] All four retained reductions and affected files pass parsing with meaningful declaration/expression/signature nodes and exact token/CST coverage.
- [ ] Formatting retains spelling and reaches stable output for focused new-form examples; naming and constraint-label traversal retain their existing contracts.
- [ ] Nearby invalid forms remain diagnosed, accepted extensions are documented accurately, and all affected records are reconciled without excluding existing formatter failures.

## Validation

- Run required workspace fmt, clippy with -D warnings and tests.
- Use a few combined public syntax/format/lint checks and original/formatted MiniZinc 2.10.1 model-check-only probes; no solver runs.
- Recheck affected corpus families read-only and retain remaining failures for the parent's final reconciliation.
