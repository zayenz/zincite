+++
schema_version = 1
id = "base-059"
key = "optional-facts"
area = "base"
status = "done"
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

- [x] Represent occurs/absent/default/deopt and option-producing decision generators/filters, preserving known, conditional and unknown presence.
- [x] Expose the difference between fixed array length and present-element cardinality for supported comprehensions, including variable sets.
- [x] Feed deopt obligations into the shared definedness interpretation; ordinary optional arithmetic is not assumed to behave like nonoptional arithmetic.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check variable-set and variable-filter comprehensions, parameter-only filters, known absence/presence, guarded deopt and default expressions without enabling lint rules.

## Result

Added independent optional presence and collection cardinality facts, with scoped deopt obligations and default capture in the shared guarded interpreter; fixed immutable local initializer presence without treating replaceable defaults as instance facts.

Validation:

- Independent whole-task PASS at frozen W9d674a96add257bf; coordinator official comparison equal with no findings.
- cargo fmt --all -- --check passed independently.
- cargo clippy --offline --workspace --all-targets -- -D warnings passed independently.
- cargo test --offline --workspace passed independently: 130 tests.
- Eight MiniZinc 2.10.1 compile-only controls matched expected diagnostics and FlatZinc; absent deopt failed as expected; no solver run.
- Enforced local optional initializer regression failed before the bounded correction and passed after it; independent counterexample replay confirms Absent/Present while defaults and omitted data remain unknown.
