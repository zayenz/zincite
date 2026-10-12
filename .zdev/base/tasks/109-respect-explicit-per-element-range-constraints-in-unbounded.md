+++
schema_version = 1
id = "base-109"
key = "per-element-domain-advice"
area = "base"
status = "done"
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

## Focused reproduction and boundary

The public reproduction in `crates/zincite-lint/tests/unbounded.rs` uses:

```minizinc
enum Objects = {first, second};
array[Objects] of int: object_bound;
array[Objects] of var int: object_size;
constraint forall(object in Objects)(let { int: lower = 0; } in
    object_size[object] in 0..object_bound[object] /\ object_size[object] >= lower);
solve satisfy;
```

The initial regression, using `lower` as the range's lower endpoint, reproduced
the false warning before implementation. The exact `0..object_bound[object]`
form now completes selected unbounded-variable analysis without errors,
limitations or findings. Filtered, subset, single-index, implication and branch
controls retain advice and completed analysis.

The record boundary uses a parameter array of `record(int: cap)` and a local
`int: upper = object_bound[object].cap` as the range endpoint. It remains Limited,
with a limitation located at the `upper` reference and no categorical advice for
the affected array. A separate unbounded declaration retains its warning.
Range evidence stays private to this rule; it does not create definitions,
evaluated bounds or search coverage.

## Done when

- [x] The supported unconditional full-index range reproduction has selected-rule outcome Completed, no relevant limitation and no false add-bounds warning.
- [x] Filtered, partial-index and conditional constraints are not promoted to complete coverage.
- [x] Local or record constructs either use supported facts or produce a located limitation with selected-rule outcome Limited; the focused reproduction and disposition are recorded.

## Validation

- Add a small complete positive model and contrasting partial/conditional cases through the public analysis API; no corpus sweep.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.

## Result

Unconditional full-index integer range constraints suppress false unbounded advice; partial constraints retain advice and unsupported required sources stay located Limited.

Validation:

- Independent review passed against W77674af089d5ec00; post-review and pre-completion comparisons equal.
- Six public unbounded tests, cargo fmt, clippy, workspace tests and zdev check passed; exact reproduction and record/local boundary are recorded above.
- Updated zincite and zincite-lint installed locally, match reviewed release binaries and pass the full-index range smoke check.
