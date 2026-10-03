# Formatter corpus checkpoint

The sections before the base-080 assessment retain historical base-028 evidence.
The [current assessment](#base-080-current-system-assessment) below supersedes
their open-gap counts while preserving the earlier commands and outcomes.

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

## Base-080 current-system assessment

This assessment uses MiniZinc 2.10.1 build `33348285743`. The current compiler
system determines source eligibility; historical Challenge inputs rejected by
that system remain classified compiler failures. These results assess source
processing and selected complete instances. They do not establish full thesis
lint coverage or close the separate save and batch performance gates.

The retained base-079 inventory contains 6,417 paths: 2,040 Challenge archive
files at `a8448864fc56162583f24aaf9c25653d93f83765`, 4,287 local files and 90
files in the official 2026 supplement. The archive year counts above remain the
recorded denominator. The supplement still contains 20 models and 70 data files;
its instances do not become archive instances by combining the inventories.

Initial hashing found 5,595 originals with identical base-079 bytes and hashes;
822 archive paths were absent from the old external location. Recovery created a
separate checkout at the exact recorded revision, leaving that old location
untouched. An independent check now verifies all 2,040 replacement source paths,
509,489,692 bytes and every historical size and SHA-256. The local and supplement
inputs remain available. Failed downloads and the initial missing-path evidence
remain recorded rather than being replaced by a claim that the old originals
were restored.

All 27 retained gap rows now have fresh classifications: six compiler-positive
large inputs with complete fidelity checks, eight compiler syntax-negative or
malformed inputs, four grammar documents, four data files containing model
declarations and five incompatible UTF-8 inputs. MiniZinc diagnostics corroborate
the grammar and malformed classifications; test metadata alone does not determine
them. The recovered train-scheduling model is compiler syntax-negative at its
tuple-pattern lambda. No demonstrated compiler-positive syntax rejection in this
assessed set requires a parser repair.

The data syntax probes use an explicitly recorded temporary `solve satisfy;`
model solely to make the compiler parse a data file. This is not a model/data
pairing. Missing declarations in that context do not prove a source failure.
Four declaration-bearing data files produce the compiler's data-mode errors;
Zincite keeps its assignment-only data mode. The five encoding failures retain
exact failing UTF-8 byte offsets. MiniZinc's acceptance of their byte-level
string syntax does not make them valid UTF-8 inputs for Zincite.

### Known instances and staged closures

Fresh model/instance-check-only results are retained separately from Gecode
compilation. Jobshop and `github_776`, which were historically unavailable to
the formatter, now format and pass those checks. Every formatted compiler check
uses the complete observed local include closure under its stage directory;
verbose dependency logs confirm that remaining sources belong to the installed
standard library. External originals and installed library sources stay unchanged.

Compile checks use `--compile` with the explicit installed IDE
`solvers/gecode.msc` configuration, Gecode 6.4.0, the same standard-library
path and identical settings before and after formatting. The first six known entries use
180-second compile limits; recovered ATSP and carpet-cutting use 300 seconds.
All attempts retain separate FlatZinc/output-model artifacts. No solver runs.
Earlier `--solver gecode` alias attempts remain recorded; the explicit repeats
remove ambiguity among the installed configurations.

| Known entry | Original / formatted compile status | Raw FlatZinc comparison |
| --- | --- | --- |
| Jobshop + `jdata.dzn` | 0 / 0 | Only generated command-invocation comment differs. |
| Jobshop2 + `jdata.dzn` | 1 / 1 | Original duplicate assignments remain compiler errors. |
| `github_776` metadata pair | 0 / 0 | Only generated command-invocation comment differs. |
| Repository model/data fixture | 0 / 0 | Only generated command-invocation comment differs. |
| Historical debruijn + `02_03.dzn` | 1 / 1 | Historical search signature remains incompatible. |
| `test_bug218` and its local include | 0 / 0 | Only generated command-invocation comment differs. |
| ATSP + `instance5_0p15.dzn` | 0 / 0 | Only generated command-invocation comment differs. |
| Carpet-cutting + `mzn_rnd_test.01.dzn` | 0 / 0 | Only generated command-invocation comment differs. |

For each successful known-instance compile, physical raw FlatZinc files differ
at line 3,
where MiniZinc records the invocation's original/staged input and output paths.
Every other line is byte-identical. The raw files and complete differences are
retained without normalization; this is a concrete comparison, not a general
semantic-equivalence proof. Missing data, include dependencies and usable
pairings outside these known entries remain availability limits. No additional
pairing is inferred from nearby filenames.

### Large-input compiler output investigation

All six newly compiled large entries use simple explicit pairs: the sole
model in each exact-year problem directory, with parameter declarations matching
the data assignments. Each original and complete formatted closure compiles
with the same explicit Gecode settings. The five community/network entries have
a 300-second limit; the 64 MB proteindesign entry has a 600-second limit. Their full
fidelity checks preserve token spellings, structure and protected bytes, with
clean parsing/reparsing and stable second-pass output. These are correctness
checks. Some subset checks ran alongside low-CPU archive downloads; the last
large check and complete corpus ran after every recovery handle was reaped.
None supplies a performance-budget conclusion.

`MODEL1507180015.dzn` changes only the generated invocation comment. Polblogs
changes that comment and generated identifiers: a full lexical comparison finds
a consistent one-to-one mapping for every identifier occurrence, with every
other text byte retained. That observation is not a general equivalence proof.
The other three network outputs also change declaration ordering, generated
identifiers and some lowered output details; a positional identifier mapping
does not account for all differences. Complete raw files and diffs stay intact.

Repeated unchanged-original compilations of those three network inputs produce
identical bodies. For `MODEL2204150001.dzn`, copied original bytes at staged
paths also preserve the body, as does formatting only the model. Formatting only
the data produces the body differences. These controls locate the triggering
change without identifying a compiler internals cause or claiming semantic
equality. The source-fidelity gates pass; raw FlatZinc equality is not established
for these entries. In `MODEL2204190004.dzn`, the formatted output has one fewer
array declaration and line, while each constraint-predicate count matches.
Counts alone do not prove equivalence.

The recovered 64 MB proteindesign entry compiles before and after formatting.
Its 10 changed FlatZinc lines are the invocation comment and nine lines carrying
five generated-identifier renamings. A whole-body lexical check verifies a
consistent one-to-one mapping across all 479 identifiers and every reference,
with every other text byte identical. Raw artifacts and the full diff remain
unchanged; no general semantic-equivalence claim follows.

### Complete syntax/format corpus

The fresh run retains all 6,417 original paths, including 564 content duplicates.
Every final input size and SHA-256 matches base-079. The 1,800-second first
campaign completed 1,993 checks, then censored `2TRX` with only 1.308 seconds
remaining and left 4,423 inputs unobserved. Its status 1, full streams, censored
row and all unobserved rows remain under `target/corpus/base080-final/`.
This cutoff is not a demonstrated source-processing failure.

A private adapter reused the unchanged runner for exactly those 4,424 pending
keys, with 600 seconds per file and a fresh 1,800-second campaign limit. It
completed every selected check under `target/corpus/base080-resume/`. Completed
first-campaign rows, including all six original large gaps, were not repeated.
The adapter and exact selector are retained under `target/corpus/base080/pinned/`
and `resume-selection.json`.

Final merged records under `target/corpus/base080-complete/` preserve both
attempts for the censored file: 6,418 checker invocations cover 6,417 unique
inputs. No final timeout, unobserved input, preservation failure, reparse failure
or second-pass failure remains. Both individual campaigns return 1 because they
retain classified negative inputs; the first also retains its cutoff.

| Final classification | Inputs |
| --- | ---: |
| Syntax-clean coverage, preservation, reparse and idempotence | 6,396 |
| Compiler syntax-negative or malformed | 8 |
| Grammar documents | 4 |
| Model declarations in assignment-only data mode | 4 |
| Incompatible UTF-8 | 5 |
| Total | 6,417 |

The five encoding exclusions remain coverage limits, with exact decoder offsets;
they are not classified as compiler-broken from a missing model or pairing.
All 6,396 clean checks complete the current default syntax lint rules. Their
235,909 warnings are advice, not failures or proof of complete thesis execution.
The final rehash covers all 6,417 originals; none changed. The old archive's
initial available/missing state also remains unchanged.

For a full repeat in a fresh output directory, the existing runner can use more
campaign headroom while keeping a finite limit:

```sh
python3 scripts/check-corpus.py --challenge /private/tmp/zincite-base080-challenge-a844 --local /Users/zayenz/minizinc --supplement /private/tmp/zincite-base023-mznc2026-probs --output target/corpus/base080-repeat --binary target/benchmarks/base079/after/check-corpus-file --timeout 600 --campaign-timeout 3600
```

Omitting `--rules` selects syntax/format checking; this is not a thesis-rule
execution campaign. Any future cutoff must retain censored and unobserved inputs.

Raw assessments, complete compiler streams and statuses, staged closures,
FlatZinc artifacts, original hashes and availability reconciliation live under
ignored `target/corpus/base080/`. They retain the earlier compiler aliases and
separate wall/child-CPU observations. Setup failures remain in
`/private/tmp/zincite-base080-validation.json`; archive recovery attempts,
including failed downloads, remain in `/private/tmp/base080-corpus-recovery.json`
and its referenced artifacts.
For example, the last large original compile took 88.686 seconds wall and
10.159 seconds child CPU; the evidence does not identify the cause of that gap.
All earlier base-079 reports and timeouts remain intact. Exact retained source
and binary hashes were verified; no production source changed, so this reporting
work adds no regression tests or Cargo rerun. No demonstrated valid-source
rejection remains in the assessed 27-gap set. Base-050 still retains its separate
semantic and save/batch performance acceptance requirements.
