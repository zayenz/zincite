+++
schema_version = 1
id = "checker-004"
key = "application-model"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-003"]
+++
# Package the checker library and application feedback example

## Outcome

A small reusable MiniZinc checker library and allocation/scheduling example connect source constraints to application-controlled structured feedback.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Read [the design context](../background/design-context.md) before working with annotation transport. Promote the proven compilation recipe into a documented offline workflow and grow the existing synthetic example just enough to exercise a realistic application mapping. Use the working runtime rather than introducing a second checking model.

## Boundaries

- Keep compilation an explicit offline step. No native compiler wrapper, arbitrary MiniZinc output reconstruction, publication or application-specific runtime rules.
- Preserve each flattened violation, including those sharing an application identifier; callers control grouping and presentation.

## Done when

- [x] The custom MiniZinc library/configuration and documented command compile the example to predicates covered by the runtime; the coverage table names accepted predicates, derivation directions and exclusions.
- [x] Application-controlled identifiers and payloads survive preparation and identify the affected entities through typed report fields without parsing messages. The example documents the mapping from application state to sufficient primary values.
- [x] A feasible edit, an infeasible edit with several violations, and omitted input give the expected reports. The compilation contract documents tested handling of eliminated decisions, domain restrictions, decomposition and symmetry-breaking choices without claiming untested equivalence.

## Validation

- Recompile the example, inspect all emitted predicates/metadata, and compare the reported violations with the intended source constraints directly.
- Add only focused coverage for metadata/feedback behavior not already exercised; run the Rust workspace checks and both zdev checks when updating records.

## Result

Packaged the MiniZinc annotation library and a scheduling feedback example that maps every structured finding to application jobs. Documented supported predicates, completion directions and tested compiler editing limits.

Validation:

- Independent verification passed against Wd9d6a68650262b50 with unchanged snapshot.
- Workspace formatting, Clippy with warnings denied and tests passed; both zdev checks passed.
- MiniZinc compile-only reproduction, all candidate reports, typed application feedback, compiler controls and Cargo package contents passed. No solver ran.
