+++
schema_version = 1
id = "base-150"
key = "shadowing-callable-formals"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Exempt callable parameters that share a top-level name

## Outcome

suspicious-shadowing does not report a callable formal parameter whose name matches a top-level declaration.

## Context

global-variable-in-function recommends passing a top-level decision into a callable as an argument, and the natural parameter name is the global's name. suspicious-shadowing then reports `predicate p(array[int] of var int: x)` as hiding the global `x`. That pattern made up about a quarter of its 312 corpus findings. Start in `crates/zincite-lint/src/shadowing.rs`.

## Boundaries

- Keep reporting generators and let declarations that hide top-level or enclosing bindings, and formals that hide another enclosing binding that is not top-level, if any exist.
- Keep the `ignore-names` option and underscore exemption unchanged.

## Done when

- [ ] The predicate example in Context reports no suspicious-shadowing finding.
- [ ] A generator `c` inside a constraint that hides a top-level `c` is still reported.
- [ ] `docs/lint-scope.md` and the rule's catalogue limitations describe the exemption.

## Validation

- Add the quiet formal case to the existing shadowing tests.
- Run the brief's Rust validation commands and `zdev check base --format json`.
