+++
schema_version = 1
id = "checker-033"
key = "full-acceptance"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-031", "checker-032"]
+++
# Verify full library coverage and update public workflows

## Outcome

The full-support claim is backed by exact library closure, public entry points and native/browser evidence.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. All family tasks must be done. Audit their table against upstream declarations, actual emitted calls and registry aliases rather than trusting checked task boxes. Reuse the workshop/browser and measurement driver instead of adding an acceptance platform.

## Done when

- [ ] Every targeted mathematical coverage row is implemented and exercised at its actual signature; both standard and pinned Gecode include/decomposition closures have no unknown/unassigned operations. Alias/deprecated and conditional set/float/MPFR paths are accounted for; external/search-state exclusions are explicit.
- [ ] Native library, both CLI forms, source/prepared/FlatZinc-JSON input and parser-free Wasm agree on representative complete, infeasible, partial and conflicting-output reports. Configurable float settings and integer exactness survive the public paths.
- [ ] The original graphical example still handles valid/invalid/recovery/omitted states. A named >=50-check M1 Max release browser/native measurement repeats the ordinary 100 ms p95 target, recording actual build/library revisions, artifacts and scope without assuming the earlier timings still apply.
- [ ] The checker guide and support documentation describe exact entry points, both stdlib profiles, supported full predicate surface, numeric limits and external-hook exclusions. Any actual target miss has a concrete profiled cause and bounded follow-up; do not assert unsupported coverage or achieved latency.

## Validation

- Audit the table against the exact two source revisions and family evidence; close a small concrete integration gap directly or use a scoped zdev split if new implementation work is needed.
- Run established workspace checks, parser-free build/dependency inspection, focused native/Wasm smoke, both CLI paths, compile-only profile checks and actual browser interactions/measurements. No broad solver or UI matrix.
