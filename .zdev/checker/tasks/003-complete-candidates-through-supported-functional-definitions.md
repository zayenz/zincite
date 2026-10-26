+++
schema_version = 1
id = "checker-003"
key = "functional-completion"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = ["checker-002"]
+++
# Complete candidates through supported functional definitions

## Outcome

Primary decisions determine supported auxiliary values before the checker evaluates the candidate's constraints.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Extend the new checker using the compilation contract's functional subset. Read [the design context](../background/design-context.md) for the author's infeasible-state use case. Runtime values must continue flowing through a domain violation so independent downstream violations are observable.

## Boundaries

- Use a direct dependency worklist and supported deterministic directions. Do not search, guess defaults or implement a general propagation engine.
- Implement only the bounded arithmetic, linear, Boolean/reified and element operations needed by the saved compilation example; preserve the distinction between equivalence and half reification.

## Done when

- [ ] Aliases and supported definitions derive a complete candidate from the example's primary decisions. Linear definitions require a unique exact result; every defining relation is still checked, including when the caller supplies its output.
- [ ] An infeasible derived value stays available to downstream checks; all evaluable violations are returned. Checked arithmetic and undefined operations follow documented MiniZinc semantics and cannot wrap or crash.
- [ ] Missing inputs, unresolved cycles and unsupported derivation directions produce structured incomplete reports that include known violations. A fully supplied cycle can be checked, and repeated checks do not reuse prior derived values.

## Validation

- Add a few public scenarios for the derived valid/invalid example, a supplied derived-value conflict, an unresolved versus supplied cycle, and a changed/removed primary assignment.
- Include one reification/half-reification distinction and one relevant arithmetic boundary; run the Rust workspace checks.
