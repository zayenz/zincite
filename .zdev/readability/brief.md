# Readable Rust code

## Objective

Improve Zincite naming and module responsibilities through verified, behavior-preserving cleanup coordinated with the base implementation loop.

## Boundaries

- Preserve public behavior, source bytes and ranges, formatting output, rule selection,
  diagnostic ordering and exit codes unless a task names a demonstrated defect.
- The [base brief](../base/brief.md) remains authoritative for MiniZinc coverage,
  formatter and lint contracts. Read its relevant sections before changing those tools.
- Prefer direct functions, cohesive private modules and names that describe the work.
  File length alone does not justify a split. Do not add crates, generic dispatch
  machinery or public API migrations solely for style.
- Review files owned by the implementation chat read-only. Before each implementation,
  confirm its reserved files with that chat; coordinate any overlapping edits and Git
  completion. Keep unrelated changes and stage only the selected task's paths.
- The coordinator owns this area's records and commits. Implementers and verifiers
  use separate agents and follow the installed zdev task contract.

## Coding guidance

Read [zdev coding guidance](/Users/zayenz/.codex/skills/zdev/references/coding.md)
for the assigned role and the supplied
[Rust guidance](/Users/zayenz/Downloads/rust-style.md) before review or implementation.
Repository guidance and explicit user decisions take precedence. Apply guidance
proportionally: improve an actual reader's understanding without introducing ceremony.

All review, implementation and verification workers use `gpt-6.1-sol` with `high`
reasoning effort through this area's `readability-high` profile. Do not change the
implementation area's profile or project default.

## Testing

Existing checks only for behavior-preserving cleanup. Establish the relevant test baseline
before refactoring and rerun it afterwards. Add a focused public-behavior regression
only for a demonstrated defect that existing checks do not catch. No tests solely
for names, file moves, documentation or metadata.

Use focused coverage for a task that explicitly repairs a demonstrated behavior
defect. Add only the regression checks needed for that defect.

## Validation

Run `zdev check readability --format json` after changing this area's records and
`zdev check base --format json` as required by repository guidance. For Rust changes,
run `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
Use the base brief's existing supplementary acceptance checks only when the task
changes syntax or layout behavior or requires performance evidence.
