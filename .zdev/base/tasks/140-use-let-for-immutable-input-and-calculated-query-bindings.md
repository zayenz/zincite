+++
schema_version = 1
id = "base-140"
key = "unified-let-bindings"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Use let for immutable input and calculated query bindings

## Outcome

Use immutable let bindings with native pipelines for input values and ordinary calculations, and emit the final expression without requiring report, retaining original-input edit safety.

## Context

The user rejected both the source/let distinction and literal("required") as the normal way to read a named assignment, and wants native Zirium-style pipelines and implicit final output. Backward compatibility is unnecessary. The concrete reporting example is `let required = items | filter(name("required")) | values | one; sum(required)` with an optional final semicolon. Use existing item/name predicates rather than inventing vars or a second predicate syntax. Read Zirium's query-language.md binding/statement sections and the pinned shared Zirql readers/evaluator. The native-only reader rejects the shared One value node; use the existing shared value-reader/preparation machinery, including when there are no function declarations. Project literal values with demanded model facts to retain declared axes and nominal identity. Input-before-calculation ordering is unnecessary because terminal edits alone create the candidate. Read docs/query-comprehension-contract.md for the remaining data restrictions. The documentation chat committed its work as 6bbf1bf and handed binding-related docs back; preserve that reader-focused content.

## Boundaries

- Use one binding keyword without compatibility aliases or deprecation machinery. Reuse shared native/value readers, cardinality, sequencing and evaluation; no second evaluator, new vars selector, generic algorithms, MiniZinc semantic analysis or unrelated Zirql expansion.
- Preserve immutable original-input reads, explicit terminal create/replace/emit behavior, model/include read-only ownership, aggregate budgets, whole-candidate validation and atomic writes. Let original-input read bindings and calculations interleave under ordinary scope. Edit input bindings may use the bounded existing item selection/filter and literal-value projection path with shared one; project, match, paths and report-only expression calls remain excluded from data programs. Keep the remaining closed data-expression policy.
- No semantic repairs or corpus sweeps. Preserve unrelated README/docs/site/highlighter work; coordinate overlapping docs rather than overwriting it.

## Done when

- [x] Program APIs and both CLI entry points run the native-pipeline example with a bare final expression, with or without its final semicolon and an unrelated unused function. It extracts exactly one literal RHS, preserves declared axes, nominal identity and completeness, and gives located failures for missing/duplicate matches or computed input. Source binding syntax is removed.
- [x] Input and calculated bindings interleave without changing original-snapshot reads. Data input pipelines retain bounded admission, remaining data-expression and explicit-effect restrictions; all rejected requests produce no partial output or write.
- [x] User-facing contract, help and current examples teach native input pipelines, let and implicit final output without requiring literal/query wrappers or report. Do not retain obsolete paths solely for backward compatibility. Record shared-versus-host ownership; no new evaluator is introduced.
- [x] A few focused existing-style public checks cover native extraction, model-backed keys, aggregate limits, statement/final-expression handling and edit acceptance/refusal. Current program tests/fixtures use let, required workspace checks pass and updated zincite/zincite-query commands are installed locally.

## Validation

- Run focused query library/CLI checks; add only behavior tests needed for changed binding and edit boundaries.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json after record changes; independently review integrated behavior, documentation and retained ownership.

## Result

Unified query bindings with let and native input pipelines. Final bare expressions emit results with an optional final semicolon. Removed source binding syntax, named-literal frontend and input-first ordering; shared Zirql One/preparation handles native values with or without functions, preserving model-backed keys and original-input edit safety. Updated help, guides and examples.

Validation:

- Independent whole-task review PASS against unchanged snapshot W044c6f6372e9f353; no findings or validation writes.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; focused query package, JavaScript syntax and staged/unstaged diff checks passed.
- Installed zincite and zincite-query locally with locked offline release builds. Both ran native selection plus sum(required) without report or final semicolon, returning complete integer 12 with no limitations or stderr; installed hashes match release artifacts.
- zdev check base passed. User-approved parking preserved the separate unfinished integration as /tmp/zincite-pending-query-integration.patch with receipt /tmp/zincite-pending-query-integration.json; that integration is excluded.
