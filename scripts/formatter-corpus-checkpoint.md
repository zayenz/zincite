# Formatter corpus checkpoint

After base-027, the complete available corpus has no recorded preservation,
reparse or second-pass defects among 6,345 completed syntax-clean checks.
Acceptance remains blocked by 24 inputs with confirmed syntax gaps and
27 checker timeouts. This checkpoint reconciles every
discovered path and checks selected complete model/data pairs and a local include
closure with MiniZinc 2.10.1, without solving.

## Evidence reused and rechecked

The checkpoint source revision is
`5acf74a81e3ced56533102415ba5dcef7c4bcce9`. The full base-027 run remains under
ignored `target/corpus/base027/`; it used the following command and a ten-second
limit for each checker process:

```sh
python3 scripts/check-corpus.py --challenge /private/tmp/portfolio-150-mzn-challenge-a844 --local ~/minizinc --supplement /private/tmp/zincite-base023-mznc2026-probs --output target/corpus/base027
```

The current checker SHA-256 is
`9796fba7a22564f0ada8c013c91a2d8e20633ace18e34af0134088ce8d40bf49`;
the formatter CLI SHA-256 is
`b30668962c739571a0ffef00a38e33a1aebf6d41167e94fee3956d52767492f9`.
Both executable digests and both formatter source digests match
`base027/validation.json`. Fresh enumeration reproduces all 6,417 paths, and
fresh SHA-256 checks of every original match the per-file records. The full scan
was reused because its executable and input scope is unchanged. The separately
identified `base027-initial` run is not the final checkpoint evidence.

Fresh reconciliation, compiler commands, original/staged hashes, statuses and
verbose dependency logs live under ignored `target/corpus/base028/`:
`reconciliation.json`, `compiler-checks.json`, and the `formatted/` and
`reductions/` directories. All 6,417 originals were checked again after the
compiler probes; no external original changed.

## Inventory and outcomes

The public Challenge archive revision is
`a8448864fc56162583f24aaf9c25653d93f83765`. The runner includes ignored files and
nested repositories under `~/minizinc`, skips Git metadata, and avoids repeated
directory traversal. It records one existing directory alias/cycle, with no
inventory read error or missing tracked archive input.

| Source | Files |
| --- | ---: |
| Complete Challenge Git archive | 2,040 |
| Local `~/minizinc` tree | 4,287 |
| Official 2026 supplement | 90 |
| Total paths, including 564 content duplicates | 6,417 |

| Challenge year | Archive `.mzn` | Archive `.dzn` |
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

The Git archive has no 2026 data at this revision. The separate official
supplement supplies 20 models and 70 data files; its counts remain separate from
the archive counts.

| Final outcome | Files |
| --- | ---: |
| Completed syntax-clean preservation/reparse/idempotence checks | 6,345 |
| Previously syntax-clean inputs timed out | 27 |
| Other classified syntax rejections | 36 |
| Model items rejected in default data mode | 4 |
| Invalid UTF-8 | 5 |
| Total | 6,417 |

The checker verifies contiguous token ranges, ordered CST leaves, spellings,
comments, protected bytes and grouping, then reparses and formats again. It
allows only the existing include ordering and permitted trailing commas. The
final run records zero preservation, formatted-parse, coverage or second-pass
failures. All 6,345 completed checks also completed the current syntax lint
rules; the 229,123 warnings are advice, not checker failures or proof of semantic
lint coverage.

All 6,372 previously syntax-clean paths are accounted for: 6,345 completed and
27 timed out. The earlier bounded parse-only checks cover every timeout path
with zero diagnostics and full token/CST coverage; their limit was 30 seconds.
The final scan introduces no new timeout path. It took 423.50 seconds and returned
1 because unresolved outcomes remain recorded. A timeout leaves formatting,
idempotence and lint completion unverified for that input.

## Remaining causes and reduced evidence

The [syntax checkpoint](syntax-corpus-checkpoint.md) classifies all 40 syntax
rejections. Of the 36 other syntax rejections, 24 contain compiler-supported syntax,
eight are malformed or compiler syntax-negative, and four are grammar documents
rather than models. Four further `.dzn` files contain model declarations. The
five invalid UTF-8 inputs remain encoding failures.

| Confirmed unsupported family | Inputs | Existing follow-up |
| --- | ---: | --- |
| Form-feed layout | 4 | [base-052](../.zdev/base/tasks/052-accept-compiler-supported-form-feed-layout-and-escaped-singl.md) |
| Escaped single quote in a string | 1 | base-052 |
| Deprecated omitted function keyword | 14 | [base-053](../.zdev/base/tasks/053-accept-the-four-remaining-compiler-supported-item-and-callab.md) |
| Anonymous enum constructor | 3 | base-053 |
| Standalone equality item | 1 | base-053 |
| Callable annotation capture | 1 | base-053 |

Fresh checks of all six retained complete synthetic models in
[syntax-gap-reproducers.json](syntax-gap-reproducers.json) return compiler status
0 and formatter status 2 with diagnostics and no source output. The omitted
function keyword retains the compiler's deprecation warning. These reductions
confirm the existing follow-up families; this checkpoint establishes no new
formatter defect.

The 27 timeouts comprise 15 groupsplitter, five network_50_cstr, four
project-planning, two proteindesign12 and one community-detection input. The
existing layout-measurement and memory work remains in base-046 and base-047,
with the save-budget gate in base-048. The public project-planning sample remains
stable at 1,052,099 original bytes and 23,037,604 bytes on both formatted passes,
with maximum indentation 276 columns. Stable output growth still needs that
performance work. The existing `base027-after.json`, `base027-rss.json` and
`base027-comparison.json` benchmark records retain the dense-input budget misses;
this documentation checkpoint runs no new benchmark.

## Complete compiler checks

Fresh checks use MiniZinc 2.10.1 build `33348285743`, a 30-second process limit,
and the pinned installed standard-library directory:

```sh
minizinc --instance-check-only --verbose --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc MODEL DATA
minizinc --model-check-only --verbose --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc ROOT_MODEL
```

The first command checks each explicit model/data pair below. The second checks
the `test_bug218` include closure and the six synthetic reductions. Neither
command solves. Standard-library and solver-library sources under that installed
directory remain external and unformatted.

Each formatted check receives only staged positional files and runs from its
stage directory. Every explicitly listed local model, data file and include is
formatted before that check. Verbose logs verify that all processed files belong
to the stage or the installed library; no original local include is used to
complete a formatted check.

| Explicit entry | Original compiler | Formatted compiler | Interpretation |
| --- | --- | --- | --- |
| Local `software/libminizinc/docs/en/examples/jobshop/jobshop.mzn` + `jdata.dzn` | 0 | Unavailable | Data contains unsupported anonymous enum constructors; base-053. |
| Local `software/libminizinc/docs/en/examples/jobshop2/jobshop2.mzn` + `jdata.dzn` | 1 | Unavailable | Original has duplicate assignments; instance checking also reports undefined `jobs`/`tasks`. Data formatting encounters the same enum family. |
| Local `software/libminizinc/tests/spec/unit/regression/github_776.mzn` + `github_776.dzn` | 0 | Unavailable | Test metadata names this data file; its enum constructor blocks formatting; base-053. |
| Challenge `2021/ATSP/atsp.mzn` + `instance5_0p15.dzn` | 0 | 0 | README names `atsp.mzn` as the main model. |
| Challenge `2021/carpet-cutting/cc_base.mzn` + `mzn_rnd_test.01.dzn` | 0 | 0 | `globals.mzn` resolves under the installed library. |
| Repository `tests/fixtures/data-model.mzn` + `data.dzn` | 0 | 0 | Complete repository model/data fixture. |
| Challenge `2008/debruijn_binary/debruijn_binary.mzn` + `02_03.dzn` | 1 | 1 | Both fail the historical `int_search(array[int] of var int,string,string,string)` signature under 2.10.1. |
| Local `software/libminizinc/tests/spec/unit/regression/test_bug218.mzn`, including `test_bug218_inc.mzn` | 0 | 0 | Both local files are formatted and read from the stage. |

The three unavailable formatted pairs stop at formatter diagnostics; the
compiler was not run against a partially formatted pair. Jobshop2's duplicate
assignments and debruijn's historical search arguments are original compiler
failures. They do not establish a Zincite preservation defect. No unavailable
pairing or unresolved local dependency appeared in the four positive entries.
Complete compiler checks for every discovered file would require explicit
pairings and include closures beyond these selected entries; individual-file
preservation results do not supply those dependencies.

## Acceptance boundary

The remaining syntax families already belong to base-052/base-053 under the
open base-026 reconciliation. Performance failures remain with base-046,
base-047 and base-048. Base-050 retains base-026 and base-048 as acceptance
blockers; no duplicate follow-up is proposed. Complete corpus acceptance remains
unverified until those gaps close and the affected inputs pass fresh checks.

This task adds only this report and ignored validation records. It changes no
production code, retained syntax-checkpoint files or external corpus source, and
requires no documentation-only tests. Task-record completion and its zdev check
belong to the coordinator.
