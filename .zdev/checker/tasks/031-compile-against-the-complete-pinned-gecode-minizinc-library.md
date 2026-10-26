+++
schema_version = 1
id = "checker-031"
key = "gecode-profile"
area = "checker"
status = "open"
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

- [ ] The documented profile selects the exact upstream library and source revision, supports models including gecode.mzn and standard globals, and does not silently replace it with the installed release or the narrow checker override directory.
- [ ] All external mathematical declarations reachable from this library, plus registered compatibility aliases, match implemented signature/semantics rows. A compact closure check detects missing operations instead of silently decomposing away the required extension coverage.
- [ ] Compile-only examples cover newer explicit-end scheduling, optional rectangles, set/float and number-theory extensions, flags/annotations and application feedback. Experimental blackbox/restart hooks produce the declared located diagnostics.

## Validation

- Run reproducible compile-only checks with MiniZinc 2.10.1 and pinned Gecode sources; compare emitted predicates and direct candidate reports, including conflicts, omitted inputs and exact integer/set transport.
- Run established native/Wasm smoke and workspace checks for changed code, authored MiniZinc format/lint and both zdev checks.
