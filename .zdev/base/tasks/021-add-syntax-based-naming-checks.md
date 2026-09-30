+++
schema_version = 1
id = "base-021"
key = "naming-lint"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-020", "base-013"]
+++
# Add syntax-based naming checks

## Outcome

The linter checks names by their declared syntactic roles, including nested bindings, without semantic guesses.

## Context

Read brief Initial lint contract, the existing label rule/suppressions and typed declaration views. Add naming as a direct traversal over the complete parser; read ../background/linting-and-literature.md for the syntax-only boundary.

## Boundaries

- No reference matching, alias resolution, unused-declaration analysis or automatic renaming.
- Do not check standalone .dzn assignment targets against unavailable declarations; retain quoted/anonymous/leading-underscore exemptions.

## Done when

- [x] All classifiable roles in the brief receive snake_case or UpperCamelCase checks, with parameter sets distinguished from set-valued decision variables.
- [x] Callable parameters, fields, generators and local declarations are checked as bindings, while references and semantically ambiguous classifications are skipped.
- [x] Existing naming suppressions now silence only naming warnings through the next item, including nested bindings; label warnings remain independent.
- [x] CLI data-file mode, exit statuses and help cover both implemented rules and the completed syntax baseline.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use compact positive/negative examples for the naming roles, including a parameter set versus decision set, a nested binding, exemptions and .dzn targets.
- Check isolation between both rules and successive items under suppression; reuse label-lint CLI tests rather than duplicate them.

## Result

Implemented syntax-based naming checks with independent next-item suppressions and updated lint help and README.

Validation:

- Independent whole-task verifier PASS against W8e077894b71aa7c3; unchanged snapshot confirmed.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all 53 tests passed.
- Focused checks passed for declared roles, sets versus arrays, nested bindings, exemptions, skipped references/data targets and rule suppression isolation.
