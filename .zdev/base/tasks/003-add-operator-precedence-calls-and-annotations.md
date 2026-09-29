+++
schema_version = 1
id = "base-003"
key = "scalar-expressions"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-002"]
+++
# Add operator precedence, calls and annotations

## Outcome

Scalar expressions retain their meaning through precedence-aware parsing and formatting.

## Context

Read the minimal expression/tree/formatter interfaces and the pinned grammar sections for expressions and operators. Extend those interfaces with the complete operator table and call/annotation forms; this is the shared expression foundation for later grammar families.

## Boundaries

- Do not simplify expressions, drop parentheses or add semantic analysis.
- Collections, control expressions, generators and interpolation remain unsupported until their own tasks.

## Done when

- [ ] All pinned unary/binary operators, associativity and precedence, backtick operators, quoted operator calls, ordinary calls including named arguments, parentheses and anonymous/absent atoms parse and format.
- [ ] Declaration and expression annotations and annotation literals retain attachment; any declarations and ann types are supported syntactically.
- [ ] Half-open/open-ended ranges described by the pinned specification are supported, and restricted numeric-expression contexts use the appropriate grammar without type inference.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a small precedence/associativity model plus named/quoted calls and an annotated expression; compare meaningful tree structure after formatting and second-pass output. Check compiler acceptance of representative valid inputs.
