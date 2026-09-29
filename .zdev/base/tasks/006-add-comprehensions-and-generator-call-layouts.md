+++
schema_version = 1
id = "base-006"
key = "generators"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-005"]
+++
# Add comprehensions and generator-call layouts

## Outcome

Comprehensions and generator calls preserve generator/filter meaning and follow the agreed layout.

## Context

Read the pinned MiniZinc 2.10.1 grammar for this family and the current supported-syntax notes. Extend the existing parser, typed traversal and formatter together; update those notes with the exact support delivered. Read brief Formatting direction, particularly forall and long generator headers. Extend collection expressions with generators, bound names and where filters.

## Boundaries

- Do not reorder generators or filters, infer types or rewrite quantifiers.
- Only syntax families delivered so far are accepted in nested bodies; later expression tasks extend them through the same parser.

## Done when

- [ ] Set/array and indexed array comprehensions and generator calls parse and format with ordered generators, bindings and attached where filters accessible to consumers; cover both in-generators and assignment generators.
- [ ] forall bodies always expand; short other generator bodies may remain compact and explicitly expanded bodies stay expanded.
- [ ] Long headers break with one generator per line and each where filter indented below its generator; nested calls preserve attachment.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a nested comprehension/filter model and short/long forall and sum examples; check binding/filter structure, comment retention and idempotence. Supplement with MiniZinc 2.10.1 acceptance.
