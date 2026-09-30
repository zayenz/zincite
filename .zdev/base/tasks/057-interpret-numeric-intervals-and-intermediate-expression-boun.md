+++
schema_version = 1
id = "base-057"
key = "numeric-facts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-034", "base-035", "base-036"]
+++
# Interpret numeric intervals and intermediate expression bounds

## Outcome

Consumers can query conservative numeric and intermediate bounds without enabling a lint.

## Context

Extend declared domains from base-035 and unconditional definitions from base-036. Use source/declaration identity and the existing instantiation facts; index, partiality and domain rules are the concrete consumers. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [ ] Evaluate literals, aliases, supported arithmetic and unconditional definitions into exact, interval, symbolic or unknown facts with checked arithmetic.
- [ ] Track intermediate expressions and distinguish a proved exclusion/conflict from an over-approximation; unsupported operations cannot become guessed values or overflow.
- [ ] Expose facts and analysis limitations independently of findings, preserving symbolic N/K distinctions and definition cycles.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Check a bounded arithmetic chain, overflowing computation, unknown parameter, self/mutual definition cycle and a range excluding zero through a public fact consumer.
