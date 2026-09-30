+++
schema_version = 1
id = "base-034"
key = "decision-advice"
area = "base"
status = "done"
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

- [x] Propagate instantiation through aliases, calls, arrays, indexing, option values, records/tuples, conditionals, let expressions and comprehensions without guessing unknown facts.
- [x] Deliver decision-variable-operator, decision-variable-generator and decision-variable-condition for every detection family in the rule contract, including elseif and where.
- [x] Messages remain modelling advice, parameter-only contexts remain quiet, and unknown relevant facts produce analysis limitations.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use compact paired par/var examples, nested bindings and a user call whose result is decision-valued; verify equivalent operator spellings and a decision-only branch that must not flag its par condition.

## Result

Added independently reusable expression-instantiation facts and decision-dependent operator, generator and condition advice with rule-specific limitations and parameter-only silence.

Validation:

- Independent whole-task PASS against Wa93d42e0fa2b1f1b; original baseline We5e71a8e00e9ba1b and all six unrelated files preserved.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: 81 tests passed; git diff --check; zdev check base --format json passed.
- Pinned MiniZinc 2.10.1 model checks and installed-core CLI: 19 positive warnings/no errors or limitations, quiet user operator and infinity; set/array binder distinction confirmed. Infinity regression failed before correction and passed afterward.
- Separate globals.mzn probes compile, but the unchanged loader reports 16 include-cycle errors/status 2. These remain incomplete closure checks; full semantic corpus coverage is not claimed.
