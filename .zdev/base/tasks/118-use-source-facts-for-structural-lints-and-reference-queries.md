+++
schema_version = 1
id = "base-118"
key = "source-facts-consumers"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-114"]
+++
# Use source facts for structural lints and reference queries

## Outcome

Consumers of identity, types, instantiation and possible references no longer require optional selected-library-body proof expansion.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Read crates/zincite-lint/src/callables.rs resolve_source_callables/resolve_callables, analysis.rs rule routing, unused_declarations.rs, bindings.rs and crates/zincite-query/src/semantic.rs ensure_callables. Base-099 already separated three decision-use rules; model queries still call resolve_callables. Inspect each candidate consumer before rerouting: selected-call pattern rules, captures/reference usage, type and instantiation queries, and unused declarations have different source requirements. Existing callables, decision_use, unused_declarations and query tests provide starting evidence.

## Boundaries

- Keep one structural type and overload implementation, including signature/default dependencies and written user-body reference/local typing. Do not equate source-only with skipping user bodies.
- Unused-declaration needs a conservative graph of possible relevant references, implicit output and annotations. If a needed edge is uncertain, withhold the unused conclusion; a missing output/value proof alone is irrelevant to reference reachability.
- Preserve current public resolve_callables behavior for callers that explicitly request its legacy body-aware result until the later migration defines compatibility. Do not route lint through the user query language.

## Done when

- [ ] Identify and migrate every existing lint/query consumer whose required facts are source identities, signatures, structural types, instantiation or static references rather than body-computed values. Document any retained body demand with the concrete property it supplies.
- [ ] Type/instantiation/navigation queries use source facts and retain existing finite scan/result limits. An irrelevant standard implementation does not block a query or simple lint that has sufficient source facts.
- [ ] Unused-declaration walks the relevant possible-reference graph without output interpretation; unresolved potentially relevant edges veto unused conclusions, while opaque values alone do not make static-reference analysis incomplete.
- [ ] Unresolved/ambiguous selection, shadowing, nominal enum identity, option qualifiers, tuple/record projections and include ownership remain unchanged on supported source facts.
- [ ] Standalone and mixed rule selections report the same source facts and findings for these consumers; enabling a deep rule cannot change a simple rule's conclusion.

## Validation

- Reuse public callable/decision tests and add a query regression only if existing tests cannot distinguish source-only demand from transitive library-body expansion.
- Check unused declarations in complete roots and fragments, implicit output, annotations, reachable uncertain references and unreachable cycles using existing controls.
- Check both query commands and selected lint entry points with installed-library navigation; no broad corpus replay or new type-combination matrix.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.
