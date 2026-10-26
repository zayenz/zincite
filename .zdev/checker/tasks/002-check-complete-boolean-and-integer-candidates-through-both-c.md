+++
schema_version = 1
id = "checker-002"
key = "native-check"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = ["checker-001"]
+++
# Check complete Boolean and integer candidates through both CLIs

## Outcome

The new zincite-check library and both CLI forms check complete candidates and return structured violations and incomplete reports.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Start with crates/zincite-syntax/src/lib.rs, src/main.rs and tests/cli.rs; sibling crates show the thin library/CLI pattern. Consume the preceding compilation contract's direct-check subset. This is a narrow working checker, not scaffolding for every future predicate.

## Boundaries

- Reuse the CST and ranges; reject MiniZinc constructs outside the admitted FlatZinc dialect rather than interpreting arbitrary source.
- Use Boolean/checked integer values, scalar and literal-array assignments, aliases and semantic domains. Keep linter/query execution and solver calls out of the checker.

## Done when

- [ ] Public preparation/checking APIs and zincite-check / zincite check accept the documented FlatZinc and literal .dzn assignment forms, with located preparation errors for unsupported types, predicates, signatures or malformed references.
- [ ] The direct-check subset reports all concrete violations in source order, with typed fields, supplied application identifiers/payloads and locations. Input names, duplicates, aliases, types and array shapes are validated; out-of-domain values are violations rather than silently changed.
- [ ] Text and JSON preserve independent completeness information, unresolved variables and blocked checks, with and without known violations. Insufficient input returns a report. CLI status follows the brief: 0 valid, 1 invalid/incomplete, 2 actual errors; both entry points agree.

## Validation

- Add a few public behavior checks combining complete success, several violations, insufficient input with/without a known violation, and malformed/unsupported input. Include one CLI equivalence check.
- Document the working command and the admitted direct subset, then run the area brief's Rust workspace checks.
