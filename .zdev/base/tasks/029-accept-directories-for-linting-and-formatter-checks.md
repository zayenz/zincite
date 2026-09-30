+++
schema_version = 1
id = "base-029"
key = "recursive-inputs"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Accept directories for linting and formatter checks

## Outcome

Users can lint or check formatting across a model tree without constructing shell argument lists.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Both command mains currently accept only explicit files/stdin. Add the deterministic read-only directory behavior specified in the brief, keeping domain behavior in the libraries.

## Done when

- [x] Both commands discover and deduplicate recursive .mzn/.dzn inputs, including ignored source files, with no directory-symlink loops; report inaccessible inputs and continue independent files.
- [x] Formatter directories require --check; reject directory stdout/write modes. Preserve explicit-file write and stdin contracts, diagnostics and status precedence.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use one small temporary tree to exercise overlapping inputs, an invalid file, hidden/ignored source and a symlink directory; check that no files change.

## Result

Added deterministic shared directory discovery to lint and formatter check mode, preserving stdin and explicit-file write behavior.

Validation:

- Workspace fmt, clippy with -D warnings, all 63 tests and diff check passed.
- Focused CLI trees cover overlapping inputs, hidden and ignored source, Git metadata, invalid and missing inputs, data mode, symlink cycles, deterministic diagnostics and unchanged bytes.
- Independent permission probes confirm error status 2 with continued processing; explicit real-file and symlink writes retain symlink refusal.
- Independent whole-task PASS and exact snapshot comparison W65bde65d0053c221.
