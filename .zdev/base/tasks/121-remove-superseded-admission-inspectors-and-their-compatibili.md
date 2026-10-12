+++
schema_version = 1
id = "base-121"
key = "retire-admission-inspectors"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-120"]
+++
# Remove superseded admission inspectors and their compatibility bridges

## Outcome

Legacy source-shape inspection that only licensed uncertainty is removed, leaving a documented finite set of useful proof owners and no semantic control flow based on diagnostic text.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Review the concrete entry points exported from callable_definitions/adapters.rs and callable_definitions.rs, their source/fallback, source/standard, source/collections and source/locals implementations, and the previous bridge sites in analysis.rs, definitions.rs and unbounded_variable.rs. Earlier tasks should already remove code made immediately dead; this task handles remaining shared admission-only paths and final consumer migration. This is a finite cleanup of the current callable-admission subsystem, not a general repository refactor.

## Boundaries

- Classify helpers by actual callers and guarantees, not line count. Remove those whose only role is to bless an Unknown result; retain any shared portion still needed for a named positive proof or required source-reference/type inspection.
- No replacement monolithic interpreter, new general safety hierarchy or broad formatting-only movement. Do not delete historical evidence or rewrite completed task records.
- Retire tests that enforce an obsolete admission classification only after preserving any distinct safety counterexample or positive proof they also exercise. Do not change expectations wholesale to obtain a green suite.

## Done when

- [x] Every remaining exported callable source-inspection entry point has a named concrete guarantee or required source-fact consumer; the semantic plan records the retained families and intentional opacity without cataloguing every private helper.
- [x] Admission-only regular/sliding/gcc, optional-matrix, local-initializer and union-domain paths no longer retain private interpreters merely to conclude Unknown. Any retained portion is justified by a current stronger consumer and a representative public witness.
- [x] Semantic branching no longer compares human-readable diagnostic text in the affected admission/definition/guarded/search paths. Consumers receive direct domain results from the owning module.
- [x] Remove unused inspected-local/whole-owner plumbing, fallback dispatch branches and adapters created solely for retired admission. No parallel old and new path remains for the migrated behavior.
- [x] The end state has less interpretation responsibility, not just more modules: the result explains the removed responsibilities, retained guarantees and intentional outcome changes, with file/line totals only as secondary context.

## Validation

- Run the relevant existing public tests before and after cleanup; add no tests solely for deleting helpers or changing private module layout.
- Search for the former refusal literals and exported adapters, inspect every remaining semantic caller, and verify rendering strings are no longer classification keys.
- Replay the representative opaque and retained-proof controls from earlier tasks, including fixes and error barriers.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.

## Result

Removed Unknown-only admission interpreters and unused inspected-local plumbing; retained proof/error owners and typed guarded limitations.

Validation:

- Independent whole-task PASS W4495ef383e427401: 402 workspace tests; 177 baseline/final focused controls and 25 retained witnesses; fmt, clippy, diff checks and zdev check passed. Intentional opacity and retired admission rows reviewed separately; no corpus or solver campaign.
