# Semantic corpus checkpoint

Task base-045 measures the public selected-analysis API over every inventoried
input. Both presets finished against one frozen checker. The results expose
execution and analysis gaps; they do not establish corpus acceptance.

## Scope and reproduction

The Challenge archive is revision
`a8448864fc56162583f24aaf9c25653d93f83765` and covers 2008–2026.
The local collection stays private. The official 2026 data supplement is separate
from the archive, whose 2026 directory has models but no data.

| Collection | Models | Data | Inputs |
| --- | ---: | ---: | ---: |
| Challenge archive | 436 | 1,604 | 2,040 |
| Private local collection | 2,085 | 2,202 | 4,287 |
| Official 2026 supplement | 20 | 70 | 90 |
| Total per preset | 2,541 | 3,876 | 6,417 |

The inventory retains all paths, including 564 duplicate-content inputs and
107 compiler-negative metadata hints. A hint is not a syntax-validity result.

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

The source HEAD is `77dad494268e441086849b97d70b878277b026e9`.
The release checker SHA256 is
`f28478c095c7b503014523bf8f38e1de8c38bde1a29bb7cb800baa812b9398a1`;
the frozen Python runner SHA256 is
`2be22bbd7e162a46a1e422571042ca78175230e7cd5230f22af9c56564217e01`.
`target/corpus/base045/pin.json` also records both source-file hashes.

Build with `cargo build --release -p zincite-fmt --example check-corpus-file`,
then retain that executable and the runner under `target/corpus/base045`.
`CORPUS_CHALLENGE`, `CORPUS_LOCAL` and `CORPUS_SUPPLEMENT` below denote the
three read-only roots recorded in each raw inventory. Run the two commands
with separate outputs:

```sh
python3 target/corpus/base045/check-corpus.py \
  --challenge "$CORPUS_CHALLENGE" --local "$CORPUS_LOCAL" \
  --supplement "$CORPUS_SUPPLEMENT" --output target/corpus/base045/thesis \
  --binary target/corpus/base045/check-corpus-file --rules thesis \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  --timeout 30

python3 target/corpus/base045/check-corpus.py \
  --challenge "$CORPUS_CHALLENGE" --local "$CORPUS_LOCAL" \
  --supplement "$CORPUS_SUPPLEMENT" --output target/corpus/base045/all \
  --binary target/corpus/base045/check-corpus-file --rules all \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  --timeout 30
```

Both commands use MiniZinc 2.10.1's installed core/library directory and no
additional include directories. Relative model includes remain part of their
read-only user closure; installed library files remain external and unmodified.
The checker skips formatting in semantic mode. It uses `analyze_file` for data
or rejected root syntax, and `load_model` plus `analyze_model` for syntax-clean
model candidates. The existing syntax/format mode remains available.

A two-child smoke supported concurrent independent preset jobs. Each model
smoke took about 1.15–1.68 seconds; data and rejected roots took at most 0.008
seconds. The final resource smoke observed 34,914,304 bytes maximum child RSS
on macOS; twice that value is only a conservative two-child estimate, excluding
the parent. These observations chose the visible 30-second per-file deadline;
they are not a performance benchmark or budget claim. The initial `time -l`
wrapper failed on a restricted sysctl and remains in the raw evidence.

## Complete accounting

Each preset has exactly 6,417 unique path rows. Independent reconciliation
matches the inventories, all per-rule partitions and diagnostic totals.
Both runners rehashed 6,417 originals after their runs; a separate final check
rehashed every original after both runs and all retained compiler probes.
There was no input drift, pinned executable drift or checker/runner source drift.

| Measure | thesis | all |
| --- | ---: | ---: |
| API reports | 6,371 | 6,371 |
| Process deadlines | 41 | 41 |
| Warnings | 20,180 | 52,430 |
| Errors | 6,644 | 6,644 |
| Analysis limitations | 57,487 | 57,487 |
| Structural complete roots | 1,123 | 1,123 |
| Complete roots with resolved dependencies | 591 | 591 |
| Complete resolved roots with every selected rule Completed | 0 | 0 |
| API status 0 | 4,728 | 4,339 |
| API status 1 | 953 | 1,342 |
| API status 2 | 690 | 690 |

The thesis run took 4624.985 seconds; the all run took
4624.384 seconds while the jobs ran concurrently.
Both campaign commands exited 1 because measured failures or limitations
remained. This is separate from child/API status: 6,376 children returned a
report, including five invalid-UTF-8 input reports; 41 children hit the deadline.
There were no full-run crashes or malformed protocol rows. The 46 inputs
without an API result have explicit **Unobserved** rule results, not API NotRun
or invented zero findings.

The five encoding failures are archive crosswords data files. The brief requires
UTF-8; no conversion or external rewrite was attempted. Forty observed inputs
report NotRun for every thesis rule: 31 rejected model roots and nine rejected
data inputs. Their syntax needs assessment; rejection is not proof that each
original is invalid MiniZinc. Both presets have 1,845 rows with at least one
failure cause. Cause sets overlap: 690 rows have analysis errors, 1,611 have
limitations, 40 require syntax assessment, five have invalid UTF-8 and 41 time out.

| Root state | Challenge | Local | Supplement | Total |
| --- | ---: | ---: | ---: | ---: |
| Structural complete | 386 | 717 | 20 | 1,123 |
| Fragment | 2 | 1,343 | 0 | 1,345 |
| Multiple solve | 0 | 1 | 0 | 1 |
| Data | 1,599 | 2,202 | 70 | 3,871 |
| Rejected model syntax | 14 | 17 | 0 | 31 |
| Unobserved | 39 | 7 | 0 | 46 |

Structural completeness counts solve items in retained user files; it does not
prove compiler validity. Of the 1,123 structural complete roots, 591 have usable
resolved dependencies and 532 do not. A resolved closure requires the implicit
core, no unresolved include edges and no loading errors. It does not prove full
typing, all rules' prerequisites or complete semantic analysis.

The fourteen thesis rule partitions and findings are identical in both presets.
Each row below sums to 6,417; `all` also selects the two default style rules.
Findings can coexist with Limited outcomes. All warning totals equal the sum of
the selected per-rule finding counts.

| Rule | Completed | Inapplicable | Limited | NotRun | Unobserved | Findings |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| array-index-start | 1,393 | 3,862 | 1,076 | 40 | 46 | 126 |
| compact-if | 1,690 | 3,862 | 779 | 40 | 46 | 7 |
| constant-variable | 1,711 | 3,862 | 758 | 40 | 46 | 88 |
| decision-variable-condition | 1,656 | 3,862 | 813 | 40 | 46 | 915 |
| decision-variable-generator | 1,779 | 3,862 | 690 | 40 | 46 | 910 |
| decision-variable-operator | 1,766 | 3,862 | 703 | 40 | 46 | 14,127 |
| effective-zero-one | 1,698 | 3,862 | 771 | 40 | 46 | 0 |
| element-predicate | 1,362 | 3,862 | 1,107 | 40 | 46 | 2,393 |
| global-variable-in-function | 1,802 | 3,862 | 667 | 40 | 46 | 504 |
| reified-global | 1,771 | 3,862 | 698 | 40 | 46 | 186 |
| search-coverage | 194 | 5,089 | 1,048 | 40 | 46 | 286 |
| unbounded-variable | 1,566 | 3,862 | 903 | 40 | 46 | 234 |
| unmarked-symmetry-breaking | 1,817 | 3,862 | 652 | 40 | 46 | 113 |
| unused-declaration | 0 | 5,089 | 1,242 | 40 | 46 | 291 |

Additional `all` rules:

| Rule | Completed | Inapplicable | Limited | NotRun | Unobserved | Findings |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| missing-constraint-label | 5,681 | 0 | 650 | 40 | 46 | 21,283 |
| naming | 5,681 | 0 | 650 | 40 | 46 | 10,967 |

The clean solve-only smoke has zero warnings but eight unused-declaration
limitations from retained standard dependencies. No corpus input reports
unused-declaration Completed. Zero warnings therefore cannot stand in for full
coverage; 284 structural complete thesis rows have zero warnings but at least one
limitation.
The raw outcomes remain authoritative even when another rule completes or emits
useful advice on the same input.

Checked examples of each outcome use public originals:

| Outcome | Input and rule | Meaning |
| --- | --- | --- |
| Completed | `2008/nmseq/nmseq.mzn`, array-index-start | This rule completed even though other rules were limited. |
| Inapplicable | `2008/debruijn_binary/02_03.dzn`, array-index-start | Standalone data has no model context. |
| Limited | `2008/debruijn_binary/debruijn_binary.mzn`, array-index-start | Dependencies or required semantic facts are incomplete; API status is 2. |
| NotRun | `2014/elitserien/handball.mzn`, array-index-start | Rejected syntax prevents this analysis; compiler validity is unclaimed. |
| Unobserved | `2012/project-planning/ProjectPlannertest_12_8.mzn` | The child timed out before reporting API outcomes. |

## Representative advice

Each location below was checked against the original bytes, spelling, line and
column. The source ownership is user. The rows review one finding per thesis ID,
not every emitted finding or every model's validity. Public paths are relative to
the Challenge archive; the effective-zero-one row is separate fixture evidence.

| Rule | Public input or fixture | Outcome | Reviewed advice |
| --- | --- | --- | --- |
| unbounded-variable | `2008/debruijn_binary/debruijn_binary.mzn` | Limited | `gcc` has unbounded integer array elements without a proven complete definition. |
| unused-declaration | `2008/nmseq/nmseq.mzn` | Limited | `s_sum` is absent from constraints, solve and explicit output; unresolved search semantics remain separate. |
| decision-variable-operator | `2009/fillomino/fillomino.mzn` | Limited | `\/` combines decision constraints; reformulation/tabling advice stays conditional. |
| global-variable-in-function | `2009/fillomino/fillomino.mzn` | Completed | `joins` captures the top-level decision array `when`, rather than a formal or local. |
| reified-global | `2009/nonogram/non.mzn` | Limited | The `regular` use supplies a Boolean value inside `nonogram_row`; the advice does not specialize every caller. |
| unmarked-symmetry-breaking | `2009/p1f/p1f.mzn` | Limited | Unwrapped `lex_less(ra, rb)` asks whether symmetry breaking is intended, without asserting it. |
| decision-variable-generator | `2010/depot_placement/depot_placement.mzn` | Limited | `TourALoc++TourBLoc` is a decision-dependent source, not the loop variable or body. |
| search-coverage | `2010/wwtp_random/wwtpp.mzn` | Limited | The whole top-level array `d` is not covered by supported seeds/definitions; Limited prevents a completeness claim. |
| element-predicate | `2013/javarouting/trip_6_3.mzn` | Limited | The resolved standard three-argument `element` relates an index, literal array and decision output. |
| decision-variable-condition | `2015/gfd-schedule/gfd-schedule.mzn` | Completed | `itemProcessDay[i] <= deadLineDay[i]` is the decision-dependent guard itself. |
| array-index-start | `2016/cryptanalysis/step1_aes.mzn` | Completed | `0..BC-1` has a proven minimum zero; symbolic and enum minima are not guessed. |
| constant-variable | `2015/zephyrus/zephyrus_5_20.mzn` | Completed | The unconditional whole-value equality to `20` supports reviewing parameter instantiation. |
| compact-if | `2019/fox-geese-corn/foxgeesecorn.mzn` | Completed | The branches are `-1` and `0`; compact-form advice compares readability and promises no speed gain. |
| effective-zero-one | `crates/zincite-lint/tests/effective_zero_one.rs` (fixture supplement) | Completed | The existing `Bit=var 0..1` fixture proves both implication values; zero polarity and exact sums are also checked by that test. |

The full corpus emits no effective-zero-one findings. The supplemental model is
an exact export of the existing public test
`invariant_bounds_both_polarities_and_whole_sums_have_independent_source_facts`;
it is not a corpus row. Its installed-core analysis reports the rule Completed
with ten findings. MiniZinc rejects the original BOM-bearing export. A separate
control removes only the three-byte BOM and passes model-check-only; both inputs,
hashes and the initial rejection remain under the fixture-supplement directory.
This does not imply compiler acceptance of the original export.

## Causes and bounded follow-ups

The runner preserves exact error and limitation counts, but diagnostic samples
are bounded: three findings per rule, twelve errors, twelve limitations and
512 characters per message. Sample frequencies are not an exhaustive cause
count. Missing external includes, malformed loaded syntax, unresolved binding
facts, incomplete generic argument types, unsupported annotation/control forms
and ordinary symbolic/domain uncertainty remain explicit. Unknown values are
not automatically Zincite defects or proof that a model is invalid.

Two confirmed repair families have separate drafts for coordinator publication:

1. **Standard include re-entries.** MiniZinc 2.10.1 accepts the small
   `include "all_different.mzn"` model, but Zincite reports four cycle errors
   through `all_different_except`/`all_different_except_0` and the optional
   global-cardinality implementations. The active canonical-file branch in
   `model.rs` rejects these retained standard back-edges. This is a loader defect,
   not an unavailable external dependency. A separate self-include smoke is
   rejected by the compiler and Zincite; genuine user-cycle errors must remain.
   Independent root and verifier replays support this distinction.
2. **Defaulted and qualified overload ranking.** A compiler-accepted
   `choose_present` reduction becomes Ambiguous between a present-array signature
   with an omitted default and an optional-array signature. The matcher appends
   omitted defaults before its equal-length minimum comparison; actual
   `all_different` declarations reproduce the same cause. Separately, a
   compiler-accepted parameter `absent(opt int)` guard becomes Ambiguous and
   limits decision-variable-condition. Generic pattern ordering can veto strict
   instantiation/optionality narrowing. Parameter integer `<=` also becomes
   Ambiguous and affects declaration dependencies, but its guard already has a
   Parameter fact and decision-variable-condition completes. Its search-control
   limitation is separate. Repair must retain all resolved formal/default facts,
   exact declaration identities, genuine ambiguity and the unknown-candidate veto.

These drafts name affected consumers and focused completion checks; they do not
repair the engine here. Their keys are `semantic-include-closures` and
`semantic-call-ranking`. They are proposals, not already recorded blockers.
The coordinator owns derived-task publication and base-050 dependencies after
independent source verification.

Counterevidence keeps the gap classification bounded. The original public
`2008/nmseq/nmseq.mzn` supplies strings to `int_search`. The current public
CallableFacts reports NoMatch and pinned MiniZinc rejects that signature.
Only this checked root is classified by that rejection; old string spellings or
compiler-negative metadata elsewhere need their own evidence. Compiler probes
use `--model-check-only`; none solves, flattens the corpus or rewrites originals.

Known syntax work remains with base-052/base-053, and batch performance work
remains with base-049. The 41 deadlines stay unobserved; previous formatter
results cannot manufacture semantic outcomes. This checkpoint neither duplicates
those tasks nor claims every remaining limitation is a newly confirmed defect.

## Validation and acceptance boundary

The release build, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
and `git diff --check` passed. The original checker mode passed the existing
integration fixture. The protocol smoke rejects null dependencies and malformed
finding samples, records a deadline and continues to a later valid data input.
The overly short initial fake-checker deadline and restricted resource-wrapper
failures remain visible in raw evidence.

Raw evidence is ignored under `target/corpus/base045`: `pin.json`, each preset's
`inventory.json`, `results.jsonl`, `summary.json` and run log, `reconciliation.json`,
`shared-rule-partitions.json`, `smoke.json`, `protocol-final/`, `resource-smoke.json`,
`finding-reviews.json`, `fixture-supplement/`, `probes/`, `followups.json` and
`validation.json`. The raw records retain private paths for local reproduction;
this tracked report includes no private corpus source copies. Counts reconcile
all inputs rather than excluding duplicates, negative hints or failed roots.

The measurement integration is complete. Corpus acceptance is unproven while
valid-input loading/ranking gaps, syntax work, deadlines and rule-local limits
remain. Base-050 must assess their repairs and actual coverage; the 591 resolved
closures and reviewed advice are not a substitute for that gate.
