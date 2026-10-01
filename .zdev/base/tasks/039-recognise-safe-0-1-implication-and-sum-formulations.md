+++
schema_version = 1
id = "base-039"
key = "zero-one-advice"
area = "base"
status = "done"
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

- [x] Recognise both a=1 -> b=1 and a=0 -> b=0 when relevant expressions have proven 0..1 domains, with sound bound propagation for supported arithmetic.
- [x] Recognise sum(i in S)(a[i]=1) only for a matching complete unfiltered 0..1 array traversal; keep unknown, optional, partial and instance-dependent rewrites unclaimed.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check the three positive families, reversed/extra domains, closed arithmetic and parameter-dependent/filtered/partial negatives.

## Result

Added invariant zero/one bounds and advice for both implication forms and complete matching array sums; independently verified Wa42a9f42b3c01305.

Validation:

- Independent whole-task PASS; cargo fmt, clippy, workspace tests, zdev check and diff check pass.
- Pinned MiniZinc 2.10.1/core CLI replay gives four main and three direction warnings, quiet accepted controls and explicit user-overload limitation; inputs and binary unchanged.
- Parameter-default division false hint reproduced before correction and quiet afterward; six unrelated paths remain unchanged and unstaged.
