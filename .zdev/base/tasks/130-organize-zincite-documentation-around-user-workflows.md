+++
schema_version = 1
id = "base-130"
key = "reader-documentation"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Organize Zincite documentation around user workflows

## Outcome

Installation, formatting, linting, queries, data edits and support limits are easy to find without duplicating authoritative content.

## Context

README.md and docs/reference.md contain usable but long combined guides. Plan separate getting-started, task guides, recipes, CLI/library references and support/uncertainty navigation. Preserve docs/semantic-processing-plan.md and historical script reports as developer/evidence material. The semantic queue is active: do not claim base-118 through base-123 complete before their results exist.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- No source/API redesign or unsupported coverage/performance claims. Keep one authoritative copy of each user-facing rule/contract.
- Do not rewrite completed task records or historical acceptance evidence.

## Done when

- [x] README navigation leads to installation, formatter/editor setup, lint/fix categories, query and data transformation guides, library reference and support limits.
- [x] Reader-facing pages distinguish implemented behavior, supported Unknown, explicit limitations, input failures and future work using current evidence.
- [x] Commands and examples are reproducible, relative links resolve, and moved material has preserved links or a clear replacement path.
- [x] The structure can feed an Astro site without a second independently maintained documentation copy.

## Validation

- Check internal links and execute a small selection of current command examples; no new Rust tests expected.
- Run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.

## Result

Organized README and Markdown workflow pages with direct paths to existing contracts, recipes, support boundaries and library references.

Validation:

- Independent verifier PASS at Wcdaad9b3413b326b; 19 installed-command checks, 94 links and anchors, byte-preserving previews and scoped explicit writes passed.
- git diff --check and git diff --cached --check passed; documentation-only change.
