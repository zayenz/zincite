+++
schema_version = 1
id = "base-031"
key = "include-models"
area = "base"
status = "done"
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

- [x] Load each explicit model root independently using the brief include order, detect cycles/missing files, and retain every source and its suppression context with diagnostic file identity.
- [x] Classify standard-library versus user declarations from resolved files, support builtin declarations needed by selected analysis, and keep default single-file linting/formatting independent of library availability.
- [x] Expose the model context to subsequent rule code with a simple library API; incomplete semantic roots report errors or analysis limitations rather than appearing fully checked.
- [x] Implement the selected-analysis result and stderr/exit precedence defined in the brief: findings, per-rule execution/applicability/limitations, and actual errors remain distinguishable. Exercise a limitation-only result and an independent warning/error result.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use a small include graph to check search-path precedence, duplicate includes, missing/cyclic dependencies, source locations and continued processing of another root.

## Result

Added explicit model/include loading with file identities, source origins, local suppressions and located errors or limitations. Selected analysis separates findings, rule outcomes, limitations and errors; syntax defaults and unavailable thesis-rule gating remain intact.

Validation:

- Independent review passed at Wdaf2949592d053e8; cargo fmt, clippy with -D warnings and all 70 workspace tests passed.
- Focused include graphs verified resolution order, canonical deduplication, cycles, missing/unreadable files, independent roots, implicit core closure, source origins and BOM/UTF-8 locations.
- Shared reporting verified real interpolated-include limitations and 0/1/2 precedence; malformed include probes returned errors without panic or unintended loading.
