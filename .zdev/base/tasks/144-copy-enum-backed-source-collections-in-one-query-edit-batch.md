+++
schema_version = 1
id = "base-144"
key = "source-copy-query-batches"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Copy enum-backed source collections in one query edit batch

## Outcome

Editing query programs can build explicit member-copy mappings and append copied enum members, records and groups with their comments through one validated atomic candidate.

## Context

The user wants to grow wedding data through Zincite query language. The brief Source-preserving growth in query programs section owns the new syntax and semantics. Existing create/replace in crates/zincite-query/src/data.rs only accepts value declarations and reconstructs uncommented values. comprehension.rs enum_cases only sees model-written explicit members; wedding enum members are data-defined. Start with program.rs native bindings/stages and terminal data edits, data.rs batch validation/finish_candidate, reduction.rs model-backed literal shape/domain reading, and the existing grow-wedding Rust example. New native copies should retain original ranges and generated identifier substitutions through existing EditPart::Original, not construct new Zirql members. The later task adds bounded table-constructor growth and the complete wedding walkthrough.

## Boundaries

- Implement the generic clone_members, clone_elements, repeat_elements and append surface defined by the brief, using existing shared Program sequencing, lexical scope and budget. No second evaluator, new Zirql API/value kind/dependency, wedding-specific library primitive or general synthesis framework.
- Keep ordinary create/replace, enum_member and read-only reports compatible. Restrict new source-copy stages to editing programs requiring model facts. Preserve original-input reads, explicit targets and one final batch; reject duplicate targets and unsupported dependent mixed edits instead of checking stale facts.
- Preserve original and copied comments and source spelling through parsed source slices; remap only established member/key tokens. Refuse copied directives and unsupported computed/ambiguous shapes. Use idiomatic Rust ownership, borrowed input lifetimes, opaque native wrappers where needed, Result/Option and located errors.
- Keep default resource allowances and file-safety policy. No version bump, publication, deployment or private-data fixtures. Do not broaden reduction or constructed semantics merely to reuse helpers.

## Done when

- [ ] A query with params/lets, clone_members, clone_elements, repeat_elements and explicit append edits produces correctly ordered fresh enum members and copied keyed records/group sets, with ordinary shared scalar argument evaluation and native mapping reuse.
- [ ] Final candidate checks validate edited and unchanged affected .dzn assignments against retained model types, final enum membership/domains, literal types/member references and array coverage. Computed or ambiguous data obligations refuse; model-derived declarations, decision arrays, constraints and solve items are not evaluated. Preserve native input/model-owner admission. Missing dependent edits, collisions, duplicate targets and unsupported data fail without partial stdout or writes.
- [ ] Original RHS and unrelated bytes remain intact; copied record/comment bytes remain exact except mapped member/key tokens. Zero additional copies preserve exact bytes. Existing report and constructed-edit behavior remains valid.
- [ ] A few existing-style public library/CLI checks cover a commented enum/record/group case, mapping reuse and identity, collision/omitted-dependent refusal and one limit boundary. New syntax, native result restrictions and budget behavior are documented in the query contract/reference.
- [ ] Required Rust checks pass; changes introduce no needless public constructors, traits or allocation/lifetime abstractions.

## Validation

- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace.
- Run git diff --check; coordinator runs zdev check base --format json.
- Use a small synthetic model/data query example through existing Program and CLI seams; no internal call-count tests, generic matrix or timing assertions. Reuse existing data/edit tests and fixture conventions.
