+++
schema_version = 1
id = "base-134"
key = "compact-nested-layouts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Keep nested collections compact and reduce deep layout expansion

## Outcome

Nested arrays and expressions use readable compact layouts without unnecessary delimiter-only indentation.

## Context

The user requests [[[ expr ]]] for fitting nested arrays. The existing bench-save.py nested family expands 4,493 bytes at depth160 to609,316 bytes; depth40 expands1,133 to22,036. Both are stable, so idempotence is insufficient. Read formatter layout/lib, formatting tests, docs/formatting-style.md and the brief layout rules. First inspect rendered nested-array, nested-conditional and mixed examples before choosing a small layout change.

## Boundaries

- Formatting only: preserve dimensions, tokens, explicit parentheses and semantics. No arbitrary indentation cap or silent width waiver merely to make measurements pass.

## Done when

- [ ] Comment-free nested arrays whose full expression fits remain compact, including three levels; array rank and expression structure are preserved.
- [ ] Long inner expressions and comments use deliberate multiline layout with compact delimiter chains where readable; comments/directives are preserved.
- [ ] Compare and document before/after nested conditional and array examples at ordinary and stress depths, including output size and maximum width; eliminate avoidable expansion and explain retained structural indentation.
- [ ] Update the style gallery while retaining explicit/implicit comprehension correspondence, configured width/indentation and second-pass stability.

## Validation

- Use a few paired fitting/nonfitting nested-array and nested-conditional controls, including a comment boundary; inspect rendered output.
- Run existing formatter preservation and CLI controls. Measure the existing depth40/depth160 shapes and ordinary save cases outside overlapping measurement jobs; no full corpus sweep.
- Run the brief's required Rust validation for code changes; use existing focused controls and only the few behavior checks specified here.
- Run zdev check base --format json for changed records.
