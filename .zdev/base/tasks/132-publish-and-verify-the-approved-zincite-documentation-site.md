+++
schema_version = 1
id = "base-132"
key = "publish-zincite-site"
area = "base"
status = "done"
complexity = "standard"
priority = "low"
blocked_by = ["base-131"]
+++
# Publish and verify the approved Zincite documentation site

## Outcome

The reviewed site is published at the chosen canonical URL with verified navigation and an operational update procedure.

## Context

Late attended gate: choose zincite.zayenz.se or zayenz.se/zincite, deployment host and authorized credentials/DNS access before any external mutation. Use the committed site from astro-doc-site and reconcile semantic support claims with the actual committed implementation; unfinished semantic work must remain labeled as future work. Hosting setup is not a prerequisite for local site work.

The user selected https://zayenz.se/zincite/ and directed deployment discovery through the web-infra and zayenz.se projects. The existing approved publication route is the personal site's Git-based Netlify project zayenz-web (36755581-737f-42cb-ac2f-a16d37fcd00f), production branch main, with existing authenticated access. No DNS, account, environment or paid-service change is needed. Prepare the static import from the current production source rather than including unrelated unpublished personal-site commits. Record the recoverable prior deploy and reviewed source candidate before the authorized publication.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

## Boundaries

- Use the explicitly approved host and URL; no unrelated personal-site redesign, account changes or paid services without authority.
- Preserve a recoverable prior deployment and record redirects/canonical behavior when an alternate URL exists.

## Done when

- [x] Canonical URL, host and necessary publication authority are recorded before deployment.
- [x] The deployed landing/docs/search/assets work over HTTPS with correct base paths, canonical metadata, sitemap and any approved redirects.
- [x] Mobile/keyboard smoke checks and representative copyable commands match the shipped version; support/performance claims match current evidence.
- [x] The update, verification and rollback procedure is documented without storing secrets.

## Validation

- Verify the actual deployed URL and links/assets/search, canonical metadata and sitemap; retain publication evidence and relevant limitations.
- Run site build before publication, git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.

## Result

Published https://zayenz.se/zincite/ through existing Netlify Git deployment 6acd19408cf0713fa404653e from personal-site commit 7ab46199c1511ccc95172b61de41cbed87e73414 (Zincite source77e5a91). Preserved prior deploy and unrelated unpublished personal-site commits; documented update and Git-revert recovery.

Validation:

- Fresh whole-task PASS Wff00a8d819b60359: all57 hosted files exact,12 canonicals/sitemap,377 links/anchors,actual search/result navigation,mobile/menu/local scroll,keyboard/focus/Escape,clipboard commands and installed CLI parity. Personal root/About and prior immutable deploy verified. Source sitecheck/build and Node24 releasecheck pass; diff/zdev checks pass. Generated Pagefind template-string whitespace retained with independently reviewed exact-file exception. Chromium desktop/mobile-width smoke; rollback inspected, not executed.
