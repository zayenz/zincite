+++
schema_version = 1
id = "base-013"
key = "data-mode"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-006", "base-007", "base-008", "base-009", "base-010", "base-011", "base-012"]
+++
# Select data-file mode and check grammar integration

## Outcome

Model and data inputs use the complete planned grammar with the correct file-mode restrictions.

## Context

Read the supported-syntax notes from the named grammar tasks, the pinned full grammar and brief Language and command-line contract. Add .dzn mode selection/restrictions and exercise combinations of the delivered syntax families. Grammar ownership is explicit in those task completion conditions; this task verifies the integration rather than supplying missing families.

## Boundaries

- Do not resolve includes, require a solve item, evaluate data expressions or perform type checking.
- If a previously unassigned grammar family is found, report the exact production for a focused follow-up and leave full-coverage acceptance incomplete; do not absorb it as unbounded implementation work.

## Done when

- [x] .mzn/.dzn paths and --stdin-filepath select model/data mode as specified; unsupported data-file item forms produce diagnostics.
- [x] Cross-family examples use the delivered parser/formatter paths correctly. A concise comparison with the pinned grammar verifies those claims; do not remove exclusions or claim complete compatibility if an omission is found.
- [x] Representative models and model/data pairs traverse parse and format successfully; current compatibility and syntax-only limitations are documented.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use one formatted model/data pair, invalid top-level data input, stdin-path mode selection and one cross-family model. Compare relevant parsed structure and source preservation after formatting.
- Run MiniZinc 2.10.1 acceptance on the valid model/data pair and cross-family model; retain only a concise grammar coverage note, not a test per production.

## Result

Added public model/data modes, path and stdin-path selection, assignment-only data diagnostics and concise cross-family grammar integration evidence.

Validation:

- Independent verifier PASS against W80affc5c71d39169; unchanged snapshot.
- Cargo fmt, Clippy with denied warnings and all 37 behavior tests passed.
- Mode/CLI selection, invalid data diagnostics, exact coverage, structure, comments and idempotence passed.
- MiniZinc 2.10.1 accepted original and independently formatted model/data pairs and cross-family models.
