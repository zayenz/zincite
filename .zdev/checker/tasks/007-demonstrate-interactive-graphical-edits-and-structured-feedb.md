+++
schema_version = 1
id = "checker-007"
key = "browser-example"
area = "checker"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["checker-004", "checker-006"]
+++
# Demonstrate interactive graphical edits and structured feedback

## Outcome

A local browser example lets users edit a small allocation/schedule and see all violations through application-defined presentation.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Use the existing synthetic application model and parser-free Wasm API. The application translates graphical state into primary values and uses structured identifiers/payloads to attach feedback to its entities. Use a simple local page with labelled, keyboard-operable controls.

## Boundaries

- Keep this a local illustrative consumer, without hosted deployment, account setup or an application framework.
- Keep infeasible edits visible. Completeness information is available to the application; insufficient input is never converted into a checker error.

## Done when

- [x] The page loads the prepared model once and checks repeated graphical edits locally, mapping violations to the affected entities without parsing checker-written text.
- [x] The example demonstrates valid edits, several simultaneous violations and recovery after another edit without stale findings. A small omitted-input example demonstrates structured incompleteness.
- [x] The documented local build/serve/run workflow works in an actual browser, with no compiler/solver/network service needed for checking. Preparation and runtime/report costs can be measured separately by the following performance task.

## Validation

- Exercise the named interactions in a real browser, including keyboard use and feedback after successive edits; compare those reports with the native example.
- Reuse the Wasm smoke checks and run the Rust workspace checks if Rust changes. Do not add a broad UI test suite.

## Result

Added a local graphical workshop schedule that reuses one Wasm model, maps every structured finding to jobs, allows infeasible edits and shows ordinary incomplete reports.

Validation:

- Independent full browser review passed after one documentation correction and focused recheck at unchanged snapshot Wda0a3e090203a408.
- Actual browser valid, four-finding invalid, recovery and omitted-input reports matched native JSON. Keyboard controls, narrow layout, feedback replacement and local-only loading checks passed.
- Existing Node/Wasm smoke passed twelve comparisons. Parser-free build and dependency inspection passed; no Rust source changed. Whitespace checks passed.
