+++
schema_version = 1
id = "checker-001"
key = "compilation-contract"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = []
+++
# Establish the checker compilation contract

## Outcome

A reproducible MiniZinc 2.10.1 experiment establishes the bounded FlatZinc dialect and metadata needed for the first checker.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Read [the design context](../background/design-context.md) before investigating compiler behavior. Zincite currently has no checker; existing syntax parsing does not establish FlatZinc validity. Use a tiny synthetic allocation/scheduling model with editable primary decisions, arithmetic auxiliaries, reification and an array. The installed compiler and library locations are recorded in the design context.

## Boundaries

- Compile only; do not build a solver, general compiler integration or runtime in this task. Keep the example redistributable and the experiment bounded.
- Keep infeasible edits representable. Do not assume -O0 preserves every constraint, decision or annotation; establish what actually survives.

## Done when

- [x] The example model, compilation command and checker library/configuration needed by that command reproduce inspected FlatZinc without running a solver.
- [x] A compact compilation contract records primary-variable/array addressing, definition directions, application metadata transport, and a finite predicate list split into direct checks and functional-completion operations. It documents transformations that affect editing or violation reporting.
- [x] Direct inspection covers a feasible candidate, an infeasible candidate with several independent violations and determined auxiliaries, and insufficient primary input. Metadata loss or unsupported constructs are resolved for this bounded example before claiming a usable recipe.

## Validation

- Run the exact compiler recipe and inspect emitted declarations, definitions, predicates, domains and application metadata against the synthetic source.
- Save the reusable findings under checker/background, index them from the brief, and run both zdev checks. No new Rust tests are expected.

## Result

Established the two-job checker compilation recipe, bounded predicate contract, structured metadata mapping and four direct candidate walkthroughs; independently verified.

Validation:

- MiniZinc 2.10.1 compile-only replay matched the archived FlatZinc except its command comment; no solver ran.
- Direct inspection verified all five relations, domains, aliases, definitions, application metadata and complete/incomplete candidate outcomes.
- Formatter checks and default lint passed, with the documented index-set-mismatch analysis limitation.
- Independent verification passed against Wc7fc8860840c21b0; checker and base zdev checks passed.
