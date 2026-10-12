+++
schema_version = 1
id = "base-124"
key = "feature-cookbook"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Document reproducible MiniZinc feature-extraction recipes

## Outcome

Users can extract useful written-model and literal-data features with existing commands and understand what each result counts.

## Context

The query consumer already supports count, call_names, annotation_names, tally, types, instantiations and literal inspection. Begin with docs/reference.md Query library, crates/zincite-query/tests/queries.rs and scripts/query-acceptance.md. Add a compact public cookbook linked from README.md; source facts and uncertainty remain governed by the brief.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Use current query syntax; no new stages, semantic proofs, private inputs or corpus sweep.

## Done when

- [x] Recipes demonstrate constraint counts, call and annotation histograms, decision declaration types and array/matrix literal shape inspection on small public inputs.
- [x] Each recipe specifies root/include ownership, duplicate handling, source counts versus flattened or executed counts, and incomplete semantic results where relevant.
- [x] Both query command forms reproduce the documented outputs and preserve input bytes.

## Validation

- Run every recipe through zincite query and zincite-query; use the installed MiniZinc library only for semantic recipes. No new Rust tests are expected.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace when Rust changes; always run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.

## Result

Added a public feature-extraction cookbook with precise ownership, duplicate and uncertainty rules.

Validation:

- Independent review passed: all twelve documented commands reproduce expected output with both command forms; all three public input hashes unchanged.
- README and cookbook ownership reviewed; both writing skills and all done conditions satisfied; git diff checks pass and review snapshot W5053c2970404ab7c compares equal.
