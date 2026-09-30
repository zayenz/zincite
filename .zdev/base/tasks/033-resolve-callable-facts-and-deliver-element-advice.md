+++
schema_version = 1
id = "base-033"
key = "call-facts-element"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-032"]
+++
# Resolve callable facts and deliver element advice

## Outcome

Thesis rule element-predicate distinguishes the standard predicate from user overloads and ambiguous calls.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.5 as the first consumer of callable/type facts. Use parsed standard-library signatures and declared types rather than matching the word element.

## Done when

- [ ] Resolve callable candidates by arity, argument type/instantiation and supported polymorphic substitutions/coercions, retaining unknown when the selection cannot be justified.
- [ ] Carry basic scalar/collection/enum/alias/optional/structured type facts needed to identify the standard three-argument element call, report indexing advice there, and skip incompatible or user-defined calls.
- [ ] Keep genuine unresolved corpus calls visible so later coverage work cannot mistake skipped semantic analysis for success.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use a standard element call, shadowing/user overload, polymorphic array indices and an ambiguous call; verify rule selection, locations and suppression.
