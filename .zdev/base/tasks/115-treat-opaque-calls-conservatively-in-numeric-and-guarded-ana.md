+++
schema_version = 1
id = "base-115"
key = "opaque-guarded-facts"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-114"]
+++
# Treat opaque calls conservatively in numeric and guarded analysis

## Outcome

Numeric and guarded consumers can finish with honest unknown facts for resolved opaque calls without specialized library-body admission.

## Context

Read .zdev/base/brief.md Semantic analysis, Interpretation before policy, Testing and Validation, and docs/semantic-processing-plan.md before implementation. Paths below are starting points, not an edit allowlist. Start at crates/zincite-lint/src/guarded.rs handlers for regular, sliding_sum, global_cardinality and optional parameter matrices, domains.rs numeric call handling, and callable_definitions/adapters.rs and source/standard modules. These inspected paths currently discard truth/numeric results and retain unknown definedness. Follow their actual consumers in partial_expression.rs, vacuous_constraint.rs, optional.rs and iteration.rs. Use existing guarded, numeric_facts, optional and partial_expression tests.

## Boundaries

- Preserve selected-declaration and type checks. An opaque fallback is not trust based on spelling, signature totality, or an assertion that the source is valid.
- Keep MiniZinc relational partiality, optional presence, raw/effective definedness and possible assertion failure distinct. Preserve known operand/source errors and existing local positive proofs.
- Limit this slice to numeric/guarded opacity and its directly affected consumers. Keep helpers still required by definition/search proofs until their consumers migrate.

## Done when

- [ ] Resolved calls outside the retained proof families have a common conservative fallback with no body-shape admission requirement. The regular/sliding_sum/global_cardinality and optional-matrix paths use it wherever their former inspection supplied only uncertainty.
- [ ] Opaque results cannot become exact numbers, true/false conditions, proven presence, totality or safe rewrites. Enclosing arithmetic, conditions and collections propagate the relevant uncertainty; an unrelated independently proved fact remains available.
- [ ] Known argument hazards and already discovered relevant errors remain visible. Replacing a body with an opaque operation cannot accidentally erase an assertion/abort barrier or make a previously unsafe elimination eligible.
- [ ] Library results and selected lint execution distinguish supported opacity from unresolved/ambiguous calls, missing promised typing and actual failures under the new contract. CLI output follows those library outcomes without a new reporting mode.
- [ ] Remove dedicated guarded admission branches and helpers that become unused in this slice. Record intentional outcome changes against the task-1 baseline.

## Validation

- Reuse existing regular/sliding_sum/gcc and guarded controls. Add only missing public cases for a resolved user call with no value proof, an opaque call inside a relevant parent expression, and a retained argument hazard.
- Check representative positive numeric/definedness findings still fire and opaque expressions enable neither vacuity conclusions nor fixes. Include an assertion/partiality counterexample using existing fixtures where possible.
- Use the brief's focused testing level and run its required Rust validation (cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace), plus git diff --check. Run zdev check base --format json for changed zdev records.
