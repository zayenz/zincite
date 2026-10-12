# Corpus acceptance

Base-050 consolidates compatibility, bounded lint support and release measurements.
Current compatibility, bounded directory/compiler controls and final named
performance checks pass within the qualifications below. Historical results
retain their checked revisions.
Warnings are successful lint execution; zero warnings is not an acceptance goal.

## Inventory and reproduction

The recorded Challenge revision is
`a8448864fc56162583f24aaf9c25653d93f83765`, covering 2008–2026. The checked
inventory contains 2,040 archive files, 4,287 private local files and 90 files
from the official 2026 supplement: 6,417 paths, including 564 content duplicates.
The supplement contains 20 models and 70 data files. It remains separate from
the archive, whose 2026 tree has no data. The recovered archive's current Git
revision and tracked-file probes return 128, so its revision is inherited from
the pinned historical inventory through complete path/content identity. This
is not a fresh Git revision check. Missing published data or dependencies
prevent a claim of full available-instance coverage.

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

Local discovery includes ignored files, nested repositories, archive and software
trees. Originals stay read-only. Raw inventories, per-file classifications,
include closures and private diagnostic locations remain in ignored local
reports; no private source is reproduced here.

The syntax/format runner uses a release `check-corpus-file` example, finite
per-file and campaign limits, temporary output and an unchanged-original rehash:

```sh
cargo build --release -p zincite-fmt --example check-corpus-file
python3 scripts/check-corpus.py \
  --challenge "$CORPUS_CHALLENGE" --local "$CORPUS_LOCAL" \
  --supplement "$CORPUS_SUPPLEMENT" --output target/corpus/base050 \
  --binary target/release/examples/check-corpus-file \
  --timeout 600 --campaign-timeout 3600
```

The three `CORPUS_*` variables name the read-only roots in the retained inventory.
Omitting `--rules` checks syntax, source preservation, formatting stability and
syntax lint execution. It does not run a full semantic instance campaign.
Semantic controls use the explicit installed MiniZinc standard-library path;
ordinary Zincite commands never invoke a compiler or solver.

## Source preservation and compiler compatibility

The completed base-080 report at source revision
`209b2d81c8d5d0630c28a51ab5ef926b83a63213` retains 6,418 invocations over
6,417 unique inputs, including an initial campaign cutoff and its exact resumed
selection. Final outcomes are:

| Classification | Inputs |
| --- | ---: |
| Clean parsing, preservation, reparsing and second-pass stability | 6,396 |
| Compiler syntax-negative or malformed | 8 |
| Grammar documents | 4 |
| Model declarations in assignment-only data mode | 4 |
| Incompatible UTF-8 | 5 |
| Total | 6,417 |

No final timeout, Unobserved input, preservation failure or second-pass difference
remains in that report. All originals were rehashed unchanged. These historical
receipts live under `target/corpus/base080-complete/`; they do not alone establish
current fidelity after subsequent syntax and layout changes.

The final base-050 run at `1fa808971fd5d466ec35c6018352eb6044823ca3`
checks all 6,417 inputs in 291.463 seconds with the current release checker
SHA-256 `d7b7b1a3f6e7d6118fecd73fb3f577cc4587388f05f80580f0a21e503d80dbe0`.
It completes parsing, source preservation, reparsing and stability on 6,401
inputs. The remaining 16 inputs retain eight compiler-negative/malformed, four
grammar-document and four data-mode classifications. The five historical
encoding failures now complete with opaque comment bytes preserved. There is
no encoding error, timeout, Unobserved input, preservation failure or second-pass
difference. All 6,417 originals and the checker remain unchanged; discovery
reports no inventory error. Its empty missing-tracked list does not establish
a fresh tracked-file check because the archive VCS probe was unavailable.
The command exits 1 for
the classified negative inputs and records 235,909 syntax warnings. Current
per-input rows and hashes live under `target/corpus/base050-current/`.

The six formerly unsupported corpus syntax families are implemented. The
[syntax reconciliation](syntax-corpus-reconciliation.md) records all 24 affected
inputs and six compiler-supported reductions. `.dzn` remains assignment-only;
code and literals use UTF-8 while opaque comment bytes are preserved. The five
historical encoding outcomes above now pass under that contract. JSON data and solver output are outside the target formats.
The [reference](../docs/reference.md#syntax-support) documents compiler-supported
extensions and grammar/compiler differences without claiming whole-language
validation.

Retained compiler assessments use MiniZinc 2.10.1 build `33348285743`, explicit
Gecode 6.4.0 configuration, finite limits and no solving. Known model/data pairs
and complete formatted include closures retain original and formatted checks,
raw FlatZinc artifacts and physical differences. Successful Jobshop,
`github_776`, repository fixture, `test_bug218`, ATSP and carpet-cutting pairs
differ only in the compiler's generated invocation comment. Jobshop2 retains
its original duplicate-assignment failure; historical debruijn retains its
incompatible search signature. No nearby-file Cartesian product supplies new
pairings. Missing data, dependencies and usable invocations remain availability
limits.

Six large compiler-positive inputs also pass complete fidelity checks and
original/formatted compilation. Some FlatZinc outputs differ in generated
identifiers or declaration order. Whole-body comparisons account for identifier
renaming in polblogs and proteindesign; three other network cases retain body
differences that these comparisons do not explain. Repeated original and staged
controls locate those differences at data formatting. Raw equality and semantic
equivalence are not established for those three entries; no demonstrated source
preservation failure follows from them. The [formatter checkpoint](formatter-corpus-checkpoint.md#base-080-current-system-assessment)
retains the investigation and its limits. Compiler acceptance supplements token,
structure, protected-text and stability checks; it does not prove semantic
facts or equivalence.

Final no-solve controls recheck the repository model/data fixture, the retained
`test_bug218` local include closure and the known Challenge ATSP pair with the
current formatter. All three originals and complete formatted closures compile
successfully with identical settings. Raw FlatZinc differs only at line 3, the
generated invocation comment; output-model bytes are identical. All 846 pinned
source, library and tool files remain unchanged. Private receipts and full
artifacts are indexed by `/tmp/zincite050-compiler-controls.json`. These three
controls establish their named instances; the other known classifications and
large-input investigations remain historical evidence.

## Bounded semantic execution

The adopted [semantic policy](../docs/semantic-processing-plan.md) promises scoped
source identity, common structural types and supported first-order call facts,
with bounded numeric, definedness, iteration, definition and search consumers.
It excludes arbitrary evaluation, recursive execution, relation inversion,
general theorem proving and exhaustive library-body interpretation. A blocked
excluded operation stays located and Limited. Unknown can complete a supported
analysis without a finding; promised-support defects cannot become exclusions.

The completed full capture under
`target/benchmarks/base083/resume-final-thesis14-all28/` belongs to
`87fcfc39a36a1ed49e23479b9aa584a48077543d`. Both presets account for all 6,417
roots, with no Unobserved rows or malformed protocol rows. Each retains 1,179
structurally complete roots and 1,173 complete resolved roots. Thesis records
970,137 warnings, 383 errors and 49,822 limitations; all records 1,222,329
warnings, 383 errors and 93,918 limitations. The complete capture retains every
Completed/Limited/Inapplicable/NotRun partition, source/dependency classification
and original hash. These counts describe that revision, before the last three
lint repairs; they are not current semantic observations.

Current repaired-source controls under
`target/benchmarks/base083/three-repair-original-controls/` select three distinct
complete resolved originals: neighbours-rect, mondoku-gcc-model-balance and
globals_all_equal_int. They exercise formal/local length sources, callable-name
collisions through standard includes and ambiguous/concrete generic demands.
Exact source and binary pins bind the working-tree capture to the repairs
committed through `11dc52a`, although its base-revision field names `1b37220`.
Both presets finish without input errors or timeouts, with byte-identical
native/companion diagnostics and 1,025 unchanged original/dependency hashes.
Thesis retains 47 located limitations; all retains 72. No selected root has every
rule Completed. Focused public positive/negative checks supply the remaining
rule-family and repair checks; the final workspace receipts record 381 passing
tests, formatting and Clippy with warnings denied.

The table lists exact current-all IDs and partitions over those three roots.
The fourteen marked thesis rules have the same partitions in the thesis preset.
Inapplicable, NotRun and Unobserved are zero in this selected set.

| Rule ID | Thesis | Completed | Limited |
| --- | :---: | ---: | ---: |
| `naming` |  | 3 | 0 |
| `missing-constraint-label` |  | 3 | 0 |
| `prefer-array-comprehension` |  | 3 | 0 |
| `array-index-start` | yes | 2 | 1 |
| `compact-if` | yes | 2 | 1 |
| `constant-variable` | yes | 3 | 0 |
| `effective-zero-one` | yes | 3 | 0 |
| `element-predicate` | yes | 2 | 1 |
| `reified-global` | yes | 3 | 0 |
| `global-variable-in-function` | yes | 3 | 0 |
| `unbounded-variable` | yes | 1 | 2 |
| `search-coverage` | yes | 0 | 3 |
| `decision-variable-operator` | yes | 3 | 0 |
| `unmarked-symmetry-breaking` | yes | 3 | 0 |
| `unused-declaration` | yes | 2 | 1 |
| `decision-variable-generator` | yes | 3 | 0 |
| `decision-variable-condition` | yes | 3 | 0 |
| `suspicious-shadowing` |  | 3 | 0 |
| `expensive-comprehension` |  | 2 | 1 |
| `index-set-mismatch` |  | 2 | 1 |
| `hidden-optionality` |  | 3 | 0 |
| `partial-expression` |  | 1 | 2 |
| `vacuous-constraint` |  | 0 | 3 |
| `unused-generator-binding` |  | 3 | 0 |
| `global-constraint-opportunity` |  | 3 | 0 |
| `missing-input-precondition` |  | 3 | 0 |
| `suspicious-domain` |  | 2 | 1 |
| `prefer-enum` |  | 3 | 0 |

The private located ledger reconciles all 47/72 limitations to supported
uncertainty, documented exclusions and unavailable or Unobserved prerequisites.
Unobserved source-admission and descendant body-demand details remain qualified;
selected signatures do not prove source safety, outputs, membership or search
coverage. In particular, all_equal remains Limited. Independent base-083
verification found no unresolved witnessed promised-support failure under this
bounded contract. It did not certify unobserved facts or every corpus instance.

Full instance semantic runs remain useful for benchmarks or a witnessed
data-dependent regression. Replaying every instance after these lint-only
repairs is not an acceptance prerequisite. Invalid syntax, resource cutoffs,
missing observations and unavailable dependencies retain their actual outcomes.

## Release performance and public commands

Final measurements use the existing release binaries on the Apple M1 Max with
64 GiB RAM, macOS 26.6.2 arm64, Rust 1.98.1 and Python 3.14.6. Builds, compiler
checks, corpus checks and profiling do not overlap the timed campaign. Filesystem
caches and host load are uncontrolled; these are ordinary warm-cache measurements.

The existing save driver takes 50 fresh-process samples for each of 14 cases
under default (D) and nested (N) EditorConfig settings: 1,400 saves. Every
applicable per-case latency and separate own-child RSS budget passes. Table
values are D/N; raw samples retain minimum, maximum, statuses and child CPU.

| Case | Input bytes | p50 ms | p95 ms | Peak child RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| ordinary-changed | 1,163 | 4.219/4.269 | 4.731/4.712 | 2.031/2.047 |
| ordinary-formatted | 1,356 | 4.133/4.205 | 4.351/4.396 | 1.984/2.109 |
| invalid-edited | 33 | 3.908/3.994 | 4.347/4.315 | 1.781/1.828 |
| matrix | 676 | 4.178/4.255 | 9.059/17.246 | 1.859/1.891 |
| nested | 1,133 | 4.995/5.136 | 5.544/5.549 | 2.156/2.141 |
| matrix-grown | 6,971 | 4.603/4.666 | 5.205/5.207 | 2.297/2.312 |
| nested-grown | 4,493 | 31.914/31.977 | 32.833/32.939 | 3.297/3.297 |
| dense-10000 | 9,669 | 4.899/4.999 | 5.336/5.592 | 2.406/2.406 |
| dense-100000 | 96,669 | 11.882/11.903 | 12.416/12.336 | 6.672/6.688 |
| dense-1000000 | 966,669 | 81.216/81.243 | 82.279/82.155 | 47.797/47.812 |
| dense-2000000 | 1,933,338 | 160.196/160.560 | 165.527/166.614 | 91.984/92.000 |
| external-0-atsp.mzn | 6,899 | 5.136/5.212 | 6.174/5.904 | 2.312/2.375 |
| external-1-trip_7_4.mzn | 85,229 | 12.870/12.944 | 13.238/13.630 | 5.453/5.625 |
| external-2-u7g2pref1.dzn | 885,733 | 85.212/85.380 | 87.876/86.970 | 51.297/51.484 |

The derived already-formatted ordinary buffer is now 1,356 bytes instead of the
historical 1,324; the other 13 case hashes match the previous campaign. Native
status/stdout match the formal samples. Invalid-buffer stderr matches after
accounting for its supplied filepath; raw captures remain intact. First-use
identical-copy help/save observations are 5.175/5.142 ms, followed by
4.253/4.617 ms. No material startup delay appears here; cold I/O was not controlled.
The input above 1 MiB remains supported and outside the interactive budget range.

Forty bounded native runs finish and are reaped without cutoffs. Diagnostic
controls with 10,000 warnings take 0.243/0.245 seconds against 0.6 seconds;
25,000 warnings take 0.595/0.598 against 2.5 seconds. Twenty shared-include roots
under all 28 rules take 2.972/2.964 seconds against 12 seconds. The three original
thesis/all controls retain byte-identical streams and statuses against final
base-083 evidence. Dense 1/3/10/20-file wall growth has no observed superlinear
increase. Native RSS grows from about 48 to 341 MiB, consistent with the retained
finite allocator-residency attribution; this establishes no plateau or global
memory bound. Shared-root all-rule RSS grows from about 36 to 76 MiB over 1–20
roots. Existing matched diagnostics, phase attribution, allocation and scope-drop
receipts support the measured repair decisions; no new speedup is inferred.

Historical full thesis/all native runs at `87fcfc3` finish in
2,814.054/4,862.008 seconds with peak child RSS 9,509.578125/10,068.828125 MiB.
They supersede the earlier 600-second semantic cutoffs at their recorded
revision, without asserting post-repair full-corpus timings. Their native
interface supplies no completed-root count; the companion supplies complete
accounting. Neither finishing without a timeout nor exact allocation drops
prove a native memory bound. The [save report](save-performance.md),
[batch report](batch-performance.md) and base-083 evidence retain earlier
throughput, attribution and observation limits separately from save latency.

Current measurements and reproduction commands live under
`target/benchmarks/base050-final-performance/`: `save/report.json`,
`native/report.json`, the 13 bounded batch reports, `hardware.json`,
`before-pins.json`, `after-pins.json` and `acceptance-check.json`.
`/tmp/zincite050-final-performance-controls.json` indexes the final review;
`/tmp/zincite050-performance-run.py` retains the driver invocations. All 3,002
raw streams rehash without mismatch; source, build, original, input and
configuration pins are unchanged. No timing assertion is added to tests.

Directory controls compare the actual repository fixture directory (11 inputs),
Challenge ATSP directory (six inputs) and a two-file `test_bug218` include subset.
The regression directory contains 316 sources, including unrelated intentional
compiler negatives; only the unchanged known include closure is copied to a
separate temporary directory. That subset is not the complete regression tree.
Each formatter check and default/thesis/all lint command uses both standalone
and unified invocation forms. Directory and exact discovered-file invocations
have identical full diagnostics and statuses; overlapping directory/file inputs
are deduplicated. A missing-input control retains status 2 with precedence.
All 82 children finish within their finite limits and are reaped; original,
library and tool pins remain unchanged. Receipts are indexed by
`/tmp/zincite050-directory-controls.json`.

Manifest-only discovery over all three corpus roots finds the exact 6,417
source identities and exact default/thesis/all selections of 3/14/28 IDs. It
retains a discovery error for a dangling non-source editor lock link, so these
manifest commands return 2. That note is not a missing target source or a
semantic observation. The bounded native directory controls do not establish
full native processing of 6,417 inputs. Formatter directories use check mode;
originals remain untouched. Text exits retain 0 without warnings,
1 with unsuppressed warnings and 2 for actual errors, with error precedence.
Located semantic limitations alone do not change the lint exit status.

## Final acceptance status

Current full syntax/format, bounded directory, three representative compiler
controls and final named performance checks pass within their stated scope.
The retained reports supply historical compatibility and bounded semantic
evidence. They do not waive
unavailable inputs, invent Completed outcomes or certify unexplained compiler
output differences. No solver run or external-source write is part of acceptance.
