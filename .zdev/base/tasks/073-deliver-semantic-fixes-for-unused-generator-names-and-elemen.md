+++
schema_version = 1
id = "base-073"
key = "lint-first-fixes"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-072", "base-066", "base-033", "base-058"]
+++
# Deliver semantic fixes for unused generator names and element calls

## Outcome

Two actual lint rules expose applicable fixes through library, preview and file application.

## Context

Use separate fact APIs and edit plans. These are narrow first consumers of the fix infrastructure, not an automatic rewriting programme for all advisory rules. Read brief.md, Linter expansion, for the shared contract.

## Boundaries

- Keep semantic interpretation, diagnostic policy and source editing separate; reuse existing facts and source identities.

## Done when

- [x] Offer a safe unused-generator-binding fix replacing only an unused name with _ where MiniZinc syntax allows it, preserving generator multiplicity and annotations.
- [x] Offer an element-predicate indexing-equality fix only for resolved supported standard calls with proved compatible indices/types/totality and preserved surrounding precedence/comments/annotations.
- [x] Keep a diagnostic without a fix when obligations fail; rule metadata and --explain accurately describe conditional support and safety.
- [x] Preview and --fix work end to end and a second application makes no further edit on the focused examples.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes. Use the focused testing level in the brief.
- Compare before/after semantics and MiniZinc acceptance for the two supported fixes; check an arithmetic aggregate, comments, unsafe/unknown element indices, user overload and a reified call.

## Result

Delivered conditional Safe fixes for unused comprehension binders and proved standard element calls through library, preview and file application; unsupported cases retain advice.

Validation:

- Independent verification PASS at Wf87b983675695da4. Cargo fmt, workspace Clippy and all 186 tests passed. Final-binary preview/apply/reapply/preview returned 1/0/0/0 with exact preserved bytes and no second edit. MiniZinc 2.10.1 accepted before/after counterparts removing only the common BOM; exact BOM rejection is retained. Semantic reasoning preserves nine Cartesian sum terms and resolved total indexing equality. No solver ran. zdev check and diff check passed; ownership unchanged and all handles reaped.
