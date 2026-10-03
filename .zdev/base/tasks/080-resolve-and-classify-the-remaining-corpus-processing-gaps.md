+++
schema_version = 1
id = "base-080"
key = "corpus-processing-gaps"
area = "base"
status = "done"
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

- Keep external sources read-only and private source out of tracked fixtures/docs. Pre-check eligibility with the installed current MiniZinc 2.10.1 system; compile known instances to FlatZinc with Gecode without solving. Do not guess model/data pairs or reinterpret .dzn declarations as model syntax.
- Repair only demonstrated valid target syntax through the existing parser/CST; no compiler/type checker, new language framework or Shackle dependency. Classify actual compiler-negative, grammar-document, malformed, incompatible-encoding and unavailable inputs explicitly.
- Initial task-owned path allocation: ["crates/zincite-syntax/src/parser.rs","crates/zincite-syntax/src/lexer.rs","crates/zincite-syntax/src/lib.rs","crates/zincite-syntax/tests/parsing.rs","crates/zincite-syntax/tests/lexing.rs","crates/zincite-fmt/examples/check-corpus-file.rs","scripts/check-corpus.py","scripts/corpus-baseline.md","scripts/formatter-corpus-checkpoint.md"]. Coordination may extend it only after checking retained parent edits and sibling assignments.

## Done when

- [x] Each of the 27 current gap rows has a reproducible result and checked valid, nonvalid or unavailable classification; compiler metadata hints alone do not establish syntax validity.
- [x] Each of the six large public data inputs has a current MiniZinc eligibility result or an explicit availability limit. Compiler-positive inputs have adequate finite fresh token/tree coverage, spelling/structure/protected-byte, clean parse/reparse and idempotence checks, or a located diagnosed processing failure with a bounded necessary follow-up; compiler-broken inputs stay classified without becoming required Zincite successes.
- [x] Source accepted and processed by the current MiniZinc system among the twelve assessed rejections is accepted without weakening malformed-input diagnostics; compiler-rejected historical cases keep their classification, precise Zincite diagnostics and unchanged originals.
- [x] Recorded known model/data pairs are pre-checked by finite Gecode compilation to FlatZinc without solving. Original compiler-positive instances and complete staged formatted include trees compile with identical compiler/backend/settings; raw FlatZinc comparisons are recorded and differences investigated without unsupported equivalence claims. Missing pairings/data/dependencies, backend failures, timeouts and invalid UTF-8 remain explicit coverage limits.
- [x] A fresh complete no-rules syntax/format corpus reconciles every original input and classification after any repair; valid unresolved failures keep final acceptance open rather than being excluded.

## Validation

- Reuse the existing checker/runner with adequate finite targeted deadlines, exact input hashes and complete statuses/streams; retain all earlier timeouts and invalid rows.
- For actual syntax repairs add only focused public regression behavior as needed; run workspace fmt/clippy/tests and the existing fidelity corpus. No tests solely for classification/reporting.
- Rehash originals and preserve historical reports. Document source extensions, compiler distinctions, known pairings and remaining coverage limitations without private source.
- Retain original and formatted FlatZinc artifacts, compiler streams/statuses and exact version/backend/settings. Keep check-only syntax evidence separate from successful instance compilation; do not run a solver.

## Result

Classified all 27 corpus gaps and reconciled all 6417 originals; current MiniZinc/Gecode compile-only checks and raw FlatZinc investigations retained.

Validation:

- Independent whole-task verification PASS at Wfd9e6c68b3b825ff: 6396 clean fidelity checks, 21 classified exclusions, zero final timeout or unobserved input; original/formatted compile checks and retained raw differences verified.
- zdev check base and git diff --check passed; no production source changes requiring Cargo rerun.
