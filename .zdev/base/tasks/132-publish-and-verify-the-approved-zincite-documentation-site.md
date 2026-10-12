+++
schema_version = 1
id = "base-132"
key = "publish-zincite-site"
area = "base"
status = "open"
complexity = "standard"
priority = "low"
blocked_by = ["base-131"]
+++
# Publish and verify the approved Zincite documentation site

## Outcome

The reviewed site is published at the chosen canonical URL with verified navigation and an operational update procedure.

## Context

Late attended gate: choose zincite.zayenz.se or zayenz.se/zincite, deployment host and authorized credentials/DNS access before any external mutation. Use the committed site from astro-doc-site and reconcile semantic support claims with the actual committed implementation; unfinished semantic work must remain labeled as future work. Hosting setup is not a prerequisite for local site work.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Use the explicitly approved host and URL; no unrelated personal-site redesign, account changes or paid services without authority.
- Preserve a recoverable prior deployment and record redirects/canonical behavior when an alternate URL exists.

## Done when

- [ ] Canonical URL, host and necessary publication authority are recorded before deployment.
- [ ] The deployed landing/docs/search/assets work over HTTPS with correct base paths, canonical metadata, sitemap and any approved redirects.
- [ ] Mobile/keyboard smoke checks and representative copyable commands match the shipped version; support/performance claims match current evidence.
- [ ] The update, verification and rollback procedure is documented without storing secrets.

## Validation

- Verify the actual deployed URL and links/assets/search, canonical metadata and sitemap; retain publication evidence and relevant limitations.
- Run site build before publication, git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.
