+++
schema_version = 1
id = "checker-032"
key = "flatzinc-json"
area = "checker"
status = "done"
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

- [x] An explicit native library/CLI input path accepts the official FlatZinc-JSON form for all supported values, references, arrays, declarations, constraints, solve goals and annotations without confusing it with --load prepared artifacts.
- [x] Text and equivalent compiler-emitted JSON produce equivalent checking/completion/application findings and objective/completeness results; exact integers are not rounded through binary64 during JSON parsing.
- [x] Malformed versions/shapes/references/types and unknown semantic operations diagnose at the input boundary. Prepared-model export/loading and parser-free Wasm consumers keep their documented roles.

## Validation

- Compile a few standard/Gecode inputs to text and FlatZinc-JSON and compare observable reports for valid, multi-violation and incomplete candidates; include large integers and floats.
- Add focused schema rejection/CLI cases, then run workspace and parser-free Wasm checks and both zdev checks.

## Result

Added official FlatZinc-JSON preparation through prepare_json and --input-format fzn-json in both native CLIs, including export. Raw numeric tokens and original JSON locations feed the existing validated lowerer, metadata and definition worklist. Compact integer/float ranges and restricted Boolean domains load and check in parser-free Wasm. Independent full verification passed without corrections.

Validation:

- Thirteen fresh standard/Gecode compiler text/JSON reports agree for valid, multi-violation, conflict and incomplete inputs after excluding source-specific locations. Focused public tests cover exact i64 endpoints, floats, sets, contextual arrays, aliases, references, annotations, malformed input and both CLI format boundaries.
- Workspace fmt, Clippy -D warnings and 668 tests passed; parser-free native 20 tests, supported native/wasm32 feature checks, locked release Wasm, matching bindings, dependency exclusions and package checks passed. Existing 168/178 and optional JSON-export 184 native/Wasm comparisons passed; browser bindings match four compiler-JSON-export reports and the exact large-integer objective.
- Coverage passes 128 standard, 316 Gecode and 954 alias signatures with no missing include edges. Local model/data formatting and lint pass; default lint reproduces the known upstream naming warning and 45 analysis limitations across the inspected models. Both zdev checks and diff checks pass; full verifier snapshot W16fa963e26b21412 is unchanged.
