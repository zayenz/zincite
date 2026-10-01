+++
schema_version = 1
id = "base-054"
key = "lint-catalogue"
area = "base"
status = "done"
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

- [x] Assign all sixteen existing rules to one of the five documented families; keep the existing IDs and default/thesis membership.
- [x] Expose metadata through the library, --list-rules and --explain RULE; unavailable rules and none/sometimes/always fix support are explicit.
- [x] Document each rule's purpose, limitations and options from the same catalogue; new rule tasks extend this catalogue.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a default rule, actual catalogue availability, family membership and unknown rule lookup through the public library/CLI; keep unavailable status explicit if an unavailable rule is later registered.

## Result

Expose all sixteen existing rules through one public catalogue and standalone --list-rules/--explain commands, with families, purposes, required facts, limitations, current availability, implemented fix support and options. Preserve existing IDs, presets and lint execution.

Validation:

- Independent Cargo fmt, all-target Clippy with -D warnings, workspace tests and zdev check pass.
- Independent listing and all sixteen explanations complete with stdin held open and no model loading; invalid/unknown inspection arguments exit 2. Current catalogue reports all rules available, fixes none and no options, and preserves ordinary lint behavior. Frozen source and foreign bytes/modes remain unchanged.
