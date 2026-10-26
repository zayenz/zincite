+++
schema_version = 1
id = "checker-006"
key = "wasm-runtime"
area = "checker"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["checker-005"]
+++
# Expose a parser-free WebAssembly checking API

## Outcome

JavaScript can load a prepared model and check typed assignments through thin Wasm bindings using the native checker core.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Build on the serialized model consumer. Check current Rust/Wasm tooling when implementing. Use feature boundaries within zincite-check to omit source preparation from the browser runtime; the full native package still prepares models and accepts .dzn assignments.

## Boundaries

- Keep filesystem access, MiniZinc execution, linter/query dependencies and parser code out of the parser-free Wasm build.
- Use thin bindings and the shared result records; no second evaluator or frontend framework.

## Done when

- [ ] A documented Wasm build exposes load-and-check operations for the prepared artifact, typed values and report data. Native CLI compilation still works.
- [ ] The parser-free target builds without zincite-syntax in its dependency graph, and a JavaScript consumer loads the artifact without access to source files or a compiler.
- [ ] A small executable JavaScript/Wasm smoke exercise agrees with native valid, invalid and incomplete reports, including application metadata, repeated checks and an integer beyond JavaScript's exact-number range.

## Validation

- Run the parser-free Wasm target build and inspect its dependency graph; document the exact repeatable commands with the example.
- Execute the small JavaScript/Wasm comparison rather than duplicating the native suite, then run the Rust workspace checks.
