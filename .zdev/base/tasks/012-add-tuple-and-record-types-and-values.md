+++
schema_version = 1
id = "base-012"
key = "structured-values"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-004"]
+++
# Add tuple and record types and values

## Outcome

Tuple and record types, literals and access expressions are parsed and formatted with field identity intact.

## Context

Read the pinned MiniZinc 2.10.1 grammar for this family and the current supported-syntax notes. Extend the existing parser, typed traversal and formatter together; update those notes with the exact support delivered. Extend shared type-inst and expression parsing; inspect the pinned grammar for structured forms, including field/tuple access. Implement only forms defined by the pinned grammar; a reserved keyword alone does not add a feature.

## Boundaries

- No field/type resolution or representation separate from the CST.
- Preserve field and tuple order; do not sort or rename fields.

## Done when

- [ ] Every tuple/record type, literal and access form defined by the pinned grammar parses and formats, including nested forms and required variant forms.
- [ ] Record-field bindings and access expressions retain precise ranges and distinct roles so naming checks do not flag references as declarations.
- [ ] Structured types work wherever the existing shared type parser is used, including parameters and local declarations once those consumers exist.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a nested tuple/record declaration and value with field access and comments; check tree roles, coverage and idempotence. Check representative valid structured syntax with MiniZinc 2.10.1.
