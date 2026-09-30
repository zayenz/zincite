+++
schema_version = 1
id = "base-034"
key = "decision-advice"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-033"]
+++
# Track expression instantiation and add decision-use advice

## Outcome

Three thesis rules report decision-dependent operators, generators and conditions using semantic facts.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement sections 4.10, 4.13 and 4.14. The same par/var/unknown expression facts are needed by later rules; derive them from bindings, operators and resolved call signatures.

## Done when

- [ ] Propagate instantiation through aliases, calls, arrays, indexing, option values, records/tuples, conditionals, let expressions and comprehensions without guessing unknown facts.
- [ ] Deliver decision-variable-operator, decision-variable-generator and decision-variable-condition for every detection family in the rule contract, including elseif and where.
- [ ] Messages remain modelling advice, parameter-only contexts remain quiet, and unknown relevant facts produce analysis limitations.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use compact paired par/var examples, nested bindings and a user call whose result is decision-valued; verify equivalent operator spellings and a decision-only branch that must not flag its par condition.
