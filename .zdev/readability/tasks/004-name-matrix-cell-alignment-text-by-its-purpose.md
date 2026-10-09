+++
schema_version = 1
id = "readability-004"
key = "matrix-alignment-name"
area = "readability"
status = "open"
complexity = "routine"
afk = true
priority = "normal"
blocked_by = []
+++
# Name matrix cell alignment text by its purpose

## Outcome

MatrixCell names clearly distinguish emitted text from the text used to measure alignment, with formatting unchanged.

## Context

In crates/zincite-fmt/src/lib.rs, MatrixCell.text is emitted by matrix row rendering. MatrixCell.slot is the same rendered fragment plus following punctuation and eligible comments; it supplies first-line width and forced-break decisions. Rename slot and its construction binding to alignment_text, retaining text. The reviewer and coordinator checked construction, measurement and emission: this is a small private naming correction, not a layout change.

## Boundaries

- Own only crates/zincite-fmt/src/lib.rs during the newly granted formatter source window; keep validation and commits sequential with the implementation chat.
- Preserve the field types, text construction, punctuation/comment handling, width calculation, break decisions and formatting output.
- Do not extract helpers, change the representation or add tests for the rename.

## Done when

- [ ] MatrixCell.alignment_text and its local construction binding identify the alignment text consistently; no MatrixCell.slot uses remain.
- [ ] The existing formatter matrix behavior and required workspace checks pass, with computation and output unchanged.

## Validation

- Establish and rerun cargo test -p zincite-fmt --test formatting matrix_columns_align_and_share_width_breaks_without_changing_rows.
- Inspect the diff for naming-only changes, search for old field/binding uses, and run required workspace commands from the readability brief.
