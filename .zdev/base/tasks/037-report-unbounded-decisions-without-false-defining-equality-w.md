+++
schema_version = 1
id = "base-037"
key = "domain-advice"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-036"]
+++
# Report unbounded decisions without false defining-equality warnings

## Outcome

Thesis rule unbounded-variable respects explicit bounds and complete definitions.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.8 using the domain and enforced-definition facts already consumed by constant-variable; keep the advice independent of solver inference.

## Done when

- [x] Report unbounded integer/float decisions and array element declarations through aliases, excluding explicit domains and initializer/complete defining equations.
- [x] Conditional equalities, circular self-definitions and partial element constraints do not incorrectly suppress warnings; nonnumeric decisions are not flagged.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use scalar/array int/float examples, alias bounds, a conditional equation and a partial array definition; check selected-rule/suppression behavior.

## Result

Implemented unbounded-variable advice with explicit limits for unsupported anchored array definitions, preserving warnings for genuine cycles and partial/conditional definitions. Independent whole-task verification confirmed the corrected candidate against Wd4605866cb37bc97.

Validation:

- cargo fmt --all -- --check passed.
- cargo clippy --workspace --all-targets -- -D warnings passed.
- cargo test --workspace passed: 91 tests.
- Independent whole-task review PASS against Wd4605866cb37bc97; official comparisons equal, no repository validation writes.
- MiniZinc 2.10.1 model-check-only accepts the representative and bounded-array-copy probes. Installed-core CLI preserves six expected warnings without limitations; the unsupported complete array-copy case reports one explicit index-membership limitation and no false warning. No solver ran.
- The extended existing public case failed before the array-copy correction and passed afterward; genuinely unanchored/self cycles and partial/conditional definitions retain their warnings.
- zdev check base --format json and git diff --check passed. Six foreign paths stayed byte-identical. Existing globals loader-cycle failures remain outside this task's acceptance claim.
