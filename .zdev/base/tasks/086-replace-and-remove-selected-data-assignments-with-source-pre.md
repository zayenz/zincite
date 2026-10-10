+++
schema_version = 1
id = "base-086"
key = "data-assignment-edits"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-085"]
+++
# Replace and remove selected data assignments with source-preserving previews

## Outcome

Explicit queries can change selected data assignments and return or preview a complete validated candidate, with optional single-file replacement.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Data edits and structured values and its comment/directive rules. Existing crates/zincite-lint/src/fixes.rs and fix_files.rs provide snapshots, range/conflict checks, reparse checks, diff output and permission-preserving replacement. Reuse or minimally separate the lower-level edit operations for this second consumer; retain existing lint safety APIs and behavior.

## Boundaries

- Intentional instance edits are not safe lint fixes; do not enable lint rules or claim semantic equivalence.
- Use one transformation stage against the original parsed input, with document emission; no later evaluation against stale ranges, implicit include writes or multi-file transactions.

## Done when

- [x] set_value and remove act on selected assignments and emit_document returns the entire candidate; replacement RHS syntax and the full data candidate are validated.
- [x] Attached comments/directives on removed items are handled as specified; retained comments, source spelling, BOM and untouched line endings survive.
- [x] --diff and --write follow the single-file contract; default execution does not write files, and stale/invalid/conflicting candidates do not partially replace a file.
- [x] Edit validation is reusable without imposing lint safe/unsafe eligibility on intentional transformations; existing lint fix behavior remains unchanged.

## Validation

- Use a few public assignment-edit checks for a real replacement/removal, retained comments/BOM/CRLF, invalid replacement and stale or overlapping edits. Reuse existing file-replacement/fix tests for permissions and unchanged originals on failure.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Result

Implemented and independently reviewed source-preserving assignment replacement/removal, complete validated candidates, diff previews and explicit single-file writes. Shared raw edit and syntax-only replacement APIs retain existing lint safety behavior.

Validation:

- Independent review PASS at Wca83cbb4d7891a1f; comparison equal before completion.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all passed (313 tests).
- Focused public checks cover preserved BOM/CRLF/comments/directives, invalid and trailing-comment replacements, stale/overlapping edits, unchanged originals, diff/write parity and existing lint behavior.
