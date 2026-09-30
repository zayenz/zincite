+++
schema_version = 1
id = "base-029"
key = "recursive-inputs"
area = "base"
status = "open"
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

- [ ] Both commands discover and deduplicate recursive .mzn/.dzn inputs, including ignored source files, with no directory-symlink loops; report inaccessible inputs and continue independent files.
- [ ] Formatter directories require --check; reject directory stdout/write modes. Preserve explicit-file write and stdin contracts, diagnostics and status precedence.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use one small temporary tree to exercise overlapping inputs, an invalid file, hidden/ignored source and a symlink directory; check that no files change.
