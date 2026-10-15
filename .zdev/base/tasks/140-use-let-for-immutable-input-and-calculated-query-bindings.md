+++
schema_version = 1
id = "base-140"
key = "unified-let-bindings"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Use let for immutable input and calculated query bindings

## Outcome

Make let the normal binding spelling for literal/query inputs and calculated values while retaining source compatibility and original-input edit safety.

## Context

The user approved removing the reader-facing source/let distinction. Zincite program.rs currently recognizes literal(...) and native query wrappers based on is_source, although both spellings lower to shared zirql::Statement::Bind. Inspect /Users/zayenz/projects/zirium/docs/query-language.md (program bindings) and Zirql program/expression APIs, plus the exact pinned Zirql source. Reuse shared immutable sequencing and budgets; no dependency change is expected. Read docs/query-comprehension-contract.md for report/data restrictions. Coordinate documentation changes with the Improve Zincite docs site chat, whose current uncommitted docs/site edits must be preserved.

## Boundaries

- Keep existing source programs compatible. Do not introduce a second evaluator or expand general language features, semantic analysis, pipelines or Zirql scope.
- Preserve immutable original-input reads, explicit create/replace/emit behavior, model/include read-only ownership, aggregate budgets, whole-candidate validation and atomic writes. In edit requests, classify input versus calculated bindings by expression so input bindings still precede calculations regardless of keyword; do not admit report-only expressions through let input wrappers.
- No semantic repairs or corpus sweeps. Preserve unrelated README/docs/site/highlighter work; coordinate overlapping docs rather than overwriting it.

## Done when

- [ ] Program APIs and both CLI entry points accept let bindings for literal(...) and query(...) input expressions and ordinary calculations, with equivalent values, native identities, output/completeness and errors to existing supported forms. Existing source syntax remains accepted.
- [ ] Report/edit parsing uses expression role rather than binding keyword for input reads. Edit programs retain original-snapshot semantics and their ordering, expression-whitelist and explicit-effect restrictions; rejected requests produce no partial output/write.
- [ ] User-facing contract, help and representative input/calculation examples teach let as normal and explain source compatibility without claiming broader expression support. Inspect Zirium/shared Zirql and record why this change belongs in the host frontend.
- [ ] A few focused existing-style public behavior checks cover let/alias equivalence, calculated binding use and edit acceptance/refusal; required workspace checks pass and the updated zincite/zincite-query commands are installed locally.

## Validation

- Run focused query library/CLI checks; add only behavior tests needed for changed binding and edit boundaries.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json after record changes; independently review integrated behavior, documentation and retained ownership.
