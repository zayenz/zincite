# Full corpus baseline

This document retains the historical base-023 baseline and base-024 recheck.
The [base-080 checkpoint](formatter-corpus-checkpoint.md#base-080-current-system-assessment)
now reconciles all 6,417 inputs: 6,396 clean syntax/format fidelity results and
21 checked negative or encoding-limited inputs. All six former large-data
timeouts pass current MiniZinc eligibility, fidelity and staged compilation.
The final accounting preserves the first campaign cutoff and its bounded
continuation; it does not claim complete thesis lint or performance acceptance.

This release-library acceptance run was collected on 2026-09-30. It checks syntax,
lossless coverage, default formatting and the two current lint rules. It does not
establish compiler/type/solver acceptance or check EditorConfig combinations.

```sh
cargo build --release -p zincite-fmt --example check-corpus-file
python3 scripts/check-corpus.py --challenge /private/tmp/portfolio-150-mzn-challenge-a844 --local ~/minizinc --supplement /private/tmp/zincite-base023-mznc2026-probs --output target/corpus/base023
```

The complete public Git archive is revision
`a8448864fc56162583f24aaf9c25653d93f83765`: all 2,040 tracked inputs are present
(436 models and 1,604 data files). The official 2026 supplement adds 20 models and
70 data files. The local filesystem contributes 4,287 files, including ignored
files and nested repositories. Every one of the 6,417 discovered paths is checked,
including content duplicates. A repeated Shackle queries-directory symlink is
recorded as an alias; it contains no model/data inputs. No external file is copied
into this repository or rewritten. The runner compares each original hash after
its check, and keeps local paths/snippets in ignored reports.

| Archive year | Models | Data |
| --- | ---: | ---: |
| 2008 | 10 | 95 |
| 2009 | 11 | 104 |
| 2010 | 25 | 85 |
| 2011 | 24 | 95 |
| 2012 | 24 | 94 |
| 2013 | 30 | 90 |
| 2014 | 32 | 85 |
| 2015 | 28 | 90 |
| 2016 | 24 | 95 |
| 2017 | 24 | 95 |
| 2018 | 20 | 100 |
| 2019 | 28 | 90 |
| 2020 | 24 | 95 |
| 2021 | 28 | 75 |
| 2022 | 20 | 85 |
| 2023 | 20 | 89 |
| 2024 | 24 | 72 |
| 2025 | 20 | 70 |
| 2026 | 20 | 0 |

The Git archive lacks 2026 data at this revision. The
[official published supplement](https://www.minizinc.org/challenge/2026/mznc2026_probs.tar.gz)
closes that available-instance gap; its duplicate models remain individual checks.

| Local subtree | Models | Data |
| --- | ---: | ---: |
| archive | 1 | 1 |
| challenge-models | 38 | 763 |
| competitions | 14 | 0 |
| library | 2 | 0 |
| models | 26 | 69 |
| projects | 94 | 1337 |
| scratch | 3 | 0 |
| software | 1907 | 32 |

## Hand-checked mismatch

`2008/debruijn_binary/debruijn_binary.mzn` in the public archive demonstrates two
real formatter problems. The source places a four-line comment group before a
conjunction token, following `forall(j in 1..n) (...)`. The first output places
that conjunction before the comments. This changes comment/token attachment and
fails the ordered preservation check even though the spelling counts still
match. Formatting that output again changes those four lines from column zero to
four-space indentation. Both formatter invocations return 0 and reparse; they
produce different bytes. The local first/second outputs are
`/private/tmp/base023-debruijn-{first,second}.mzn`; external input stayed unchanged.
This task records the failures and leaves production formatting untouched.

## Reconciled results

The initial full run took 486.59 seconds with a ten-second limit per file and
returned 1 because it found failures. All 6,417 paths have one result, including
564 content duplicates; no tracked archive file or inventory entry is missing.
Token/CST coverage passed for all 6,380 completed UTF-8 child runs. The remaining
37 inputs were five encoding failures and 32 explicit timeouts. Parse, formatting
and lint completed for 6,108 inputs; lint emitted 191,910 warnings across the two
current rules. No child crash or formatted-output parse error occurred.

The first checker omitted grammar-optional GeneratorList trailing commas from
its punctuation normalization and reported 387 preservation failures. The
[2.10.1 grammar notation](https://docs.minizinc.dev/en/2.10.1/spec.html#notation)
allows the final separator in non-empty lists, including comprehension tails.
After correcting that bounded allowance, every one of those 387 records was
rechecked in 2.78 seconds; `preservation-recheck.jsonl` retains the supplemental
results. Other initial outcomes remain unchanged. The final spelling counts
match in every completed formatting check; 63 files still change ordered CST or
comment attachment.

| Final cause | Files |
| --- | ---: |
| Parse failure requiring assessment | 268 |
| Data extension containing model items | 4 |
| Invalid UTF-8 | 5 |
| Per-file timeout | 32 |
| Ordered structure/comment preservation | 63 |
| Different second formatting pass | 224 |

Causes overlap: 534 distinct files have at least one failure. The archive accounts
for 213, the local tree for 314, and the official supplement for seven. These
figures include duplicates and deliberately avoid claiming every parse failure
is a Zincite bug or every compiler-negative test is invalid syntax. Known valid
unsupported forms remain parse failures requiring assessment.

Opening compiler-test metadata was reconciled separately after the run, using
explicit `!Error` expectations rather than the initial narrower `MiniZincError`
name check. There are 107 negative-test hints, all local; 15 also have Zincite
parse errors. Some compiler negatives describe type or evaluation errors and
parse successfully. The hints change no acceptance outcome. Original hashes were
rechecked for all 6,417 records, including timeouts; none changed. The final
metadata/coverage counts preserve exactly one result per originally inventoried
path. `target/corpus/base023/{inventory.json,results.jsonl,summary.json}` holds the
local durable evidence and explicitly records both supplemental steps.

A three-file smoke check processes a malformed declaration first, then a labelled
constraint and a data assignment. All three results are retained, the latter two
complete lint/formatting, and the command returns 1. Focused child probes also
pass for expanded trailing-comma lists and a final include without a written
semicolon that moves into a sorted group. The required workspace fmt, clippy and
test checks pass. This baseline does not change production parser, formatter or
lint behavior; subsequent bounded compatibility tasks own those fixes.

## Protected-byte supplemental check

The initial signature check normalized whitespace and optional punctuation even
inside skipped/off spans. The checker now separately compares the ordered byte
slices returned by the formatter's protected-range API, including boundary trivia
and the number of ranges. A mismatch joins `format_preservation`; ordinary layout
allowances cannot excuse it. Focused checks detect changed skipped whitespace,
a removed protected comma and a removed range. Actual skip output and off/on
output with CRLF boundaries match their original protected slices exactly.

A complete byte scan of every original input found zero `zincite-fmt:` markers;
all 6,417 recorded hashes still match. Consequently there are no directive or
protected-span candidates requiring supplemental formatting. The zero-candidate
inventory is `target/corpus/base023/protected-inventory.json`, and
`protected-recheck.jsonl` is explicitly empty. Each original result remains in
place, with `protected_bytes = not_applicable_no_directives`; the summary records
this supplemental scan. Previous check outcomes and the final failure totals
remain unchanged. This distinction records the newly added invariant without
claiming the original full run checked protected bytes.

## Compiler-supported source forms recheck

The base-024 recheck selected all 64 prior parse-failure records outside the local
software tree, including duplicates and mislabelled data files. It was a focused
recheck, not another full-corpus acceptance run. External hashes stayed unchanged.
The complete records and classification counts are in ignored
`target/corpus/base024/{results.jsonl,summary.json}`.

Forty-two files now parse and reach formatting/reparse and current lint: 19 public
archive records and 23 local records. Their spelling, ordered structure and
protected-byte checks pass. Ten connect copies and two stripboard models retain
second-pass differences; the existing formatter compatibility work in base-027
owns those layout failures. None was excluded from the report.

The remaining 22 parse failures comprise:

| Classification | Records |
| --- | ---: |
| Deprecated function declarations without `function` | 9 |
| Historical form-feed whitespace | 4 |
| Model declarations in default data mode | 4 |
| Malformed comma-separated interpolation | 1 |
| Missing array-entry comma, including a copied model | 2 |
| Field outside its record literal | 1 |
| Unclosed call | 1 |

MiniZinc 2.10.1 accepts minimal omitted-function and form-feed probes; those remain
explicit compatibility families for base-026. It rejects minimal comma
interpolation, missing-entry-comma and misplaced-field probes. The four data-mode
cases still require model grammar rather than silently changing `.dzn` behavior.
Software/library unnamed or annotated parameter forms remain outside this recheck
and belong to base-025.

The installed MiniZinc build `33348285743` accepts original and formatted minimal
models for infinity bounds, bare `:: output`, singleton record values, annotations
on local declarations and local/global `==` declaration initializers. All six
`--model-check-only` pairs returned 0 with no diagnostics; no solver ran. Focused
public checks retain CST/token ranges, spelling and comments, reject malformed
records and reserved-output placements, retain data-mode rejection, and reach
nested naming/constraint-label lint checks. Required workspace fmt, clippy and
tests pass. This recheck does not claim all historical syntax or formatter corpus
failures are resolved.
