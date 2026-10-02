+++
schema_version = 1
id = "base-072"
key = "lint-fix-cli"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-055", "base-071"]
+++
# Preview and apply explicitly selected lint fixes

## Outcome

Users can preview fixes or apply eligible fixes to explicit files while ordinary linting stays read-only.

## Context

Read the brief Fixes contract. Reuse the formatter's safe write behaviour where practical without forcing a cross-crate abstraction; directory discovery must never imply permission to rewrite dependencies. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [x] Implement mutually exclusive --fix/--diff and explicit --unsafe-fixes, with configuration fixable/unfixable selection independent of lint selection.
- [x] Reject stdin/directories and invalid modes before writes; apply only to explicit regular files, preserve permissions and refuse symlinks, stale sources and parse-invalid candidates.
- [x] Apply one pass, reanalyse resulting sources and report remaining findings/errors with the specified exits; conflicts are reported and include-only/system files stay untouched.
- [x] Library clients can plan/apply in memory independently; CLI write failures leave affected originals intact and independent files proceed.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Use temporary files to verify preview, safe/unsafe eligibility, restrictions, conflict handling, a parse-invalid edit, stale source, permissions/symlink refusal and documented post-fix exits. Real producers land in the next task.

## Result

Added explicit lint fix and diff modes, independent fix restrictions, validated in-memory candidates and previews, and permission-preserving one-pass replacements followed by final analysis. Existing rules remain diagnostic-only.

Validation:

- Independent whole-task verification passed at W5e158f592bd780e0: Cargo fmt, workspace Clippy and all 183 workspace tests, zdev check and diff check. Focused public and native checks cover eligibility, modes, stale/invalid candidates, permissions, symlinks, conflicts, independent failures and final exits. Verified source and foreign-file snapshot comparisons remained equal.
