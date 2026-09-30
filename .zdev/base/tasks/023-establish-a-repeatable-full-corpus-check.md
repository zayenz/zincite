+++
schema_version = 1
id = "base-023"
key = "corpus-runner"
area = "base"
status = "open"
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

- [ ] Acquire the complete external Challenge archive, enumerate all local model/data files, report revision/year/subtree counts and absent published data, and reconcile every file with a result or explicit classification.
- [ ] Run lossless token/tree coverage, parse, formatting/reparse/second-pass and current lint checks with per-file failures and limits; preserve source spellings, comments and include/directive attachment using the brief allowances.
- [ ] Keep corpus paths configurable, originals read-only and private, and reports outside tracked source by default; document the command and group failures by a small number of actionable causes.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run against all available inputs, demonstrate continuation after malformed input, and hand-check one formatting mismatch. Reuse existing test invariants; do not add a framework or large fixture collection.
