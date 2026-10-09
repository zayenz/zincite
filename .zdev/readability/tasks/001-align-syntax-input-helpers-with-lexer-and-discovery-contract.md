+++
schema_version = 1
id = "readability-001"
key = "syntax-input-contracts"
area = "readability"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Align syntax input helpers with lexer and discovery contracts

## Outcome

Literal include decoding and path-based file mode agree with syntax already accepted by the lexer and input discovery.

## Context

The read-only review found two inconsistencies in crates/zincite-syntax/src/lib.rs. literal_include_path does not decode escaped apostrophes although lexer::string_escape accepts them; semantic include loading consequently reports an unevaluated include and formatter include sorting treats it as a barrier. FileMode::from_path recognizes only lowercase dzn although inputs::source_extension discovers it case-insensitively; a directory-discovered .DZN file is consequently parsed as model input. Begin at those two public helpers and the focused tests in crates/zincite-syntax/tests/parsing.rs. The syntax reviewer and coordinating agent independently checked both findings against the same source. Read the readability brief's coding guidance and base brief's language/command contract.

## Boundaries

- Own zincite-syntax/src/lib.rs and focused syntax tests for these corrections. Confirm the exact shared-main edit and commit window with the implementation chat before edits; do not touch its reserved lint, formatter core or CLI paths.
- Preserve public signatures, retained spelling/bytes/ranges, parser grammar and existing supported escapes. Do not introduce a decoding framework or extension-policy abstraction.
- Keep case-insensitive discovery as the existing policy and make file-mode selection agree with it. Unknown extensions still select model mode.

## Done when

- [x] A valid literal include containing an escaped apostrophe decodes to the intended path bytes without an evaluation limitation; original source spelling remains retained.
- [x] Lowercase, uppercase and mixed-case dzn paths select data mode consistently with directory discovery; other paths retain model mode.
- [x] Focused public-behavior regression checks detect both original inconsistencies, and the required workspace checks pass.

## Validation

- Establish the syntax test baseline before editing. Add only focused regression assertions for the two public helpers and demonstrate that they fail on the original behavior.
- Run the Rust workspace validation commands in the readability brief. Inspect the source-preserving result and unchanged public signatures; no solver run or broad corpus sweep is required for these helper corrections.

## Result

Decoded escaped apostrophes in literal includes and aligned case-insensitive data-file mode selection with discovery; independently verified both helper contracts.

Validation:

- Both focused regressions failed before correction and pass afterwards.
- Independent cargo fmt, workspace Clippy with denied warnings, workspace tests and doctests passed; source spelling/ranges and signatures preserved.
