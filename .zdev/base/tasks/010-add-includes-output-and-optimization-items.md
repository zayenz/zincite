+++
schema_version = 1
id = "base-010"
key = "model-items"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-003"]
+++
# Add includes, output and optimization items

## Outcome

Models can format includes, output expressions and all solve modes with their annotations intact.

## Context

Read the current top-level parser/formatter and pinned include, output and solve productions. Reuse existing expression and annotation parsing; later expression families extend these items without a separate grammar.

## Boundaries

- Do not load or sort includes in this task.
- Only already implemented expression forms are accepted in item bodies.

## Done when

- [ ] Include and output items and satisfy/minimize/maximize solve items parse and format.
- [ ] Output-specific annotations and solve annotations preserve original attachment and spelling.
- [ ] Typed item views distinguish includes, output and solve forms and expose their expressions and ranges.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a complete model with includes, annotated output and optimization; check tree roles and stable formatting with compiler acceptance. No separate test for every keyword.
