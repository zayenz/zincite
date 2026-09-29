+++
schema_version = 1
id = "base-009"
key = "callable-declarations"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-005"]
+++
# Add callable and annotation declarations

## Outcome

Functions, predicates, tests and annotation declarations can be parsed and formatted with complete signature syntax.

## Context

Read the pinned callable/parameter and type-inst grammar, then current declaration/type parsing. Add callable signatures and optional bodies, generic type-inst variables and parameter defaults. Reuse the existing expression parser; later expression families automatically extend callable bodies.

## Boundaries

- Parameter grammar excludes set cardinalities and dependent array indices where specified, even though those forms are legal in ordinary declarations.
- No name or type resolution; enum/alias and structured type syntax is supplied by its own tasks.

## Done when

- [x] Function, predicate, test and annotation declarations parse and format with parameters, defaults, declaration annotations and optional bodies.
- [x] Generic type-inst variables, any-qualified type variables and parameter-specific type restrictions are represented and enforced syntactically.
- [x] Typed views expose callable names and parameter bindings; malformed declarations recover at following items.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a generic callable with parameter defaults and an annotation declaration; verify typed parameter traversal, comments and idempotence. Include a parameter form forbidden by the grammar and recovery past a malformed callable.

## Result

Added function, predicate, test and annotation declarations with generic type-inst variables, parameters/defaults, direct binding traversal and stable formatting.

Validation:

- Independent verifier PASS against W579235d05bf2f1ab; unchanged snapshot.
- Cargo fmt, Clippy with denied warnings, and all 25 integration tests passed.
- Names/ranges, exact coverage, parameter grammar restrictions, recovery, comments and idempotence passed.
- MiniZinc 2.10.1 accepted representative original/formatted model with literal default; earlier-parameter dependent default remains syntax-only evidence.
