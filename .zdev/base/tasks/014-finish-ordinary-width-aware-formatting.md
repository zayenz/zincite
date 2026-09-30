+++
schema_version = 1
id = "base-014"
key = "width-layout"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-013"]
+++
# Finish ordinary width-aware formatting

## Outcome

All non-matrix syntax follows consistent width, indentation and line-breaking rules.

## Context

Read brief Formatting direction and Remaining layout defaults, plus ../background/formatting.md. Syntax slices already supply valid layouts. Finish common width accounting and breaks across those forms through the formatter options API; the CLI/configuration tasks expose options later.

## Boundaries

- Aligned matrix columns and synchronized row wrapping belong to matrix-layout.
- No extra style settings, alignment of adjacent declarations, synthetic operands or semantic rewrites.

## Done when

- [x] The formatter options support indentation style/size, tab width and finite/unlimited line width; defaults and tab/Unicode measurement match the brief.
- [x] Binary chains, annotations, labelled constraints, nested expressions and lists break without changing precedence, comment attachment or explicit expansion choices.
- [x] Trailing commas, blank-line groups and unavoidable overruns follow the brief and remain stable on a second pass.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a small set of narrow/default-width examples combining annotations, binary chains and explicit multiline lists, including a tab/Unicode case; check reparsed structure and idempotence. Reuse generator tests instead of multiplying layout combinations.

## Result

Added configurable ordinary width-aware formatting while preserving precedence, comments, explicit expansion and blank groups.

Validation:

- Independent verification PASS on W1da0e80838d6ed8b; focused layout, suffix, callable and blank-group regressions preserve structure and idempotence.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all pass (39 integration tests).
