+++
schema_version = 1
id = "base-023"
key = "corpus-runner"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Establish a repeatable full-corpus check

## Outcome

One explicit developer command inventories and checks the published Challenge archive and local MiniZinc tree without changing their files.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. The planning scan found 239 input/parse failures among 4,287 local files and 35 second-pass differences among 2,315 parseable files checked for formatting. Start with the three public crate APIs, existing formatting tests and CLI behavior.

## Boundaries

- This is correctness acceptance, not solver benchmarking. Do not vendor the corpora or classify valid unsupported syntax as invalid.

## Done when

- [x] Acquire the complete external Challenge archive, enumerate all local model/data files, report revision/year/subtree counts and absent published data, and reconcile every file with a result or explicit classification.
- [x] Run lossless token/tree coverage, parse, formatting/reparse/second-pass and current lint checks with per-file failures and limits; preserve source spellings, comments and include/directive attachment using the brief allowances.
- [x] Keep corpus paths configurable, originals read-only and private, and reports outside tracked source by default; document the command and group failures by a small number of actionable causes.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run against all available inputs, demonstrate continuation after malformed input, and hand-check one formatting mismatch. Reuse existing test invariants; do not add a framework or large fixture collection.

## Result

Added a bounded full-corpus developer runner and reconciled read-only compatibility baseline for 6,417 inputs.

Validation:

- Independent whole-task PASS against unchanged W6606bcd8e9a92b9f; complete archive, local tree and official supplement inventory and hashes reconcile.
- Full run and documented supplemental audits retain one result per input, 534 failed files with explicit causes, and no changed originals; malformed continuation and real formatting mismatch reproduced.
- Cargo fmt, clippy, all 53 workspace tests and focused protected-byte example regression pass.
