+++
schema_version = 1
id = "base-143"
key = "grow-wedding-instances"
area = "base"
status = "done"
complexity = "standard"
afk = true
blocked_by = []
+++
# Grow wedding instances with deterministic guest and group copies

## Outcome

A small executable example grows the selected wedding instances by a positive copy factor using Zincite public literal and source-edit APIs, with deterministic guest/group remapping and candidate-only output.

## Context

The user requested increasing instance size as well as reduction. The actual wedding data has literal Guests, guest record arrays, same_table/different_tables guest-set arrays, Tables=T(1..n), and literal table_capacities. The mixed data contains internal comments. Existing query create/replace deliberately refuses new nominal members and some commented reconstruction; keep those contracts. A dedicated Rust example in zincite-query can use Input::parse_named, QueryResult::Literals and public LiteralKind/source ranges, and zincite_lint SourceSnapshot/TextEdit/EditPart/prepare_text_edits. Use query APIs for literal selection and CST for the bounded table-constructor shape. Read the existing doubled-instance guide as a separate generic construction example; this task is the nominal/comment-preserving wedding workflow.

## Boundaries

- No new generic resize framework, query syntax, enum synthesis primitive, dependency, random data generator or Zirql change. Support only the documented wedding assignment shapes; refuse unsupported/computed/ambiguous shapes and invalid copy factors without partial stdout.
- Keep private inputs read-only and outputs temporary. Do not publish private model/guest data. Preserve original record source fragments/comments and unrelated assignment bytes; remap explicit guest identifiers only, never substrings or display-name strings. Generated names must be valid and collision-checked.
- Scale table count and repeat capacities with checked arithmetic and finite input/output limits. Copies preserve original guest/group relationships; no claim of satisfiability, independent solutions, runtime cost or random realism.

## Done when

- [x] The executable example accepts a positive copy factor, produces original and cloned guests with unique identifiers, clones same/different group memberships through an explicit mapping, repeats capacities and updates the supported table constructor consistently.
- [x] The example uses existing public APIs and reparses a complete buffered candidate before printing. Invalid factors, unsupported shapes, name collisions and exceeded bounds fail without partial output; originals remain unchanged.
- [x] A small anonymized public example/test demonstrates correct copied references and retained comments. Existing docs explain this growth workflow alongside reduction and distinguish it from generic integer-indexed doubling.
- [x] The small and150guest mixed private cases both grow by factor2 in temporary files, pass MiniZinc instance checking and direct enum/record/group/table/capacity comparisons, and preserve their original files.

## Validation

- Run the example on the focused public fixture and both private cases, with direct remapping/count/comment checks and MiniZinc --instance-check-only on candidates using the selected seating model and existing installed include setup. No solver.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace, git diff --check and zdev check base --format json. Add only the small useful public-behavior checks named above, reusing existing styles.

## Result

Added bounded candidate-only wedding growth example using public literal and source-edit APIs. Deterministic guest/group mappings preserve records, comments and unrelated bytes while scaling tables and capacities.

Validation:

- Independent Astra/high verification PASS on W1d24c26661d37f80; Rustfmt, Clippy, workspace/example tests, diff and zdev checks passed.
- Public factor-two candidate independently reproduced and instance checked; private factor-two cases24guests8tables and300guests32tables reconciled for mappings, source preservation, unchanged inputs, successful instance checks and refusal behavior.
