+++
schema_version = 1
id = "checker-005"
key = "serialized-model"
area = "checker"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["checker-003"]
+++
# Export and load prepared models without reparsing source

## Outcome

Native applications can serialize, load and check a prepared model with the same observable results.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Add Serde to the new checker's executable representation, not the shared CST. The current library already separates preparation from repeated checks. Provide a small export/load path through the existing checker CLI so this capability has a concrete consumer before Wasm bindings.

## Boundaries

- Use a small versioned format with validated loading; no generalized artifact storage or migration framework.
- Represent integers exactly at the JavaScript boundary, including beyond 2^53; preserve structured metadata and completeness fields.

## Done when

- [x] The native CLI can prepare/export a model and load it for checking literal assignments; the documented library path exposes the same operation without parsing the FlatZinc again.
- [x] Loading rejects incompatible format versions and malformed executable references/types before checking. Serialized content contains executable values, operations and reporting metadata rather than a CST.
- [x] Feasible, infeasible and incomplete reports, including known violations in an incomplete candidate and application payloads, agree before and after round trip. Large integers are exact.

## Validation

- Use a few report round-trip checks and one deliberately invalid artifact, with a focused large-integer transport check.
- Exercise export/load through the documented CLI and run the Rust workspace checks.

## Result

Added versioned prepared-model JSON export and validated parser-free loading. Native library and CLI load/check paths preserve reports, application metadata, completeness and exact integer strings.

Validation:

- Independent verification passed against unchanged snapshot W1358352a70b69344.
- Workspace formatting, Clippy with warnings denied and tests passed. Parser-free loader tests, library build and Clippy passed; its dependency tree excludes zincite-syntax.
- Report round trips and malformed artifact checks passed, including integers beyond 2^53. Both CLI entry points passed export/load checks, statuses 0/1/2 and overwrite refusal.
