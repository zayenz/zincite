+++
schema_version = 1
id = "base-133"
key = "formatter-stale-write"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Reject stale formatter write targets

## Outcome

Explicit formatter writes preserve changes made after the original input was read.

## Context

At reviewed revision 42f4d4b, crates/zincite-fmt/src/cli.rs reads the input once and renames completed output over the target without comparing current content. Start with process_file/replace_file and crates/zincite-lint/src/fix_files.rs. Read the approved usability follow-up in the brief.

## Boundaries

- No file locking framework or claim of eliminating the final filesystem race.

## Done when

- [ ] Before replacement, a changed original or changed target file type causes a located error and preserves the newer contents.
- [ ] Ordinary successful writes retain permissions and complete-output replacement; stdout/check behavior remains intact.

## Validation

- Use one deterministic stale-target check at the replacement seam, plus existing successful-write and symlink checks; avoid timing-dependent race tests.
- Run the brief's required Rust validation for code changes; use existing focused controls and only the few behavior checks specified here.
- Run zdev check base --format json for changed records.
