+++
schema_version = 1
id = "base-125"
key = "literal-mapping-contract"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Specify shared comprehension syntax for reports and structured data

## Outcome

A concrete bounded comprehension contract serves report construction and explicit creation or replacement of structured instance data.

## Context

set_value inserts expression source but does not evaluate it; values and filter_elements operate on literal data, with key and rectangularity checks. Read crates/zincite-query/src/structured.rs, language.rs and docs/reference.md Assignment edits. The user requires MiniZinc-style comprehension syntax for reports as well as creating or replacing data, including a useful way to double a structured instance. Specify a shared bounded contract over query results and literal data before either consumer implements it. Start with structured.rs, language.rs, lib.rs and existing report/data result types. Propose exact grammar and supported scalar operations.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

The user confirmed that doubling means duplicating entities and their dependent data with explicit index/reference mapping.

## Boundaries

- Investigation and documentation only; no production evaluator or dependency. Exclude model calls, decision values, recursive evaluation and solver execution.
- Keep private inputs outside the repository. Do not infer approval for the implementation contract from completion of this investigation.

## Done when

- [x] The contract defines shared MiniZinc-style comprehension syntax, generators, binding/index scope, supported scalar operations, numeric failures and aggregate limits for report and data consumers.
- [x] Report examples construct arrays or keyed structures from query results; data examples create new assignments and replace existing structured values without confusing source insertion with evaluated construction.
- [x] A complete public double-instance example expands a structured collection and its dependent arrays, sets or records with explicit index/reference mapping; it distinguishes this operation from merely multiplying numeric values.
- [x] The contract states positional/keyed/multidimensional output shape, rectangularity, comment policy, identifier/member representation, uncertain input handling and refusal behavior.
- [x] Recommend one shared bounded comprehension implementation using existing syntax/literal/query machinery where sufficient; exclude a general MiniZinc evaluator and record concrete user choices for the data implementation approval gate.

## Validation

- Check examples against current parser/edit behavior and current MiniZinc primary documentation; no new product tests or benchmark campaign.
- Run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.

## Result

Specified a proposed shared bounded comprehension contract with report/data adapters, coherent doubled-instance mappings and concrete data approval choices; no implementation approval granted.

Validation:

- Fresh independent review passed all five done conditions and both writing guides; original and doubled public instances accepted by MiniZinc instance-only checks and twelve expression bodies parsed by Zincite.
- Confirmed compiler string-set gap corrected with located common-subset refusal; grammar, scopes, numeric errors, limits, identifiers, shape, comments and coherent candidate axis obligations reviewed.
- Git diff checks and zdev check pass; corrected review snapshot W7eb81fb71c57614f compares equal; private evidence retained in /tmp/zincite125-validation.txt.
