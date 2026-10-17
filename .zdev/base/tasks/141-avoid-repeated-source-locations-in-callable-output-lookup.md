+++
schema_version = 1
id = "base-141"
key = "hoist-callable-lookup-range"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Avoid repeated source locations in callable output lookup

## Outcome

Callable output lookup avoids rebuilding one invariant source location per expression row, with unchanged analysis outcomes and a measured comparison on java-routing.

## Context

The user authorized the next-step recommendations on 2026-10-17. Read scripts/java-routing-profile.md, especially its final recommendation, and crates/zincite-lint/src/callable_definitions/interpretation/outputs.rs. The type_of closure in literal_call_outputs computes location(node.range()).range inside .find for every candidate. The earlier profile established this hot path but did not measure this optimization. Use the current release source as the before baseline.

## Boundaries

- Preserve exact file/range matching, first-match behavior, diagnostics, Unknown/Limited outcomes and rule membership. No cache, new lookup index, semantic repair or general refactor.
- Measure only the unchanged java-routing root and normal include closure used by the existing profile. No solver, corpus campaign or permanent timing tests. Preserve private input files.

## Done when

- [x] The physical range is computed once per lookup and no redundant source-location computation remains in the row predicate.
- [x] Before/after release measurements of the same root retain complete status/diagnostic comparisons for default, thesis and all; results and measured benefit or lack of benefit are recorded honestly in the existing performance report.
- [x] Existing required Rust checks pass; no new tests are added solely to assert the mechanical hoist.

## Validation

- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace.
- Use at most three fresh-process samples per preset per revision, each capped at 60 seconds, with identical explicit stdlib and --isolated --show-limitations settings from scripts/java-routing-profile.md; record samples/spread and compare complete outputs. Run measurements serially without concurrent task-owned builds. Keep raw captures temporary.
- Run git diff --check and zdev check base --format json.

## Result

Hoisted invariant source-range construction from callable output lookup. Matched java-routing medians improve 2.70x for thesis and 2.64x for all, with byte-identical statuses and diagnostic streams.

Validation:

- Independent Astra/high verification PASS on We7b33c0b2d14da43; rustfmt, Clippy, workspace tests, zdev check and diff checks passed.
- 18 serial fresh-process measurements, no timeouts, saved captures independently reconciled; private inputs unchanged.
