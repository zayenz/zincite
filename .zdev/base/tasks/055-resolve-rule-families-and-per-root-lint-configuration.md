+++
schema_version = 1
id = "base-055"
key = "lint-config"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-054"]
+++
# Resolve rule families and per-root lint configuration

## Outcome

Library and CLI resolve exact rules, families and built-in presets with deterministic configuration precedence.

## Context

LintOptions::from_selection currently accepts one preset or comma-separated IDs; main.rs owns CLI parsing and model roots. Add zincite.toml discovery and explicit settings without changing formatter EditorConfig. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Implement select/extend-select/ignore expansion, existing --rules compatibility and CLI replacement semantics; unavailable rules remain errors.
- [ ] Implement nearest per-root configuration, --config, --isolated and stdin-path lookup; root settings apply to its include closure and libraries receive explicit settings.
- [ ] Expose --show-settings with expanded IDs and settings paths; reject invalid selectors/settings before processing or writing.
- [ ] Existing exact next-item suppressions and plain lint exit statuses remain unchanged.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Use a compact nested-directory example covering nearest-file precedence, CLI override, stdin discovery and unknown settings; check family inclusion and exclusions.
