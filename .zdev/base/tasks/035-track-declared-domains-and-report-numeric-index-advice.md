+++
schema_version = 1
id = "base-035"
key = "array-index-advice"
area = "base"
status = "open"
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

- [ ] Represent numeric ranges, literal sets, named domains, array index sets and unknown bounds; retain symbolic binding identity and fold safe closed arithmetic without requiring data.
- [ ] Report provably non-one numeric lower bounds, including named domains, while leaving enum and unknown index sets free of numeric-offset guesses.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check one-based/non-one literal and aliased ranges, enums, unconstrained int and distinct N/K parameters; verify advice and suppression.
