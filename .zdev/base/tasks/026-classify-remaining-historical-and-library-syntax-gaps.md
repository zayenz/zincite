+++
schema_version = 1
id = "base-026"
key = "syntax-corpus-checkpoint"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-025"]
+++
# Classify remaining historical and library syntax gaps

## Outcome

A complete syntax corpus report separates valid unsupported syntax from malformed or intentionally negative inputs and names bounded follow-up work.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. The planning scan sampled only 38 Challenge models and did not classify every software test. Use the now-available full runner and compiler expectations to establish the remaining syntax families.

## Boundaries

- This task investigates and classifies; it does not promise to fix an unknown number of syntax families in one context.

## Done when

- [ ] Run every available model/data file through syntax checks and reconcile every rejection with valid unsupported syntax, malformed/negative input, encoding or unavailable dependency evidence.
- [ ] For each remaining Zincite syntax family, retain a minimal reproducer and propose a bounded implementation follow-up with concrete proof; make unresolved follow-ups blockers of corpus-acceptance. Report zero gaps only if the full inventory supports it.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check representative reductions against MiniZinc 2.10.1 or a documented historical grammar/compiler difference. No new tests are expected until an implementation follow-up fixes a confirmed defect.
