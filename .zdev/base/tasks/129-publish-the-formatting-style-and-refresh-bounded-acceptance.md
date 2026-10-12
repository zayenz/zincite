+++
schema_version = 1
id = "base-129"
key = "formatter-public-style"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Publish the formatting style and refresh bounded acceptance evidence

## Outcome

Users have a concrete style guide and current bounded evidence for stable source-preserving formatting.

## Context

Read the brief Formatting direction, docs/format-on-save.md, crates/zincite-fmt/tests/formatting.rs and the latest pinned scripts/save-performance.md result. Layout changed after that save pin, including explicit/implicit comprehension pairing. Add a public before/after style gallery and rerun existing representative checks on the current release.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Keep settled layout policies; no formatter redesign, new style configuration, full corpus sweep or semantic rewriting.
- Retain historical measurement records and report current results separately.

## Done when

- [x] The guide demonstrates let-in alignment, short quantified conditions, paired implicit/scalar/tuple comprehension layouts, matrices, comments, directives and relevant width/indent options.
- [x] Existing focused controls demonstrate reparsed structure, spelling/comment preservation and second-pass stability; compiler no-solve checks supplement only a few complete examples where useful.
- [x] Current named save cases retain their outputs/statuses and measured latency/memory evidence, with misses or unavailable evidence stated honestly.

## Validation

- Reuse existing formatting tests and representative save driver; do not add tests merely duplicating the gallery. Run required Rust checks if code changes.
- Run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.

## Result

Published a current formatting gallery and separate bounded save checkpoint; dense default latency miss remains open for a bounded follow-up.

Validation:

- Independent review passed all done conditions and writing guidance; gallery replay and 31 formatting plus 7 CLI controls pass.
- All 1400 timed samples and 28 native captures independently checked against table rows, complete outputs/statuses, hashes and RSS; default dense p95 111.014 ms honestly misses unchanged 100 ms gate.
- Links, diff checks and zdev check pass; review snapshot W101baf8187b773c2 compares equal; raw successful and failed-attempt evidence retained privately.
