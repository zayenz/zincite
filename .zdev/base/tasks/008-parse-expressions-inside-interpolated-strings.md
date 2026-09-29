+++
schema_version = 1
id = "base-008"
key = "string-interpolation"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-003"]
+++
# Parse expressions inside interpolated strings

## Outcome

Interpolated strings expose embedded expressions without corrupting string contents or source ranges.

## Context

Read the pinned MiniZinc 2.10.1 grammar for this family and the current supported-syntax notes. Extend the existing parser, typed traversal and formatter together; update those notes with the exact support delivered. Read the scanner plain-string path and the scalar expression parser. Replace the temporary interpolation diagnostic with support for the pinned interpolation grammar; later expression forms use the same embedded parser.

## Boundaries

- Preserve literal segments, escapes and interpolation delimiters exactly. Do not normalize a string value or stringify and reparse a second tree.
- This task need not support expression families not yet implemented.

## Done when

- [x] Interpolated strings expose embedded syntax and precise ranges while leaf/source coverage remains exact with no duplicate or missing bytes.
- [x] Embedded expressions format with their original literal segments, nesting and annotation attachment intact.
- [x] Malformed interpolation reports an error and cannot produce successful partial formatting; supported-syntax notes remove the interpolation exclusion.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use one escaped string with multiple or nested interpolations and one malformed interpolation; check literal bytes, embedded structure, coverage and second-pass stability. Supplement the valid example with compiler acceptance.

## Result

Added source-preserving interpolated string chunks and embedded expression CST nodes using the existing parser and formatter.

Validation:

- Independent verifier PASS against W4196f5dd3f2da6f2; unchanged snapshot.
- Cargo fmt, Clippy with denied warnings, and all 22 integration tests passed.
- Exact token/leaf coverage, literal bytes, ranges, nesting, annotations, labels, malformed recovery/refusal and idempotence passed.
- MiniZinc 2.10.1 model-check-only accepted original scalar fixture and independently formatted output.
