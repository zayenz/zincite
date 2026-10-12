+++
schema_version = 1
id = "base-123"
key = "semantic-right-sizing-acceptance"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-122", "base-111", "base-112"]
+++
# Verify the smaller semantic contract across lints and queries

## Outcome

The revised semantic system has demonstrated useful retained findings, conservative uncertainty, demand-driven reuse and honest user-facing accounting, with a clear handoff for future changes.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Read the task-1 support table and baseline selection, scripts/semantic-corpus-checkpoint.md, scripts/batch-performance.md, scripts/query-acceptance.md and current docs/linting.md/reference.md. Reconcile against the actual completed results of this bundle. Base-111 owns informational severity and base-112 owns root-aware compact limitation presentation; use their completed behavior rather than redesigning it. Base-050/base-083 remain historical acceptance, not evidence of this revision.

## Boundaries

- Use a bounded named set of existing public controls and representative original models; no full instance semantic replay, solver run, new lint family or guarantee of zero warnings/limitations.
- Keep private inputs read-only and out of committed fixtures/reports. If an old artifact or private original is unavailable, use an available representative for the named guarantee and explicitly limit the evidence; do not claim the missing original was replayed.
- Fix only demonstrated regressions within this contract using the normal task workflow; larger discoveries require a bounded follow-up, not an open-ended completion campaign. Do not silently weaken retained guarantees.

## Done when

- [x] Every current rule retains a useful supported path, source location, selection and suppression behavior; all fourteen thesis rules and the actual current all-rules set are accounted for using existing tests plus only missing focused checks.
- [x] The selected controls demonstrate retained structural types/overloads, checked numeric bounds, relational partiality/presence, exact index versus extent distinctions, direct/whole-array definitions, useful callable/search summaries and conservative possible-reference handling.
- [x] Opaque bounds/definitions/relations do not create negative semantic claims or safe fixes; known errors and input failures remain visible. Simple positive domain/search advice still works. Semantic query negation and best-effort data edits do not turn Unknown into a match or a complete write.
- [x] Both lint command forms and both query command forms agree with library results on the named controls. Informational-only output, warnings, errors, root labels, compact/default limitations and detailed limitations follow the completed CLI contracts.
- [x] Record intentional old-to-new outcome differences separately from regressions: supported opacity may complete with Unknown, while unimplemented promised operations, missing prerequisites and resource failures retain their actual status. Never relabel historical captures.
- [x] Standalone, combined and reordered semantic requests agree on relevant facts/findings; independent roots do not contaminate each other's caches. The selected fresh/mixed/repeated-request measurements show bounded memory lifetime and no unexplained material regressions.
- [x] Update the brief, semantic/callable plans and user documentation to describe the implemented state, retained exclusions and Salsa decision. Remove superseded future instructions without rewriting history. Future extensions require a named consumer, compositional guarantee, uncertainty behavior and proportionate evidence; clearing a limitation alone is not justification.

## Validation

- Run the full required Rust validation and zdev check base --format json; use existing focused public tests for rule coverage rather than generating a new exhaustive matrix.
- Run the baseline's bounded original-model and public controls through library and native commands. Use MiniZinc 2.10.1 no-solve acceptance only where it helps establish fixture validity; compiler acceptance is not proof of Zincite facts.
- Reuse existing release measurement scripts/cases for fresh small input, installed-library mixed analysis and repeated in-process requests. Attribute intentional result differences before comparing cost; report source preservation and unavailable evidence explicitly.
- Review the final diff and architecture against the task-1 retained contract, including proof barriers and removals, rather than judging success from reduced line counts or fewer Limited results.

## Result

Verified the implemented smaller semantic contract across all fourteen thesis and all 28 current rules, three original models, library/native command parity and reordered shared-owner requests. Reconciled current documentation and retained explicit uncertainty, Limited prerequisites and Salsa deferral; no production or test changes.

Validation:

- Independent whole-task PASS at W0c9ea856054dd2b8 with unchanged context; fmt, Clippy and full workspace tests (402) pass. Both lint/query command forms agree with library results; MiniZinc 2.10.1 model-check-only accepts three originals; 600 source/dependency hashes unchanged. Reused independently verified base-122 raw cost/lifetime evidence with no implementation delta. Canonical diff and zdev checks pass.
