+++
schema_version = 1
id = "checker-009"
key = "interactive-evaluation"
area = "checker"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["checker-007", "checker-008"]
+++
# Measure interactive checking and document the supported workflow

## Outcome

A small repeatable evaluation records native/browser checking latency and scaling, with user documentation matching the implemented dialect.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Reuse the working browser/application examples and the style of scripts/bench-save.py rather than making a benchmark service. The checker has a prepared-model interaction path, distinct from formatter process startup. Run measurements on the user's M1 Max; record the actual build and browser.

## Boundaries

- Keep to one named ordinary interactive example and a few growing inputs, using a small driver. No noisy cargo-test timing assertions, broad corpus/solver matrix, release or deployment.
- Keep incremental execution and evaluation-order redesign as evidence-backed follow-ups rather than silently enlarging this measurement task.

## Done when

- [ ] The repeatable report separates preparation, serialized load, repeated native checking, and Wasm boundary/report costs; it records p50/p95, sample counts/spread, input and model sizes, and serialized/Wasm sizes.
- [ ] The named ordinary example is compared with the 100 ms p95 target using at least 50 repeated checks; growing cases show time/memory scaling. Known result correctness and fresh-state behavior are checked outside timed sections.
- [ ] The documentation explains current CLI/library/browser use, structured violations, incomplete reports and exact support limits. Any missed target or unexplained scaling has a concrete profiled cause and bounded follow-up, without claiming the performance goal was met.

## Validation

- Execute the recorded release-build native/browser measurements and inspect known reports before accepting timings; keep local raw reports untracked by default.
- Review the documented commands against the working example; run established Rust/Wasm checks only when their code changes, plus both zdev checks for record changes.
