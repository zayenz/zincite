+++
schema_version = 1
id = "base-084"
key = "umbrella-cli"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Add the root zincite command with shared fmt and lint entry points

## Outcome

The root zincite binary provides fmt and lint subcommands with the same behavior as the standalone binaries.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Commands and ownership. Cargo.toml currently defines a virtual workspace; crates/zincite-fmt/src/main.rs and crates/zincite-lint/src/main.rs own argument processing and execution. Their existing CLI tests are the behavior baseline. Move command execution into callable CLI modules and dispatch in-process from a root package named zincite.

## Boundaries

- Retain standalone installation, current defaults, configuration lookup, stdin/file/directory rules and 0/1/2 exit semantics; no subprocess dispatch or generic command framework.
- This task advertises only implemented fmt and lint subcommands; the item-query task registers query.

## Done when

- [ ] A Cargo package at the repository root builds and installs the zincite binary; fmt and lint dispatch through the same entry points as their standalone binaries.
- [ ] Root/subcommand help, no-subcommand help and unknown-subcommand errors follow the brief and reflect the actual invocation.
- [ ] Both command forms agree for formatter stdout/check/write and lint selection/settings/diff/fix/error paths; existing library APIs remain usable independently.
- [ ] README installation and command examples describe both invocation forms without claiming query support yet.

## Validation

- Reuse existing CLI tests; add only a few public comparisons for formatter stdout/check, lint selection/settings and usage failure. Preserve existing write/fix checks rather than duplicating their full matrix.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.
