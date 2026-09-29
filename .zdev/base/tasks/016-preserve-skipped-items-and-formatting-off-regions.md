+++
schema_version = 1
id = "base-016"
key = "format-directives"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-002"]
+++
# Preserve skipped items and formatting-off regions

## Outcome

Formatting directives preserve item/region bytes and reject invalid marker structure.

## Context

Read brief Formatting direction and Remaining layout defaults, then top-level item spans and comment traversal. Apply directives before emitting formatted output, retaining their protected source ranges independently of layout.

## Boundaries

- Skipped source is still syntax checked, and unsupported syntax remains an error until implemented.
- Includes are not sorted in this task; expose enough item/span information for include-sorting to respect barriers.

## Done when

- [ ] Standalone skip/off/on directives preserve complete items/regions and their boundary whitespace exactly.
- [ ] Nested, unmatched, inside-expression and dangling markers fail the whole file without partial output.
- [ ] Ordinary surrounding formatting does not consume, duplicate or alter protected bytes, including their original line endings.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a skipped item and off/on region with distinctive spacing and line endings, plus representative invalid markers. Verify exact protected bytes and second-pass stability.
