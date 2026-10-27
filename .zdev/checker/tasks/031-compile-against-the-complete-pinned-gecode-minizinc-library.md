+++
schema_version = 1
id = "checker-031"
key = "gecode-profile"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-030", "checker-019", "checker-020", "checker-021", "checker-022", "checker-023", "checker-025", "checker-026", "checker-028", "checker-029", "checker-018"]
+++
# Compile against the complete pinned Gecode MiniZinc library

## Outcome

The checker accepts mathematical FlatZinc emitted by the actual pinned Gecode library.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Use Gecode main commit 186551447db0f74639d65cb8de5af190f15ac2c2, including gecode.mzn/gecode_fzn.mzn, overrides and versioned redefinitions. Compile-only testing needs its library sources, not a built solver.

## Done when

- [x] The documented profile selects the exact upstream library and source revision, supports models including gecode.mzn and standard globals, and does not silently replace it with the installed release or the narrow checker override directory.
- [x] All external mathematical declarations reachable from this library, plus registered compatibility aliases, match implemented signature/semantics rows. A compact closure check detects missing operations instead of silently decomposing away the required extension coverage.
- [x] Compile-only examples cover newer explicit-end scheduling, optional rectangles, set/float and number-theory extensions, flags/annotations and application feedback. Experimental blackbox/restart hooks produce the declared located diagnostics.

## Validation

- Run reproducible compile-only checks with MiniZinc 2.10.1 and pinned Gecode sources; compare emitted predicates and direct candidate reports, including conflicts, omitted inputs and exact integer/set transport.
- Run established native/Wasm smoke and workspace checks for changed code, authored MiniZinc format/lint and both zdev checks.

## Result

Completed the exact pinned Gecode compiler profile with isolated compatibility includes, signature and alias accounting, fixed-bound Boolean declaration aliases and explicit external-execution/restart-history diagnostics. Compiled application feedback exercises completion, supplied conflicts and incomplete reports. Independent full verification passed without corrections.

Validation:

- Both profiles have zero missing include edges. Native preparation validates 128 standard signatures, 316 Gecode signatures and 954 distinct registered mathematical alias/signature pairs across 918 names and all feature guards; excluded hooks have located reasons.
- All 28 compile controls pass against the fixed sources. Both CLI source/load forms match directly calculated valid, four-finding invalid, one-finding supplied-output conflict and eleven-unresolved incomplete cases; an invalid/incomplete edit also passes native/Wasm comparison.
- Workspace fmt, Clippy -D warnings and 662 tests passed; parser-free native 20 tests, supported native/wasm32 feature checks, locked release Wasm, matching Node/web bindings and dependency checks passed. Original smoke passes 168 comparisons; the optional Gecode artifact passes 178 across 20 artifacts and 89 candidates.
- Authored MiniZinc formatting/local lint/model checks, Ruff, ty, coverage and package checks pass. Default closure lint reproduces one pinned upstream naming warning and 44 analysis limitations. Archived FlatZinc is byte-identical compiler output, including intentional command-header whitespace. Full verifier snapshot W94be480ea9641333 is unchanged.
