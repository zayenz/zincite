+++
schema_version = 1
id = "base-011"
key = "enums-aliases"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-004"]
+++
# Add enum definitions and type aliases

## Outcome

Enums, constructors and type aliases participate in declarations and expressions through the shared syntax.

## Context

Read the pinned MiniZinc 2.10.1 grammar for this family and the current supported-syntax notes. Extend the existing parser, typed traversal and formatter together; update those notes with the exact support delivered. Read declaration/type-inst parsing and ordinary call expressions. Add the pinned enum/type-synonym productions, including constructor forms and their declaration identifiers.

## Boundaries

- Do not resolve aliases, enumerate values or infer the role of references.
- Alias targets use currently supported types; structured-values extends the shared type parser separately.

## Done when

- [x] Enum definitions, anonymous and named constructor forms, enum combinations and type aliases parse and format with their annotations and original spelling.
- [x] Enum/type names and constructor declaration names have precise typed views for naming checks.
- [x] Named enum/type references work syntactically in declarations and expressions without semantic resolution.
- [x] Type-inst concatenation is supported by the shared type parser wherever the pinned grammar permits it.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use compact enum/constructor and alias examples including an expanded list with comments; check named-role traversal and format stability, with compiler acceptance for a complete model.

## Result

Added enum definitions/constructors/combinations, annotated type aliases, precise declaration-name roles and shared type-inst concatenation.

Validation:

- Independent verifier PASS against Wa97dfc670b46ed12; unchanged snapshot.
- Cargo fmt, Clippy with denied warnings and all 31 integration tests passed.
- Exact coverage/ranges, typed name roles, concat contexts, recovery, comments/parentheses and idempotence passed.
- MiniZinc 2.10.1 accepted original and independently formatted complete enum/alias fixture; concat examples establish syntax support only.
