+++
schema_version = 1
id = "base-019"
key = "editorconfig"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-018", "base-015"]
+++
# Resolve EditorConfig and explicit formatter overrides

## Outcome

Formatting obeys resolved file configuration and explicit CLI overrides while retaining protected source.

## Context

Read brief Language and command-line contract and the EditorConfig specification. Extend the existing CLI and formatter options, preferably using a suitable existing EditorConfig implementation. Width and file modes already have their own behavior checks.

## Boundaries

- No custom configuration framework or extra style knobs.
- Preserved comments, literal contents and skipped spans take precedence over whitespace/newline settings.

## Done when

- [x] Parent lookup, section precedence and unset resolve supported EditorConfig properties; explicit CLI overrides win and stdin-filepath supplies lookup context.
- [x] Indentation, width/off, line endings, final newline, trimming and UTF-8/BOM behavior follow the brief; unsupported charset and invalid supported values fail clearly.
- [x] Check and write use the same resolved settings, and help documents properties, option values and pre-stable formatting policy.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use one nested EditorConfig fixture with precedence/unset and an override; check stdin context and protected text under newline/trimming changes.
- Verify check/write consistency, a BOM/charset case and invalid supported configuration without expanding into a property-combination matrix.

## Result

Resolved EditorConfig and explicit CLI layout overrides with protected-text-aware newlines, trimming and UTF-8 BOM handling.

Validation:

- Independent PASS on Wf1d9303d663e0d51; precedence/unset, overrides, value-case and empty-value fixes, protected bytes, BOM, stdin and check/write consistency verified.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all pass (48 tests).
