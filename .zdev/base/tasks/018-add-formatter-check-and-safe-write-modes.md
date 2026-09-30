+++
schema_version = 1
id = "base-018"
key = "formatter-file-modes"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-013", "base-017"]
+++
# Add formatter check and safe write modes

## Outcome

Users can check or rewrite explicit files with predictable statuses and per-file failure isolation.

## Context

Read brief Language and command-line contract in full, then the current formatter CLI. Configuration remains at defaults in this slice; add --check/--write and multi-file handling around the existing complete-source formatter.

## Boundaries

- No EditorConfig in this task, directory discovery, include traversal or publication.
- Do not truncate or overwrite a failing file; reject writes through symlinks.

## Done when

- [x] Stdout/check/write modes, input restrictions and exit-code precedence follow the brief; independent valid files still process when another file fails.
- [x] Check writes nothing; write formats before replacement, preserves permissions and avoids truncation on failure.
- [x] Stdin/path usage and mode errors have clear diagnostics and help; basic local installation and usage are documented.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use temporary files to check clean/changed/error statuses, unchanged bytes on failed formatting, mixed valid/invalid inputs, retained permissions and symlink refusal. Reuse existing syntax/directive errors as inputs.

## Result

Added check/write and multi-file modes with error precedence, complete-source safe replacement, retained permissions and symlink write refusal.

Validation:

- Independent PASS on Wf29a05fb9ecc3bf7; statuses, failure isolation, unchanged bytes, permissions, symlink handling and subsequent clean checks verified.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all pass (46 tests).
