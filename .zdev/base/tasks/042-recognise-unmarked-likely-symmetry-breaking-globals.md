+++
schema_version = 1
id = "base-042"
key = "symmetry-advice"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-033"]
+++
# Recognise unmarked likely symmetry-breaking globals

## Outcome

Thesis rule unmarked-symmetry-breaking covers the full named family as explicit advice.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.11 and every function name in its coverage row. Resolve both standard predicates and symmetry_breaking_constraint rather than trusting spelling.

## Done when

- [x] Report the complete documented predicate family when outside a resolved symmetry wrapper, with a message that asks whether the constraint is intended to break symmetry.
- [x] Respect nested wrappers and user shadowing; never claim the predicate proves symmetry or that removing it preserves solutions.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use a small table-driven check of the finite predicate list and focused wrapped/shadowed/required-constraint wording examples.

## Result

Implemented resolved symmetry marker facts and advisory policy for eleven standard predicates; independently verified W7531ecd1a249c6f1.

Validation:

- Independent cargo fmt, clippy with denied warnings, workspace tests and diff checks passed.
- Pinned MiniZinc model-check accepted the bounded increasing model; native CLI emitted one unwrapped advisory and quiet nested wrapper controls, with no errors or limitations.
- Whole-task PASS W7531ecd1a249c6f1; exact candidate and six unrelated paths preserved.
