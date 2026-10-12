+++
schema_version = 1
id = "base-113"
key = "report-version-and-library-hint"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Report the build version and suggest standard-library setup

## Outcome

Users can identify the zincite build and resolve standard-library setup without guessing.

## Context

Read brief.md Adopted Atlas feedback and Language and command-line contract. Start at src/main.rs, root CLI tests, crates/zincite-lint/src/cli.rs and model.rs. zincite --version currently reports an unknown command. Choose the approved lightweight library-path suggestion alternative rather than automatic installation discovery.

## Boundaries

- No MiniZinc compiler/solver execution for ordinary linting, no network access or elaborate installation discovery. Preserve explicit --stdlib-dir and MZN_STDLIB_DIR precedence.
- Build revision is best-effort: builds outside a Git checkout must work.

## Done when

- [ ] zincite --version succeeds and shows the package version plus build revision when available.
- [ ] An unresolved standard-library dependency gives an actionable --stdlib-dir/MZN_STDLIB_DIR hint, including the common MiniZincIDE library path when relevant.
- [ ] Help and usage documentation describe version reporting and the setup hint.

## Validation

- Use focused root CLI version and missing-library diagnostic checks; verify building without an available revision remains supported.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
- Run zdev check base --format json; install the changed local commands after verification.
