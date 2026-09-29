+++
schema_version = 1
id = "base-002"
key = "scalar-path"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-001"]
+++
# Parse and format a minimal scalar model

## Outcome

A zincite-fmt library and command format a complete scalar model using the shared concrete syntax tree.

## Context

Read the token API, brief Implementation direction and formatting examples, and ../background/syntax-and-reuse.md. Build a minimal CST, item-boundary recovery and formatter for declarations initialized by atoms, constraint true and solve satisfy. This first complete path intentionally has no expression-operator engine; scalar-expressions adds that next.

## Boundaries

- Support stdin and one file to stdout only; check/write, configuration, data mode and complete grammar arrive later.
- Do not silently accept unsupported input or emit partial formatted source on an error.

## Done when

- [ ] Primitive scalar declarations/assignments with var/par modifiers and literal or identifier values, anonymous atoms, direct string-labelled constraints and solve satisfy parse into a source-preserving tree. Calls, general annotations and operators are explicitly unsupported until scalar-expressions.
- [ ] Recoverable syntax errors retain precise diagnostics and permit a subsequent valid top-level item to be inspected; tree traversal exposes items, expressions and ranges.
- [ ] Formatting retains comments and atom spelling, applies constraint-label layout, and reaches a stable second pass for this minimal subset.
- [ ] The CLI reads UTF-8 stdin or one file, reports located errors with status 2 and emits formatted source only on success; usage and the temporary grammar subset are documented.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use one complete minimal model for token/tree coverage, comment preservation and idempotence, plus malformed input followed by a valid declaration to check recovery.
- Run stdin/file CLI checks and supplement the valid model with MiniZinc 2.10.1 acceptance.
