+++
schema_version = 1
id = "base-111"
key = "symbolic-comprehension-info"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Render unproven Cartesian expense as informational advice

## Outcome

Symbolic Cartesian advice without proven numeric expense is informational and does not fail lint alone.

## Context

Read brief.md Rule catalogue severity policy. Start at crates/zincite-lint/src/expensive_comprehension.rs, analysis.rs, lib.rs and tests/expensive_comprehension.rs. CandidateCount symbolic findings already state that no numeric count or threshold exceedance is established; current rendering labels all findings warning.

## Boundaries

- Keep families and fix capability independent of severity. Do not introduce a general severity configuration system.
- Preserve existing warning/error exit precedence, selection and suppression behavior; no automatic modelling rewrite.

## Done when

- [ ] Symbolic-only Cartesian advice is represented and rendered as informational, with exit zero when no warnings/errors exist.
- [ ] Existing stronger numeric expense evidence retains warning behavior; mixed findings and errors preserve exit precedence.
- [ ] CLI help and rule explanation describe the distinction.

## Validation

- Add focused informational-only and mixed-severity public checks; retain existing numeric threshold tests.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
