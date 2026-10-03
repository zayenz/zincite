+++
schema_version = 1
id = "base-080"
key = "corpus-processing-gaps"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Resolve and classify the remaining corpus processing gaps

## Outcome

The current corpus has checked source-validity and availability classifications, adequate fidelity evidence for large inputs, and no remaining demonstrated valid-source rejection within the assessed gap set.

## Context

Read brief Corpus and thesis expansion, background/corpus-coverage.md, scripts/formatter-corpus-checkpoint.md and target/benchmarks/base079/corpus-audit.json. Base-079 inventories 6417 inputs and retains 27 gaps: six large-data ten-second timeouts, twelve syntax assessments, four data files containing model items and five invalid UTF-8 files. These are not 27 established Zincite bugs. Previous syntax follow-ups are complete; assess these actual remaining rows rather than historical counts. Preserve the archive revision, year counts, private input accounting, 2026 supplement and recorded explicit model/data/compiler entries.

## Boundaries

- Keep external sources read-only and private source out of tracked fixtures/docs. Use MiniZinc 2.10.1 no-solve checks as supplementary evidence; do not guess model/data pairs or reinterpret .dzn declarations as model syntax.
- Repair only demonstrated valid target syntax through the existing parser/CST; no compiler/type checker, new language framework or Shackle dependency. Classify actual compiler-negative, grammar-document, malformed, incompatible-encoding and unavailable inputs explicitly.
- Initial task-owned path allocation: ["crates/zincite-syntax/src/parser.rs","crates/zincite-syntax/src/lexer.rs","crates/zincite-syntax/src/lib.rs","crates/zincite-syntax/tests/parsing.rs","crates/zincite-syntax/tests/lexing.rs","crates/zincite-fmt/examples/check-corpus-file.rs","scripts/check-corpus.py","scripts/corpus-baseline.md","scripts/formatter-corpus-checkpoint.md"]. Coordination may extend it only after checking retained parent edits and sibling assignments.

## Done when

- [ ] Each of the 27 current gap rows has a reproducible result and checked valid, nonvalid or unavailable classification; compiler metadata hints alone do not establish syntax validity.
- [ ] The six large public data inputs have adequate finite fresh token/tree coverage, spelling/structure/protected-byte, clean parse/reparse and idempotence checks, or a located diagnosed processing failure with a bounded necessary follow-up.
- [ ] Compiler-supported valid syntax among the twelve assessed rejections is accepted without weakening malformed-input diagnostics; remaining nonvalid cases keep precise diagnostics and unchanged originals.
- [ ] Recorded known model/data pairs and staged formatted include trees are checked with the installed 2.10.1 compiler without solving; missing pairings/data/dependencies and invalid UTF-8 remain explicit coverage limits.
- [ ] A fresh complete no-rules syntax/format corpus reconciles every original input and classification after any repair; valid unresolved failures keep final acceptance open rather than being excluded.

## Validation

- Reuse the existing checker/runner with adequate finite targeted deadlines, exact input hashes and complete statuses/streams; retain all earlier timeouts and invalid rows.
- For actual syntax repairs add only focused public regression behavior as needed; run workspace fmt/clippy/tests and the existing fidelity corpus. No tests solely for classification/reporting.
- Rehash originals and preserve historical reports. Document source extensions, compiler distinctions, known pairings and remaining coverage limitations without private source.
