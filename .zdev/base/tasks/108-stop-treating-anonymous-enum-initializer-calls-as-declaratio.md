+++
schema_version = 1
id = "base-108"
key = "anonymous-enum-call-role"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Stop treating anonymous enum initializer calls as declarations

## Outcome

Naming and unused-declaration analysis distinguish anonymous enum initializer calls from declared constructors.

## Context

Read brief.md Initial lint contract and Adopted Atlas feedback. enum Position = anon_enum(2); currently produces a naming warning on anon_enum. Begin at enum_cases in crates/zincite-syntax/src/parser.rs, naming.rs, bindings.rs and unused_declarations.rs in zincite-lint. The unused-declaration observation has not yet been independently reproduced.

## Boundaries

- Preserve genuine enum member/constructor declarations and exact source ranges. Do not fix this through spelling-only naming exemptions.
- No general semantic expansion or external feedback file copy.

## Done when

- [ ] A complete anonymous-enum root with solve satisfy and required standard-call facts checks Position without treating anon_enum as a model declaration; unused-declaration analysis is Completed with no relevant limitation or false warning.
- [ ] Genuine named enum constructors retain naming, binding and unused-declaration behavior.
- [ ] The unused-declaration observation is independently tested and its actual current status recorded.

## Validation

- Add a focused public-behavior regression for the reproduced false positive and a genuine constructor counterexample; reuse parser source-preservation checks.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
