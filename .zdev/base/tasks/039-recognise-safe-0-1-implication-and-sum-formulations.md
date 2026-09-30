+++
schema_version = 1
id = "base-039"
key = "zero-one-advice"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-036"]
+++
# Recognise safe 0..1 implication and sum formulations

## Outcome

Thesis rule effective-zero-one covers both implication forms and whole-array sums with proven domains.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.4 using declared domains, safe static arithmetic and symbolic index-set equality. The thesis warns that instance-dependent constants can make suggested rewrites invalid.

## Done when

- [ ] Recognise both a=1 -> b=1 and a=0 -> b=0 when relevant expressions have proven 0..1 domains, with sound bound propagation for supported arithmetic.
- [ ] Recognise sum(i in S)(a[i]=1) only for a matching complete unfiltered 0..1 array traversal; keep unknown, optional, partial and instance-dependent rewrites unclaimed.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check the three positive families, reversed/extra domains, closed arithmetic and parameter-dependent/filtered/partial negatives.
