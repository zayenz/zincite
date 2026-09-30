+++
schema_version = 1
id = "base-052"
key = "lexer-corpus-compatibility"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Accept compiler-supported form-feed layout and escaped single quotes

## Outcome

The two confirmed lexical families parse losslessly and reach stable formatting without accepting malformed escapes.

## Context

Read the area brief and scripts/syntax-corpus-checkpoint.md. scripts/syntax-gap-reproducers.json retains compiler-positive form_feed and escaped_single_quote cases; five corpus files currently fail these lexical forms. Start in lexer.rs and existing public lexing tests. MiniZinc 2.10.1 accepts both reductions.

## Boundaries

- Limit changes to form-feed whitespace and the demonstrated single-quote string escape; preserve source bytes, token kinds, precise ranges and recovery.
- Use direct scanner changes; no scanner framework, syntax-family expansion or copied external fixtures.
- Initial task-owned path allocation: ["crates/zincite-syntax/src/lexer.rs","crates/zincite-syntax/tests/lexing.rs"]. Coordination may extend it only after checking retained parent edits and sibling assignments.

## Done when

- [ ] Both retained reductions and affected corpus files pass lossless parsing with exact spelling and ranges.
- [ ] Formatting preserves the accepted lexical bytes and is stable on focused examples; malformed nearby escapes still diagnose.
- [ ] Document the lexical extensions beside the scanner behavior and report the affected-file reconciliation.

## Validation

- Run required workspace fmt, clippy with -D warnings and tests.
- Use a few public lossless/escape checks, original/formatted MiniZinc 2.10.1 model-check-only reductions and the affected corpus files; never rewrite originals.
