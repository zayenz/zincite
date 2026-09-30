+++
schema_version = 1
id = "base-040"
key = "unused-declarations"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-033"]
+++
# Report unreachable declarations in complete model roots

## Outcome

Thesis rule unused-declaration follows resolved references across files and respects model entry points.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.12. Constraints, solve and explicit/default output plus relevant annotations seed reachability; callable bodies and declaration types/initializers supply edges.

## Done when

- [ ] Find unused variables/callables including unreachable cycles, while retaining declarations used transitively through constraints, solve, output, types, annotations and included user code.
- [ ] Suppress nested reports under an unused enclosing declaration and respect intentional-unused bindings; do not declare exported fragment definitions unused without a complete root.
- [ ] Unresolved references/call candidates cannot produce confident false unused findings; report the analysis limitation instead.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use one multi-file model with used and unreachable cycles, overloaded calls, implicit output, shadowing and an unused outer declaration; check a library fragment remains safe.
