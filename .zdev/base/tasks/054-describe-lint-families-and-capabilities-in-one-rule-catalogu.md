+++
schema_version = 1
id = "base-054"
key = "lint-catalogue"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-030"]
+++
# Describe lint families and capabilities in one rule catalogue

## Outcome

Users and library callers can inspect stable rule IDs, families, semantic requirements and conditional fix support.

## Context

Start with rules.rs, Rule::id, is_available and requires_model, and the CLI help. Existing rules and thesis membership already exist; extend their metadata rather than add a second registry. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Assign all sixteen existing rules to one of the five documented families; keep the existing IDs and default/thesis membership.
- [ ] Expose metadata through the library, --list-rules and --explain RULE; unavailable rules and none/sometimes/always fix support are explicit.
- [ ] Document each rule's purpose, limitations and options from the same catalogue; new rule tasks extend this catalogue.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a default rule, a registered unavailable thesis rule, family membership and unknown rule lookup through the public library/CLI.
