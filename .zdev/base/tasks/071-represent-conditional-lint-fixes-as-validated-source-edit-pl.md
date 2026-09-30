+++
schema_version = 1
id = "base-071"
key = "lint-fix-plans"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054"]
+++
# Represent conditional lint fixes as validated source edit plans

## Outcome

Findings can expose optional titled safe/unsafe fixes without mutating files.

## Context

Extend FileFinding/LintDiagnostic carefully and reuse existing SourceLocation ranges, BOM offsets and syntax source ownership. Keep the planner independent of CLI writes and of semantic interpretation. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Represent optional fix title, safety and atomic edit group tied to exact original bytes; advertise per-rule fix capability separately.
- [ ] Validate UTF-8 boundaries and edit ranges, reject stale source, omit conflicting fix groups deterministically and preserve independent groups.
- [ ] Return complete candidate source for reparse validation, preserving untouched comments/line endings; suppressed findings produce no edits.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Exercise the public edit API with a real source snippet, BOM/non-ASCII ranges, overlapping groups, stale content and preserved comments; avoid implementation-mirroring unit tests.
