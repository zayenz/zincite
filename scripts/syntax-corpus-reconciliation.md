# Syntax corpus reconciliation

On 2026-10-01, Zincite at commit `767d3dd6172353d6dece3f6c09352bcb668e1e91` checked all
6,417 available model/data paths after the base-052 lexer and base-053 parser
fixes. Of these, 6,396 parse cleanly, 16 retain classified syntax diagnostics,
and five fail UTF-8 decoding. Every UTF-8 input has complete contiguous token
coverage and ordered CST leaves. No compiler-positive unsupported syntax family
or unassessed input remains in this inventory.

The [previous syntax checkpoint](syntax-corpus-checkpoint.md) recorded 24
unsupported instances. All 24 now parse cleanly: four form-feed files, one
escaped-single-quote file, fourteen omitted-function files, three anonymous enum
constructor data files, one standalone-equality file and one annotation-capture
file. The six retained [synthetic reductions](syntax-gap-reproducers.json) pass
fresh MiniZinc 2.10.1 model checks, lossless parsing and stable formatting.
No further syntax implementation task is needed from this reconciliation.

## Inventory

The run checked 2,040 Challenge archive files, 4,287 local files and 90 files
from the retained official 2026 supplement, including 564 content duplicates.
The Challenge revision is `a8448864fc56162583f24aaf9c25653d93f83765`.
Fresh discovery matches the previous 6,417-path inventory exactly: no added,
missing or changed source file. The archive has no missing tracked input.

| Archive year | Models | Data files |
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

The archive's 2026 tree has no data files. The supplement contains 20 models
and 70 data files and covers the retained official download separately. Local
discovery includes ignored files and nested repositories. It skips Git metadata
and visits each physical directory once; the one retained directory-alias note
introduces no missing input. Fresh discovery after checking matches the initial
inventory, and every original SHA-256 and permission mode is unchanged.

## Syntax assessments

The normal checker completed 6,389 UTF-8 inputs: 6,373 parse cleanly and 16
retain diagnostics. Another 23 inputs reached the ten-second full-check limit.
A separate public-API parse-only checker completed every one under a 30-second
bound, with zero diagnostics and full token/CST coverage; its maximum observed
time was 2.61 seconds. These checks complete the syntax
assessment without replacing the original timeout records.

| Remaining classification | Files | Evidence |
| --- | ---: | --- |
| Malformed or compiler-negative source | 8 | Fresh compiler syntax errors, including malformed arrays/calls/interpolation, the reserved `int(...)` call and corrupted source |
| Grammar/output production documents | 4 | EBNF documents with `.mzn` extensions; the compiler rejects them as models |
| Model declarations in data mode | 4 | Clean only in explicit model parsing; the compiler rejects variable declarations when paired as data |
| Invalid UTF-8 | 5 | The unchanged 2017 crossword data files fail decoding at recorded byte offsets |

All sixteen syntax rejections have fresh MiniZinc 2.10.1 build `33348285743`
compiler evidence. Data-file checks use a synthetic complete model containing
`solve satisfy;`; they do not infer a model/data pairing. No compiler timeout or
unavailable dependency prevented rejection classification. Compiler-test metadata
was only a hint; the compiler results and source form establish the categories.
Detailed classifications and encoding offsets remain in the local raw reports.

## Formatting and acceptance limits

All 6,373 clean inputs completed by the full checker preserve token spellings,
ordered structure and protected text, reparse without diagnostics and produce
identical second-pass output. This run records zero preservation failures and
zero second-pass differences among those completed checks. Its 23 full-check
timeouts remain unresolved formatting/completion outcomes. Clean parse-only
results do not establish successful formatting for those inputs.

The runner exits 1 because it retains all 44 affected outcomes: 23 timeouts,
16 classified syntax rejections and five encoding failures. The raw records
retain every original result. Intervening committed formatter work explains why
these results differ from the previous checkpoint; this report does not overwrite
that checkpoint or claim that a syntax fix resolved its formatter failures.

This closes the syntax investigation for the available inventory. It does not
establish whole-corpus compiler acceptance, successful formatting of the timed-out
inputs, or complete thesis-rule analysis. The full checker runs only the two
default lint rules; it completed linting on 6,373 inputs and reported 234,289
warnings. Warnings are completed lint outcomes, not syntax failures. No solver ran.
The broader formatter/performance and semantic-lint acceptance gates remain open.

## Repeating the check

```sh
cargo build --release -p zincite-fmt --example check-corpus-file
python3 scripts/check-corpus.py --challenge /private/tmp/portfolio-150-mzn-challenge-a844 --local ~/minizinc --supplement /private/tmp/zincite-base023-mznc2026-probs --output target/corpus/base026-reconciliation
```

The full replay took 402.85 seconds. Local evidence stays under
ignored `target/corpus/base026-reconciliation/`:

- `baseline-manifest.json`, `inventory.json`, `results.jsonl`, `summary.json` and
  `run-command.json` retain discovery, hashes/modes, commands and all runner outcomes.
- `parse-only.rs`, `parse-build.json` and `timeout-syntax-recheck.jsonl` retain the
  bounded public parse checks used to complete syntax assessment.
- `compiler-rejections.json`, `synthetic-reductions.json`,
  `syntax-classifications.jsonl`, `syntax-assessment.jsonl` and `reconciliation.json`
  retain rejection evidence and the complete per-path syntax reconciliation.

Validation: the release checker and parse-only helper build successfully;
`zdev check base --format json` passes. This investigation changes documentation,
not Rust code or tests. External inputs and the six unrelated documentation paths
retain their original bytes and modes.
