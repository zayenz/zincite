+++
schema_version = 1
id = "base-015"
key = "matrix-layout"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-014"]
+++
# Align and wrap two-dimensional literals

## Outcome

Two-dimensional literals use aligned columns and common row breakpoints.

## Context

Read the current matrix syntax/formatter and brief Formatting direction. Ordinary width measurement is already available from width-layout; implement the matrix-specific alignment policy using that behavior.

## Boundaries

- Keep logical row boundaries, cell order, comments and cell spelling; no alignment of unrelated source constructs.
- An indivisible cell may overrun the width limit without warning or failure.

## Done when

- [ ] 2D literals format one logical row per line with aligned columns.
- [ ] Rows that exceed the target width wrap at the same column boundaries across rows while preserving alignment and logical row grouping.
- [ ] Comments and uneven cell widths do not corrupt the literal; the second formatting pass is unchanged.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use one uneven-width matrix and a narrow-width version requiring shared breaks, plus a preserved comment or indivisible cell. Check valid reparse, logical rows and idempotence.
