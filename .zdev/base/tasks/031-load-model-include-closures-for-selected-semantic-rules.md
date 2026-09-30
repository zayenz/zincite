+++
schema_version = 1
id = "base-031"
key = "include-models"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-030", "base-025"]
+++
# Load model include closures for selected semantic rules

## Outcome

The linter has a reusable model context with file-aware source diagnostics and resolved include files.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. The current linter owns one ParsedFile and byte ranges without file identity. Implement root/include ownership and explicit -I/--stdlib-dir configuration only when selected rules require semantic analysis.

## Done when

- [ ] Load each explicit model root independently using the brief include order, detect cycles/missing files, and retain every source and its suppression context with diagnostic file identity.
- [ ] Classify standard-library versus user declarations from resolved files, support builtin declarations needed by selected analysis, and keep default single-file linting/formatting independent of library availability.
- [ ] Expose the model context to subsequent rule code with a simple library API; incomplete semantic roots report errors or analysis limitations rather than appearing fully checked.
- [ ] Implement the selected-analysis result and stderr/exit precedence defined in the brief: findings, per-rule execution/applicability/limitations, and actual errors remain distinguishable. Exercise a limitation-only result and an independent warning/error result.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use a small include graph to check search-path precedence, duplicate includes, missing/cyclic dependencies, source locations and continued processing of another root.
