+++
schema_version = 1
id = "base-109"
key = "per-element-domain-advice"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "high"
blocked_by = []
+++
# Respect explicit per-element range constraints in unbounded advice

## Outcome

Full-index unconditional per-element range constraints avoid advice to add bounds already present.

## Context

Read brief.md Adopted Atlas feedback and bounded semantic contracts. Begin at crates/zincite-lint/src/unbounded_variable.rs, domains.rs and tests/unbounded.rs. The reported pattern is forall (object in Objects) (let { local declarations } in object_size[object] in 0 .. object_bound[object] /\ further_constraints), where object_size is indexed by Objects. The original excerpt is incomplete; first build a small complete reproduction.

## Boundaries

- Use existing bounded source facts and conservative traversal; no general callable-body interpreter or inferred numeric bounds.
- If required constructs fall outside supported analysis, retain a located limitation and Limited outcome instead of categorical add-bounds advice; do not suppress unrelated findings.

## Done when

- [ ] The supported unconditional full-index range reproduction has selected-rule outcome Completed, no relevant limitation and no false add-bounds warning.
- [ ] Filtered, partial-index and conditional constraints are not promoted to complete coverage.
- [ ] Local or record constructs either use supported facts or produce a located limitation with selected-rule outcome Limited; the focused reproduction and disposition are recorded.

## Validation

- Add a small complete positive model and contrasting partial/conditional cases through the public analysis API; no corpus sweep.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
