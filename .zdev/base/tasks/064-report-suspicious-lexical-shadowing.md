+++
schema_version = 1
id = "base-064"
key = "lint-shadowing"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-056", "base-032"]
+++
# Report suspicious lexical shadowing

## Outcome

suspicious-shadowing reports inner declarations hiding relevant outer bindings and honours an explicit name allowlist.

## Context

Reuse base-032 declaration identities and scope/reference tables. This task needs no new numeric, global-model or fix machinery. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Deliver suspicious-shadowing in family suspicious with inner and outer locations for parameters, lets and generators when an outer binding is visible.
- [ ] Do not warn about names reused in disjoint scopes, field labels, anonymous/intentionally unused bindings or configured ignore-names.
- [ ] Exercise library and CLI settings, preset overrides and next-item suppression without automatic renaming.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a captured outer decision hidden by a local, nested generator shadowing, disjoint loop reuse and a configured exception.
