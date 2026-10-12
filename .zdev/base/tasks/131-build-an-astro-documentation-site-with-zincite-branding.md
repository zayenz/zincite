+++
schema_version = 1
id = "base-131"
key = "astro-doc-site"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-130"]
+++
# Build an Astro documentation site with Zincite branding

## Outcome

A repository-local static homepage and documentation site presents Zincite clearly and remains deployable at either proposed URL.

## Context

Use Astro with Starlight as the initial documentation shell. Inspect current zayenz.se conventions and its available source before styling; adapt typography/navigation and Zincite logo/orange accent rather than copying an unrelated theme. Reuse authoritative docs from reader-documentation and assets/ logos. This user-approved follow-up extends the earlier hosted-site boundary, but publication remains a separate attended task.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Static documentation only; no backend, accounts, analytics service, browser MiniZinc runtime or playground.
- Keep canonical site URL and base path configurable until publication; do not change DNS or deploy.
- Do not duplicate maintained Markdown content or claim unfinished semantic work is shipped.

## Done when

- [ ] A landing page explains fmt, lint and query with working installation/examples and links to the documentation.
- [ ] Docs have clear navigation, working search, MiniZinc code highlighting, light/dark themes and Zincite branding consistent with zayenz.se.
- [ ] Keyboard focus, contrast, mobile navigation and readable code/tables pass focused visual checks.
- [ ] Documented clean build/preview commands work; root and /zincite base-path builds retain valid links/assets/canonical configuration.
- [ ] Documentation ownership and build integration are recorded; the repository remains usable for Rust-only contributors.

## Validation

- Run site build and available type/content checks, inspect desktop/mobile and keyboard navigation, and validate local links/search/assets under both base paths. No broad browser test suite.
- Run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.
