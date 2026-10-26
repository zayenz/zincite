+++
schema_version = 1
id = "checker-032"
key = "flatzinc-json"
area = "checker"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["checker-030"]
+++
# Accept the standard FlatZinc-JSON representation

## Outcome

Text and official FlatZinc-JSON model inputs lower to the same validated executable and reports.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read the 2.10.1 FlatZinc-JSON schema/section. This is compiler model JSON, distinct from the existing versioned prepared-model JSON and typed Wasm assignment JSON.

## Boundaries

- Reuse validated lowering/executable types directly; no second evaluator or generic JSON migration framework.

## Done when

- [ ] An explicit native library/CLI input path accepts the official FlatZinc-JSON form for all supported values, references, arrays, declarations, constraints, solve goals and annotations without confusing it with --load prepared artifacts.
- [ ] Text and equivalent compiler-emitted JSON produce equivalent checking/completion/application findings and objective/completeness results; exact integers are not rounded through binary64 during JSON parsing.
- [ ] Malformed versions/shapes/references/types and unknown semantic operations diagnose at the input boundary. Prepared-model export/loading and parser-free Wasm consumers keep their documented roles.

## Validation

- Compile a few standard/Gecode inputs to text and FlatZinc-JSON and compare observable reports for valid, multi-violation and incomplete candidates; include large integers and floats.
- Add focused schema rejection/CLI cases, then run workspace and parser-free Wasm checks and both zdev checks.
