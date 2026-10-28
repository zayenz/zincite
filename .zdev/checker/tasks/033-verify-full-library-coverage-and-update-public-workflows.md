+++
schema_version = 1
id = "checker-033"
key = "full-acceptance"
area = "checker"
status = "done"
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

- [x] Every targeted mathematical coverage row is implemented and exercised at its actual signature; both standard and pinned Gecode include/decomposition closures have no unknown/unassigned operations. Alias/deprecated and conditional set/float/MPFR paths are accounted for; external/search-state exclusions are explicit.
- [x] Native library, both CLI forms, source/prepared/FlatZinc-JSON input and parser-free Wasm agree on representative complete, infeasible, partial and conflicting-output reports. Configurable float settings and integer exactness survive the public paths.
- [x] The original graphical example still handles valid/invalid/recovery/omitted states. A named >=50-check M1 Max release browser/native measurement repeats the ordinary 100 ms p95 target, recording actual build/library revisions, artifacts and scope without assuming the earlier timings still apply.
- [x] The checker guide and support documentation describe exact entry points, both stdlib profiles, supported full predicate surface, numeric limits and external-hook exclusions. Any actual target miss has a concrete profiled cause and bounded follow-up; do not assert unsupported coverage or achieved latency.

## Validation

- Audit the table against the exact two source revisions and family evidence; close a small concrete integration gap directly or use a scoped zdev split if new implementation work is needed.
- Run established workspace checks, parser-free build/dependency inspection, focused native/Wasm smoke, both CLI paths, compile-only profile checks and actual browser interactions/measurements. No broad solver or UI matrix.

## Result

Verified full pinned-library mathematical coverage and public workflows, updated support documentation, and corrected benchmark fixture ordering. Fresh M1 Max native/browser acceptance meets the 100 ms p95 target.

Validation:

- Independent full review passed at W77d9d866d72afc41 with no required corrections; 128 standard, 316 Gecode and 954 registered alias/signature contracts; 959 exact names exercised; zero include gaps.
- Workspace fmt, Clippy and 668 tests; 20 parser-free tests; release Wasm and dependency inspection; 184 native/Wasm comparisons; 64 public-path reports and 18 numeric checks passed.
- Both pinned compiler profiles and documented source limits checked; Python Ruff/ty, generated coverage and zdev checker/base checks passed.
- Independent real-browser graphical valid/invalid/recovery/omitted checks passed through Playwright while the Mac was locked.
- Fresh m1-max-release-2026-10-28-full-acceptance-v2: 60 individual checks after 10 warmups; four complete records, zero failures; ordinary native p95 0.008834 ms, browser assignment-to-parsed-report p95 0.100000024 ms; artifact hashes verified.
