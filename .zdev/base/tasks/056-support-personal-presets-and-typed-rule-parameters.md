+++
schema_version = 1
id = "base-056"
key = "lint-presets-options"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-055"]
+++
# Support personal presets and typed rule parameters

## Outcome

Projects can save a named rule selection and its options, then override options explicitly without ambiguous inheritance.

## Context

Use the new settings resolver. The brief defines flat custom presets and the two concrete parameters; register their metadata here even if the corresponding rule bodies are still unavailable. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Implement lint.presets.NAME, one-custom-preset selection and defaults/preset/root-option precedence; reject reserved names, nested custom presets and conflicting custom selections.
- [ ] Add typed suspicious-shadowing.ignore-names and expensive-comprehension.max-candidates settings with documented defaults and range checks, including disabled rules.
- [ ] Round-trip effective selections/options through --show-settings and explicit library settings; document one personal preset combining families, exclusions and options.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check one composed preset, an explicit option override, invalid threshold/type, reserved/nested preset and two conflicting custom presets; do not add a generic options DSL.
