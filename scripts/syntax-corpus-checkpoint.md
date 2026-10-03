# Remaining syntax checkpoint

This investigation checks every model and data file in the complete public
Challenge archive, the official 2026 supplement, and `~/minizinc`, including
ignored files and nested repositories. It changes no production syntax and
copies no external source. The public archive revision is
`a8448864fc56162583f24aaf9c25653d93f83765`; MiniZinc is 2.10.1 build
`33348285743`.

```sh
cargo build --release -p zincite-fmt --example check-corpus-file
python3 scripts/check-corpus.py --challenge /private/tmp/portfolio-150-mzn-challenge-a844 --local ~/minizinc --supplement /private/tmp/zincite-base023-mznc2026-probs --output target/corpus/base026
```

The raw inventory, one-record-per-path results, summary and supplemental syntax
assessments live under ignored `target/corpus/base026/`. Formatting failures are
recorded separately; they do not establish syntax rejection. Each formatter
timeout receives a bounded parse-only public-API check, including contiguous
token ranges and ordered CST leaves. Original hashes are checked again.

## Reconciled results

The full run took 457.47 seconds and returned 1 for recorded failures. It checked
all 6,417 paths: 2,040 public archive files, 4,287 local files and 90 supplemental
files, including 564 content duplicates. No tracked archive input, inventory
entry or available supplement is missing. The archive year counts match the
[recorded full baseline](corpus-baseline.md); the supplement closes the Git
archive's 2026 data gap. All original hashes were checked again after assessment
and remained unchanged.

The normal checker completed syntax and token/CST coverage for 6,381 UTF-8
inputs. Of these, 6,341 parsed cleanly and 40 were rejected. Five other inputs
failed UTF-8 decoding. Formatting timed out after ten seconds for 31 inputs.
The supplemental parse-only checks completed for all 31 under a 30-second limit
(maximum observed 6.24 seconds), with zero diagnostics and complete token/CST
coverage. Thus 6,372
files parse cleanly, 40 have classified syntax rejections and five have encoding
failures; every discovered path has a syntax assessment. No syntax timeout,
crash or unreadable input remains unclassified.

| Syntax classification | Files |
| --- | ---: |
| Compiler-confirmed unsupported forms | 24 |
| Malformed or compiler syntax-negative source | 8 |
| EBNF/output grammar documents, not models | 4 |
| Model items in default data mode | 4 |
| Invalid UTF-8 | 5 |

The 24 unsupported instances comprise 14 omitted-function declarations, four
form-feed inputs, three anonymous enum constructor data files, one standalone
equality test, one escaped-quote test and one annotation-capture test. Every
rejected `.mzn` was checked directly with the target compiler, with no compiler
timeout or unavailable dependency. The data constructor cases use explicit
example/test pairings as described below. Complete compiler acceptance for all
6,417 inputs is outside this syntax investigation.

The full checker also reports 63 ordered preservation failures and 239 differing
second passes, with overlapping causes and 316 distinct failing files overall.
These outcomes remain in the original records. They belong to the existing
formatter tasks, not new syntax families. For example, public
`2015/project-planning/ProjectPlannertest_14_7.mzn` parses without diagnostics
but expands from 1,052,099 input bytes to 33,822,562 first-pass bytes and
32,351,700 second-pass bytes; that is a formatter growth/stability failure.

`syntax-classifications.jsonl` assigns every rejection a classification;
`compiler-rejections.jsonl` retains direct compiler outcomes for rejected
models; `timeout-syntax-recheck.jsonl` retains the bounded parse-only results.
The summary explicitly records supplemental postprocessing. The full result
file retains one row per original path and all initial formatting/lint outcomes.
No source hash or inventory outcome was replaced to improve coverage.

## Confirmed unsupported syntax

[The synthetic reproducers](syntax-gap-reproducers.json) contain complete models
for every confirmed family. They contain no copied corpus text. Write each
`source` value as UTF-8 to a temporary `.mzn` file and run
`minizinc --model-check-only FILE`; no solver runs. Every case exits 0 with the
installed target compiler and produces Zincite parse diagnostics. The omitted
function keyword emits a deprecation warning. JSON encodes the form-feed byte
as `\f` and preserves the escaped quote spelling.

| Family | Minimal source form | Follow-up |
| --- | --- | --- |
| Form-feed layout | `int: x=1;` then U+000C then `solve satisfy;` | Lexer compatibility |
| Single-quote string escape | `output ["A \'quote"];` | Lexer compatibility |
| Deprecated omitted function keyword | `int: identity(int: x) = x;` | Parser compatibility |
| Anonymous enum constructor | `enum E; E=_(1..2);` | Parser compatibility |
| Standalone equality item | `var int: x; x == 1;` | Parser compatibility |
| Callable annotation capture | `predicate p() ann: anns = tag in anns;` | Parser compatibility |

The follow-ups must retain spelling, useful declaration/expression nodes,
precise ranges and error recovery, and make formatting/lint traversal work for
these forms. They must reject nearby malformed forms. They do not authorize
general historical grammar support or semantic rewrites.

## Inputs that do not establish a syntax gap

Grammar production documents have `.mzn` extensions but contain EBNF, not models.
The target compiler rejects them. The tuple interpolation negative test and
corrupted regression source also fail compiler syntax checks. The reserved
`int(...)` call in another regression file is rejected by 2.10.1; the synthetic
`int: x=int(1.0); solve satisfy;` probe confirms this is not an accepted cast.
Negative metadata alone never excuses a parser failure: the `test_same` and
annotation-capture tests are compiler-positive, despite being software tests.

Four data files contain model declarations. Default `.dzn` assignment-only mode
continues to reject them. Remaining malformed local experiments and the public
comma-interpolation model retain diagnostics. These are not opaque accepted
syntax. The five invalid UTF-8 files stay explicit encoding failures.

The jobshop data pair and the `github_776` test's declared `extra_files` pair
pass `--model-check-only`. The second jobshop example reports duplicate
assignments, rather than a syntax error: its anonymous constructor syntax is
still demonstrated by the complete synthetic model. Individual-file parsing
does not resolve includes or need data. Missing pairings cannot prove complete
compiler acceptance, and this checkpoint makes no such claim.

## Acceptance gate

The proposed split retains this investigation report and keeps base-026 open
until the lexer/parser children finish and a fresh reconciliation verifies the fixes. Base-050 already depends on
base-026; no acceptance claim can bypass unresolved syntax gaps. Formatting
preservation, second-pass and growth failures remain with base-027 and the
existing performance tasks.

Validation: workspace fmt, clippy with `-D warnings`, workspace tests and
`git diff --check` pass. This task adds no production code or tests.
