+++
schema_version = 1
id = "base-033"
key = "call-facts-element"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-032"]
+++
# Resolve callable facts and deliver element advice

## Outcome

Thesis rule element-predicate distinguishes the standard predicate from user overloads and ambiguous calls.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.5 as the first consumer of callable/type facts. Use parsed standard-library signatures and declared types rather than matching the word element.

## Done when

- [x] Resolve callable candidates by arity, argument type/instantiation and supported polymorphic substitutions/coercions, retaining unknown when the selection cannot be justified.
- [x] Carry basic scalar/collection/enum/alias/optional/structured type facts needed to identify the standard three-argument element call, report indexing advice there, and skip incompatible or user-defined calls.
- [x] Keep genuine unresolved corpus calls visible so later coverage work cannot mistake skipped semantic analysis for success.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use a standard element call, shadowing/user overload, polymorphic array indices and an ambiguous call; verify rule selection, locations and suppression.

## Result

Implemented independent bounded callable/type facts and standard element readability advice. Semantic rules share bindings and retain separate completion outcomes.

Validation:

- Independent whole-task PASS at W243765c7234a503a; exact snapshot comparisons remained equal and no validation writes occurred.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: 77 tests passed.
- Public fact/API and CLI checks cover standard and user overloads, polymorphic indices, ambiguity, unknown calls, optional/structured boundaries, locations, suppression and rule-local outcomes.
- MiniZinc 2.10.1 --model-check-only and installed-library CLI checks independently reproduced three accepted positive/user probes and two no-match negative probes; input files unchanged.
- zdev check base --format json and git diff --check passed; six unrelated baseline paths preserved.
