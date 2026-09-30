+++
schema_version = 1
id = "base-041"
key = "reified-global-advice"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-036"]
+++
# Report globals in reified and half-reified contexts

## Outcome

Thesis rule reified-global distinguishes enforced standard constraints from Boolean-valued uses.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.6 with resolved standard-library identity, expression instantiation and enforced-context traversal; an occurrence under a constraint item is not necessarily always true.

## Done when

- [ ] Report included standard-library Boolean predicates used in decision-dependent disjunction, implication, equivalence, negation, conditionals and value contexts.
- [ ] Do not report enforced standalone/conjoined globals, parameter-only evaluation, user lookalikes or implicitly available builtins; retain file/rule/suppression behavior.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check a directly enforced global, conjunction/forall, Boolean initializer and both implication sides, plus a shadowing predicate and par-only call.
