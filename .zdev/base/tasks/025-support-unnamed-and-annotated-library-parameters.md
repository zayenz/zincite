+++
schema_version = 1
id = "base-025"
key = "library-historical-syntax"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-023", "base-024"]
+++
# Support unnamed and annotated library parameters

## Outcome

Standard-library declarations with unnamed parameters and parameter annotations parse, format and expose usable signature nodes.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Confirmed examples in software/libminizinc/share/minizinc are std/diversity.mzn (annotation diversity_aggregator(string)) and linear/fzn_cumulative.mzn (var int: b :: promise_ctx_monotone). The original parser rejects these parameter forms. Start in parameter parsing and callable formatting; validate against the installed 2.10.1 library/compiler.

## Boundaries

- Limit implementation to these two confirmed signature families; remaining historical/library syntax is classified by the integration checkpoint.

## Done when

- [ ] Support unnamed callable/annotation parameters and parameter annotations in their compiler-supported positions, retaining type, annotation, name presence and exact source ranges.
- [ ] Format both families stably and let naming skip absent declaration names; document the grammar extensions and retain diagnostics on malformed signatures.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use minimal positive/negative signature examples, inspect original/formatted 2.10.1 acceptance, and rerun the affected standard-library files. Do not absorb unrelated corpus failures.
