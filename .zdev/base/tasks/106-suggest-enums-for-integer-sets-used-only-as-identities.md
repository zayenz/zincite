+++
schema_version = 1
id = "base-106"
key = "advise-enum-for-identity-sets"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-099"]
+++
# Suggest enums for integer sets used only as identities

## Outcome

An opt-in, diagnostic-only prefer-enum rule identifies a bounded class of integer identity sets, checks supported member uses throughout the model and withholds advice on numeric, representation-sensitive or uncertain affected uses.

## Context

The user requested advice when an integer set represents object identities rather than quantities. Absence of arithmetic in constraints alone is insufficient: initializers, user functions, solve objectives, output, annotations and data-facing declarations can depend on numeric representation. Use the brief's bounded Semantic analysis policy and docs/semantic-processing-plan.md. Base-083 is parked and concerns existing thesis support/accounting; it promises no member-use classifier. This task owns its narrow new analysis and does not depend on base-083. Completed base-099 provides the source/signature callable route; base-105 demonstrates independent default membership/model requirements and a small consumer of binding/call/domain facts.

Investigation on 2026-10-11 at db78d463: bindings.rs retains declaration IDs, lexical resolutions, reference kinds and user-body ownership; callables.rs retains source signatures, selected declarations, instantiated formal/result types and expression types without requiring selected standard-body expansion. domains.rs retains Domain::Named identity, written integer ranges/sets and separate array index/element domains. iteration.rs links generator bindings to sources/dependencies. unused_declarations.rs supplies roots/dependencies and explicit uncertainty, including solve/output/annotations. These facts support navigation but do not prove that an Int value is an identity. Do not reuse unused reachability as a numeric-use proof or ignore numerical observations in declarations merely because they are unreachable from a constraint.

Two inspected gaps guide the bounded design: TypeKind::Int does not retain its originating named set, and tuple/record domains currently become Domain::NonNumeric. Structured types/projection CST therefore need separate candidate-member tracking if later supported; matching integer bounds never establishes common identity. An instantiated generic formal may become Int because the current actual is an integer member: inspect the original selected signature before treating that as a concrete integer API requirement. Conversely a generic signature does not prove its implementation ignores arithmetic. Array index and element roles must remain separate: cost[m] + 1 uses m as a key and its result as a cost, not as a member. Arithmetic on card(Machines) concerns a count and must not block advice by itself.

Compiler observations used installed MiniZinc 2.10.1 with Gecode compilation only, without solving. Both set People=1..3 and enum People={Ada,Bo,Cy}, with array[People] of var bool: chosen and sum(p in People)(bool2int(chosen[p]))=1, compiled. The enum control 2*card(People)=6 compiled. For set People=10..12, sum(People)=33 compiled; substituting the three-member enum produced a deprecated implicit enum-to-int warning and failed the same assertion because ordinals sum to 6. A second original with charge(int:p)=2*p and sum([p:charge(p)|p in People])=66 compiled, while its enum replacement warned and failed the assertion. The enum controls Ada<Bo /\ Bo<Cy and array[People] of int: costs=[10,20,30] with endpoint lookups compiled. These observations establish categorical indexing/counting, representation-sensitive arithmetic and hidden calls; they do not certify general conversion or equal FlatZinc. Enum ordering can preserve intended order, so declining ordering in the first version is a conservative boundary, not a language restriction.

Primary sources: MiniZinc 2.10.1 specification, Enumerated types (https://docs.minizinc.dev/en/2.10.1/spec.html#enumerated-types), describes nominal enum types, declaration order and integer coercion; the pinned changelog (https://docs.minizinc.dev/en/2.10.1/changelog.html) documents deprecated implicit enum-to-int conversion. Compilation success alone cannot prove migration equivalence. External data/consumers and enum member names remain human choices; no automatic conversion is justified.

## Boundaries

- Initial candidates are named, nonempty finite parameter integer sets with written literal sets or closed integer ranges. Follow candidate-domain scalar declarations, generator members and direct aliases by declaration identity. Support identity indexing, same-family equality/membership/set operations and count-only uses; never infer membership from equal bounds. Require at least one established categorical member or index use; unused and count-only sets are not positive candidates.
- Check every affected written model use, including initializers, supported acyclic first-order user-call actual/formal/result links, objectives, explicit/default output and annotations. Numeric member arithmetic/aggregation, numeric constants or sentinels in member comparisons, ordering/extrema/successors, numeric conversions and concrete integer API contracts withhold advice. Identify implicit enum coercion risks rather than trusting compiler acceptance.
- Candidate-member storage in tuple/record fields or array elements, computed membership, recursive calls and opaque or unresolved propagation may remain explicitly unsupported initially; affected candidates receive no suggestion. Candidate-indexed arrays with independent numeric payloads are supported. Unrelated unknown facts must not veto an independently classified candidate. Missing implementation support remains located Limited; supported but unproved facts remain Unknown under the brief.
- Require a complete resolved model context; fragments and standalone data cannot establish the needed model-wide evidence. Advice concerns observed model uses and does not promise external data/output compatibility or equivalent migration. Withhold when the model exposes numeric member representation, including supported implicit output behavior.
- Keep the rule opt-in in the style family, available by explicit ID and existing applicable family/all selections, outside defaults and thesis. Use existing library/CLI selection, suppression, locations and per-rule outcomes; emit no fixes and do not alter formatter behavior.
- Use a small lint-specific static analysis over existing facts/CST. No general evaluator, new semantic crate, relation-body inspector, unrestricted library-body expansion, base-083 repairs, corpus sweep or solving. Preserve existing body-aware consumers in mixed rule selections.

## Done when

- [ ] Library and both lint entry points expose prefer-enum with advisory wording, a declaration location, working selection/suppression and no fix; defaults and thesis selections remain unchanged.
- [ ] Focused positive models establish candidate identity use through domains/generators/direct aliases and supported user calls, including numeric array payload arithmetic and cardinality arithmetic without confusing them with numeric member use.
- [ ] Focused counterexamples withhold advice for hidden numeric operations, objectives, literals/sentinels, representation-sensitive output/annotations, ordering and concrete integer contracts; nominal identity, lexical shadowing and generic versus concrete callable signatures are respected.
- [ ] Affected unknown/unsupported propagation and incomplete context withhold advice with honest outcomes; unrelated limitations do not suppress supported candidates. Document the initial supported operations and exclusions.
- [ ] The implementation records its precise supported call/alias boundary, compiler controls and any remaining gaps without claiming whole-language completeness or safe enum conversion.

## Validation

- Use a few grouped public behavioral checks for the positive identity/count/index cases, negative representation cases and affected/unrelated uncertainty; follow focused Testing in the brief, avoiding a combinatorial test matrix. Include unused/count-only negative controls in that grouped coverage.
- Recreate minimal identity-index/count and numeric-offset/hidden-function controls with MiniZinc 2.10.1: compile original controls first using minizinc --compile --solver gecode with finite limits and temporary outputs, then inspect replacement controls, warnings and assertions. No solving, private inputs or corpus sweep; equivalence is not an acceptance claim.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace and zdev check base --format json; install current local commands after implementation as requested.
