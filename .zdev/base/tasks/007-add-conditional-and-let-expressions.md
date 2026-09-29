+++
schema_version = 1
id = "base-007"
key = "control-expressions"
area = "base"
status = "open"
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

- [ ] The pinned conditional and let productions, including local declarations, local constraints and nested bodies, parse and format.
- [ ] Local declaration/binding ranges are available to naming checks; parentheses, comments and annotation attachment are preserved.
- [ ] Recovery from a malformed local block progresses to following items, and temporary type exclusions remain documented.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a nested conditional/let example with a local constraint and comments, plus one malformed block; check structure, recovery and format stability. Check a complete valid model with MiniZinc 2.10.1.
