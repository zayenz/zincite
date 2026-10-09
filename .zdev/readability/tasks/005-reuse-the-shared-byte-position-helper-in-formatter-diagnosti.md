+++
schema_version = 1
id = "readability-005"
key = "shared-byte-diagnostic-position"
area = "readability"
status = "done"
complexity = "routine"
afk = true
priority = "normal"
blocked_by = []
+++
# Reuse the shared byte-position helper in formatter diagnostics

## Outcome

The formatter CLI uses zincite_syntax::byte_line_column for encoding-error positions instead of keeping a duplicate coordinate implementation.

## Context

In crates/zincite-fmt/src/main.rs, format_source calls private input_line_column for encoding errors. Its calculation body is identical to the already exported byte_line_column in zincite-syntax/src/bytes.rs. The shared helper adds a byte-boundary assertion: parser encoding-error ranges start at invalid UTF-8 bytes, which are valid boundaries. The caller already uses BOM-stripped bytes and render_diagnostic restores the BOM byte offset separately. Reuse the documented helper and remove the duplicate without changing that arrangement.

## Boundaries

- Own only crates/zincite-fmt/src/main.rs during the newly granted formatter source window; keep validation and commits sequential with the implementation chat.
- Preserve diagnostic text, line/column and byte-range semantics, BOM handling, error paths and exit codes. Do not change the syntax helper or API.
- No compatibility wrapper, new helper, new dependency or new tests are needed.

## Done when

- [x] The CLI imports and calls the shared byte_line_column helper, and the private input_line_column duplicate is removed.
- [x] The existing CLI byte/encoding/BOM checks and required workspace checks pass; the shared helper precondition holds at the caller.

## Validation

- Establish and rerun cargo test -p zincite-fmt --test cli opaque_comment_bytes_survive_cli_modes_while_errors_preserve_input.
- Inspect the caller, parser diagnostic ranges and shared boundary contract; compare the removed calculation with the shared implementation and run required workspace commands from the readability brief.

## Result

Formatter encoding-error diagnostics now call zincite_syntax::byte_line_column and the private duplicate is removed. Source offsets, separate BOM restoration, diagnostics and error paths are preserved. Independently verified against W527c2522bb43f8e2.

Validation:

- Existing opaque-comment/encoding/BOM CLI test passed before and after; independent focused CLI test and workspace formatting, Clippy with warnings denied, and tests/doctests passed.
- Checked identical calculation bodies and parser invalid-byte boundary precondition. git diff --check passed; only main.rs changed and no new tests or validation writes.
