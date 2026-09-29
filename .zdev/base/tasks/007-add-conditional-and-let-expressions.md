+++
schema_version = 1
id = "base-007"
key = "control-expressions"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-004"]
+++
# Add conditional and let expressions

## Outcome

Conditional branches and local declaration blocks parse and format without changing scope or expression structure.

## Context

Read the pinned MiniZinc 2.10.1 grammar for this family and the current supported-syntax notes. Extend the existing parser, typed traversal and formatter together; update those notes with the exact support delivered. Read current expression parsing and declaration parsing; reuse them for if/elseif/else and let syntax rather than introducing separate local-expression machinery.

## Boundaries

- Support local types already available; later enum/structured tasks extend the same type parser.
- No name resolution, branch simplification or semantic rewrites.

## Done when

- [x] The pinned conditional and let productions, including local declarations, local constraints and nested bodies, parse and format.
- [x] Local declaration/binding ranges are available to naming checks; parentheses, comments and annotation attachment are preserved.
- [x] Recovery from a malformed local block progresses to following items, and temporary type exclusions remain documented.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a nested conditional/let example with a local constraint and comments, plus one malformed block; check structure, recovery and format stability. Check a complete valid model with MiniZinc 2.10.1.

## Result

Added conditional branches and let blocks with local declaration traversal, preserved expression structure and block formatting.

Validation:

- Independent verifier PASS against Wac02f6482680b74c; unchanged snapshot.
- Cargo fmt, Clippy with denied warnings, and all 20 integration tests passed.
- Nested branches/local blocks, source coverage, binding ranges, comments, annotations, recovery and idempotence passed.
- MiniZinc 2.10.1 model-check-only accepted original and independently formatted complete fixture.
