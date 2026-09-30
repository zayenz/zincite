+++
schema_version = 1
id = "base-059"
key = "optional-facts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-034", "base-035", "base-058"]
+++
# Interpret optional presence and comprehension cardinality

## Outcome

Consumers can distinguish array capacity, present-element counts and definedness of optional expressions.

## Context

Use existing optional types and instantiation facts, not surface spelling. Read the MiniZinc option-type documentation linked in the brief. hidden-optionality and partial-expression rules consume these facts. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Represent occurs/absent/default/deopt and option-producing decision generators/filters, preserving known, conditional and unknown presence.
- [ ] Expose the difference between fixed array length and present-element cardinality for supported comprehensions, including variable sets.
- [ ] Feed deopt obligations into the shared definedness interpretation; ordinary optional arithmetic is not assumed to behave like nonoptional arithmetic.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check variable-set and variable-filter comprehensions, parameter-only filters, known absence/presence, guarded deopt and default expressions without enabling lint rules.
