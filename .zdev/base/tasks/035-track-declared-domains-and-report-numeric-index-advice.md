+++
schema_version = 1
id = "base-035"
key = "array-index-advice"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-033"]
+++
# Track declared domains and report numeric index advice

## Outcome

Thesis rule array-index-start uses known numeric bounds while respecting enum and symbolic domains.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.1. Add simple declared-domain and symbolic index-set facts to the existing semantic context, for reuse by constant/search/0..1 rules.

## Done when

- [x] Represent numeric ranges, literal sets, named domains, array index sets and unknown bounds; retain symbolic binding identity and fold safe closed arithmetic without requiring data.
- [x] Report provably non-one numeric lower bounds, including named domains, while leaving enum and unknown index sets free of numeric-offset guesses.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check one-based/non-one literal and aliased ranges, enums, unconstrained int and distinct N/K parameters; verify advice and suppression.

## Result

Added reusable declared-domain and symbolic index-set facts with array-index-start advice for proven nonempty numeric minima other than one; named aliases and source constants retain binding identity.

Validation:

- Independent whole-task PASS against Waa9225785206e879; original baseline Wfe9413d721ff99fe and all six unrelated files preserved.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: 84 tests passed; git diff --check; zdev check base --format json passed.
- Focused public and CLI checks cover aliases, parameter constants, symbolic identity, checked arithmetic, half-open bounds, source ranges, suppression and distinct empty/unknown/unsupported outcomes. Both annotated and aliased-set regressions failed before correction and pass afterward.
- Pinned MiniZinc 2.10.1 model checks and independent installed-core CLI replay: six representative warnings, corrected aliases warn at minimum zero, and enum/one-based/empty/unconstrained/symbolic cases stay quiet. No solver or full corpus run; existing globals loader errors remain outside this task's coverage.
