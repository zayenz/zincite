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

## Base-075 bounded standard-include repair

Base-075 changes the loader's active canonical-file check. A non-self back-edge
now reuses the existing file when every file from the repeated target through the
including file belongs to the configured standard library. A user root before
that segment is allowed. Self-includes and segments containing a user file still
produce the original located error and retain their target edge. The loader keeps
relative-first lookup, ordered include directories, canonical deduplication,
first readable paths, implicit/explicit flags and original include ranges.

Five public loader checks pass. The new checks cover aliased standard back-edges,
a user root outside their segment, implicit core availability, explicit standard
roots, BOM/CRLF ranges, standard and user self-includes, user cycles and both mixed
cycle target classifications. The initial standard re-entry check fails on the
old loader at the retained `second.mzn` include range `8..26`. The existing checks
continue to cover search order, missing/unreadable/malformed dependencies,
independent roots, suppressions and reporter status.

The replay uses the original small `all_different` model, the retained self-include,
the unchanged public `2023/sudoku_fixed/sudoku_fixed.mzn`, and a globals reduction.
The planner named that reduction `globals.mzn` while it includes `"globals.mzn"`.
Zincite's relative-first lookup therefore reaches the reduction itself: its one
self-cycle remains an error. A separate `root-globals.mzn` contains exactly the
same 105 bytes (SHA256
`a0b93c5efa8d9c19ae8e793c01716c268701c2e6c1b54435fa4815f4371272cd`)
and exercises the installed globals closure. The original shadowed input and its
results remain intact. No search-order exception was added.

MiniZinc 2.10.1 build 33348285743 accepts the small model, both globals filenames
and public Sudoku with `--model-check-only` and an explicit installed standard
library. It rejects the retained self-include with a cyclic-include error. All
five commands finish within their 60-second deadlines; none runs a solver.
Compiler acceptance of the shadowed filename does not change Zincite's explicit
relative-first search contract.

The table records fresh library/native results. Each pair has identical complete
stderr bytes and exit status. Files and include edges, their target IDs, source
classifications, flags and original ranges equal the pre-repair library replay.
Only the accepted standard cycle errors disappear. The earlier base-045 corpus
counts above remain historical evidence.

| Input | Canonical files / edges | Load errors before → after | Thesis14: status; warnings; limits; Completed/Limited | All26: status; warnings; limits; Completed/Limited |
| --- | --- | --- | --- | --- |
| Original `all_different` | 65 / 79 | 4 → 0 | 0; 0; 5; 11/3 | 1; 1; 6; 22/4 |
| Shadowed `globals.mzn` | 23 / 23 | 1 → 1 | 2; 0; 1; 0/14 | 2; 1; 2; 0/26 |
| Retained self-include | 23 / 23 | 1 → 1 | 2; 0; 0; 0/14 | 2; 0; 0; 0/26 |
| Public Sudoku | 484 / 657 | 16 → 0 | 0; 0; 18; 11/3 | 1; 3; 21; 22/4 |
| Distinctly named globals reduction | 484 / 657 | 16 → 0 | 0; 0; 5; 11/3 | 1; 1; 6; 22/4 |

Accepted controls have no analysis errors; each rejected control retains one.
All selected rule outcomes are recorded: none is Inapplicable or NotRun here.
The small model and globals reduction still have ambiguous `all_different` and
unknown `not`/`occurs` callable facts. Their Limited rules are unused-declaration,
element-predicate and search-coverage; All26 also limits vacuous-constraint.
Sudoku instead limits unused-declaration, element-predicate and reified-global,
plus vacuous-constraint under All26. Its All26 warnings are one missing label and
two expensive-comprehension findings. These limitations remain independent of
loading, and zero warnings under thesis do not imply complete semantic analysis.

Ignored evidence is under `target/base075`: original ownership and input pins,
`standard-reentry-red.log`, the corrected focused logs, `baseline-closures/`,
`counterpart-baseline/`, `compiler-controls/`, `analysis-controls/`, binary pins
and the final validation logs. The replay pins and rechecks external controls,
the installed closure, compiler and retained base-045 reports/probes. It does not
edit those sources or run a full corpus campaign. Corpus acceptance still needs
its final gate, callable-ranking repair, unresolved syntax/dependencies and
rule-local limitations; this bounded result establishes the standard include
re-entry repair only.

## Base-076 bounded callable-ranking repair

Base-076 ranks applicable overloads using only the arguments supplied by the
caller, in their corresponding positional or named formal slots. Omitted
defaults still undergo applicability and lexical-scope type checks. The resolved
operation retains every instantiated formal type; its declaration identity keeps
defaults in their original scope for consumers. Component coercion, strict qualifier ordering, the
unknown-candidate veto and genuine competing implementations keep their existing
behavior.

The public regression fails on the old matcher: `choose_present(values)` is
Ambiguous between a defaulted present-array operation and an optional-array
operation. It now selects the present-array declaration and retains both formal
types. The combined check also covers reordered named arguments, a default in a
middle slot, caller shadowing, a retained dependency on the declaration-scoped
default, genuinely competing defaults and Parameter guards. Seventeen focused
checks pass, reusing the existing user-overload, unknown-candidate, incomparable,
prototype/implementation, default-output, conditional and partiality checks.

Fresh installed-library facts reproduce and resolve the two default-ranking
failures. The original `choose_present` call at `300..314` selects its present-array
declaration at `10..24` with its second formal intact. The original `all_different`
call at `73..86` selects the present-array operation in `std/all_different.mzn`
at `830..843`, retaining the defaulted `except` formal as `set of int`.
The distinct-name globals control selects that same operation. All three public
Sudoku `alldifferent` calls select the defaulted present-array synonym.

The historical qualifier probes above do not reproduce a current ranking defect.
Before and after this repair, `absent(opt int)` selects the parameter generic in
`stdlib_opt.mzn` at `3399..3405`; parameter integer `<=` selects the parameter
generic in `stdlib_compare.mzn` at `3512..3516`. Their enclosing guards at
`170..193` and `63..76` remain Parameter. The complete public-fact streams for
both controls are byte-identical before and after. Their individual
decision-variable-condition analyses complete with no errors, limitations or
findings. No qualifier or consumer code needed a change.

All six unchanged controls pass pinned MiniZinc 2.10.1 model-check-only with an
explicit installed stdlib: the four original reductions, distinctly named
globals control and public Sudoku. Commands have 60-second deadlines and retained
full streams; none runs a solver. Twelve before and twelve after thesis14/all26
library/native pairs finish within their 120-second deadlines. Every pair agrees
on complete stderr and status. Canonical files, first paths, source flags and
include edges equal their pre-repair graphs, with zero load errors throughout.
The original shadowed `globals.mzn` fixture and base-075 evidence stay intact.

The table records the current outcomes. Limitations show before → after counts;
Completed/Limited partitions are the current per-rule results. No rule is
Inapplicable or NotRun in these replays, and no analysis error occurs.

| Input | Thesis14: status; warnings; limits; Completed/Limited | All26: status; warnings; limits; Completed/Limited |
| --- | --- | --- |
| `choose_present` reduction | 1; 4; 5 → 2; 13/1 | 1; 4; 6 → 3; 24/2 |
| Optional guard | 0; 0; 3 → 3; 12/2 | 0; 0; 5 → 5; 23/3 |
| Parameter comparison | 0; 0; 3 → 3; 12/2 | 1; 1; 5 → 5; 23/3 |
| Original `all_different` | 0; 0; 5 → 15; 12/2 | 1; 1; 6 → 16; 23/3 |
| Distinct-name globals | 0; 0; 5 → 15; 12/2 | 1; 1; 6 → 16; 23/3 |
| Public Sudoku | 0; 0; 18 → 5; 13/1 | 1; 3; 21 → 8; 24/2 |

The located ambiguity limitations disappear from element-predicate and
unused-declaration for the repaired calls. Element-predicate now completes on
all four affected inputs; reified-global also completes on Sudoku instead of
reporting its three ambiguous global calls. Global-variable-in-function remains
Completed throughout. The `choose_present` reduction now reports the unselected
optional overload, its unused present-overload formals and search-coverage
advice for `values`. It retains the separate unknown `not`/`occurs` facts.

Resolving a root call can expose more unfinished body analysis. The small
`all_different` and globals controls now retain unknown generic-body operations
at installed `all_different.mzn` ranges `910..1073`, and search-coverage reports
unsupported body control flow at `900..1090`. Their old root-call ambiguity is
gone; unused-declaration and search-coverage remain Limited. Sudoku retains five
unused-declaration limitations for `not`, `occurs`, generic synonym-body
`all_different`/`array1d` and `int_search`. All26 also retains vacuous-constraint
limits for opaque calls. The optional/comparison controls retain their unrelated
conditional search, unused-declaration and suspicious-domain limits. These are
recorded by source range and cause. Their per-rule outcomes remain Limited.

Evidence under `target/base076` includes original ownership and input pins,
`default-ranking-red.log`, `focused-green.log`, preserved baseline/current
binaries, `before-closures/`, `compiler-controls/`, `before-replays/`,
`after-replays/` and `reconciliation.json`. External sources and historical
base-045/base-075 artifacts remain unchanged. This bounded repair does not run
a corpus campaign, certify Safe fixes, or establish full corpus acceptance.

## Base-083 diagnostic scaling review

The retained thesis capture for the public Challenge input
`2012/project-planning/ProjectPlannertest_16_7.mzn` reports 66,106 warnings,
including 65,694 `decision-variable-operator` warnings. Independent checks of
the original source hash and every operator finding confirm 65,694 distinct
byte ranges, each spelling `not`, in the 3,869,648-byte input. These findings
come from expressions already unrolled in the source, rather than warnings
multiplied by generator iterations or callable instantiations during analysis.

The operator findings occupy 53 source lines. One constraint on line 1888 is
3,698,158 bytes long and contains 65,519 of the reported operators. A count
grouped under that constraint could make the repeated advice easier to scan,
but a shared line alone is not an actionable grouping boundary: distinct
expressions can share a line, and one item can span several lines.

The decision for this task is to retain the precise per-occurrence findings.
The rule walks original syntax and checks item suppressions before emitting
findings; its results retain individual locations for library consumers.
Collapsing those results to one representative location would lose that
information. Any future grouped display should summarize already-filtered
findings by their original containing item and advice, report the occurrence
count, and retain access to every location. Presentation grouping must leave
rule outcomes and analysis limitations intact.

The checked evidence is retained under
`target/benchmarks/base083/semantic53-project16-whole/`: `before-inputs.json`
pins the original, `0-native.stderr` contains the complete thesis diagnostics,
and `0-native.json` records the completed native invocation and stream hash.
These counts establish the scaling behavior of this captured model; they do
not establish complete corpus coverage or explain its analysis cost.

## Base-083 guarded scope sharing

The retained native Project17 thesis replay finished within its 1,800-second
deadline with byte-identical diagnostics to the unsampled semantic55 baseline.
Its single late one-second sample places all 78 sampled stacks under guarded
interpretation, including private scope and assumption-vector cloning. This
identifies work present in that window, rather than its share of total runtime.

Commit `c07e0ac` shares the private assumption vector between unchanged scopes.
Existing mutation sites detach it before adding assumptions; callable bodies
that discard assumptions receive a fresh empty vector. Original assumption
order, queries and public snapshots remain unchanged.

Formatting, Clippy and all 193 workspace tests pass. Full ordered public
`GuardedFacts` from both interpreter entry points are byte-identical between
semantic55 and semantic56 on the two retained compiler-positive controls.
The complete paired outputs are 77,655 and 221,798 bytes. Evidence is retained
in `target/benchmarks/base083/semantic56-guarded-parity/`; original source,
compiler controls, executables and output hashes were independently checked.
The matched unsampled semantic56 native replay completed with byte-identical
diagnostics in 1,235.87 seconds, compared with 1,177.04 seconds for semantic55.
Peak RSS was 4,452.48 MiB, compared with 4,265.75 MiB. This single comparison
shows no measured speed or memory improvement. It does not establish a stable
regression.

The serial matched allocation companions both completed with identical source
graphs, fourteen-rule outcomes and full diagnostics. Analysis requested bytes
fell from 175,428,664,043 to 118,780,205,547, a 32.29% reduction. Allocation calls
fell by 0.15%; retained analysis bytes remained 69,813,828 and peak tracked
analysis bytes fell by 0.13%. Load and render allocation counters were unchanged.
Both scopes returned to their 2,395-byte starting level after drop. These
whole-phase counters demonstrate an allocation-byte benefit from scope sharing;
they do not establish a speed or native-memory improvement. The paired evidence
is retained in `target/benchmarks/base083/semantic56-project17-allocation-control/`.
Original inputs, current source, artifacts and complete output streams were
independently checked. Complete corpus acceptance remains pending.

The seven retained diagnostic and shared-include regression controls also
completed: 26 native runs and 13 companions preserve semantic55 statuses,
diagnostics and analysis outcomes, and every reported scope returns to its
starting level. All current streams and preserved comparison inputs were
checked. These controls contain no enforced nonempty assumptions, so they check
regressions and growth rather than the specific allocation benefit above.

## Base-083 remaining acceptance checks

The reconciled `target/corpus/base080-complete/` run accounts for all 6,417
inputs through syntax and formatting checks, with no rules selected. It does
not establish semantic coverage or current compiler validity. The historical
availability split also needs reconciliation against fresh dependency results.
Final acceptance must keep source-only compiler checks distinct from successful
flattening of known model/data pairs; a compilation timeout remains unresolved
processing, rather than evidence of invalid source.

One concrete boundary in the retained `full-final20` thesis capture is
`2011/black-hole/black-hole.mzn`: its completed, resolved root has thirteen
Completed rule outcomes and a Limited search outcome. The located limitation
at line 105, bytes 3122..3145, is the `inverse` call annotated with `domain`.
The current callable interpreter rejects non-string expression annotations
before inspecting the call; the installed standard library declares `domain`
as a propagation-strength hint. A fresh, pinned MiniZinc source check passes;
the semantic56 focused capture confirms this boundary and a separate extrema
proof limitation in `std/fzn_table_int.mzn`.

The semantic57 repair accepts only a bare, uniquely resolved `domain`
declaration from the implicit standard library. It preserves underlying call
prerequisites. Existing search-fixture checks now accept the standard hint,
retain unknown results for shadowed and unresolved annotations, and confirm
that the hint cannot make a partial body total. Formatting, Clippy and all 193
workspace tests pass.

The fresh semantic57 Black Hole captures now complete for selected search,
thesis and all rules. Each library/native pair has identical statuses, full
diagnostics and loaded-source graphs. The thirteen other thesis rules complete;
search still reports two limitations. The all preset completes twenty-four
rules and retains limitations in search and vacuous-constraint. Original
compiler/model/standard-library inputs, current source, artifacts and complete
streams were independently checked. These results do not establish corpus
acceptance.

The annotation rejection is replaced by a located limitation inside the
standard inverse wrapper: `index2int(enum2int(invf))` is a computed output actual
that the current interpreter cannot invert. Public facts retain whole-array
coverage for searched `x` and Unknown for `y`, with no derived definition for
`y`. The separate table limitation retains twelve target identities; its only
uncovered target is the standard body's private decision `i`. Its domain uses
`lt..ut`, whose extrema remain unsupported. The actual model supplies a
two-element decision array and a parameter table with explicit nonempty axes
`1..416` and `1..2`. Inspecting those prerequisites needs further support;
discarding private targets would conceal the unfinished body inspection.

The complete captures and public facts are retained in
`target/benchmarks/base083/semantic57-black-hole-boundary/` and
`target/benchmarks/base083/semantic57-black-hole-facts/`. Both helpers and all
children finished within their existing deadlines. No limitation was suppressed
and no Completed outcome was inferred from compiler success.

The original Black Hole model also compiles successfully to FlatZinc with
Gecode and the challenge's supplied `10.dzn`, without solving. That data assigns
the model's remaining `layout` parameter with the declared `1..17, 1..3` shape;
the original model already assigns `neighbours`. Compilation finishes in
0.17 seconds and produces 19,524 bytes. The complete receipt and output are in
`target/benchmarks/base083/semantic57-black-hole-known-pair-compiler/`; all
1,038 original compiler, standard-library, model and data inputs match before
and after. This strengthens this case's processing classification beyond the
earlier source-only check while leaving semantic acceptance open.

The immutable semantic57 formatter's copy also compiles with the same data and
Gecode. The two complete FlatZinc outputs differ only on line 3, the generated
command-line invocation comment containing the different file paths. All other
bytes match. The comparison, formatter/compiler receipts and unchanged-original
checks are retained alongside the original compilation evidence.

## Base-083 whole-array conversion views

The semantic58 repair maps WholeArray callable-output guarantees through the
standard `enum2int` and `index2int` array views. MiniZinc 2.10.1 implements both
as single-argument representation views. Zincite checks the unique implicit
standard declaration, exact selected signature, rank and qualifiers before
following the argument to a bare present decision-array declaration. Scalar
and ArrayElement guarantees gain no new mapping. Strict dependency inspection
also follows the checked `index2int` argument; underlying prerequisites remain
required.

The existing callable-output regression fails before this repair and passes
afterward. It retains Unknown for optional and user-defined noninjective
conversions and does not promote a filtered body to whole-array coverage.
Formatting, Clippy and all 193 workspace tests pass. Independent bounded review
found no defect in these mapping guards. The fresh semantic58 optimized Black
Hole captures still retain the same inverse computed-output limitation, as
well as the table extrema limitation. Selected search, thesis and all pairs
finish with identical library/native statuses, full diagnostics and source
graphs; thirteen thesis rules and twenty-four all rules complete. The checked
conversion shape in the regression is insufficient to establish coverage for
the actual standard wrapper instance. Its selected conversion types and output
mapping need further diagnosis before changing any guard. Originals, current
source and artifacts remain unchanged; the complete evidence is retained in
`target/benchmarks/base083/semantic58-black-hole-boundary/`. Base-083 remains
open.

The semantic58 public-facts supplement identifies the remaining inverse gap:
both `fzn_inverse` outputs are ArrayElement guarantees, with no dependencies.
Its equalities select `invf[f[i]]` and `f[invf[j]]`; ordinary exact-traversal
coverage does not establish a whole output through those computed indices.
The WholeArray conversion guard therefore correctly declines them. This is a
missing reciprocal inverse-body proof, not evidence for relaxing conversion
type checks or promoting an existing ArrayElement guarantee. Any whole-array
proof must use both complete reciprocal relations and retain the opposite array
as a dependency. The checked public rows and complete output are retained in
`target/benchmarks/base083/semantic58-black-hole-facts/`; original inputs,
source, artifacts and both successful child receipts were independently checked.

A small reduction with the same reciprocal body, converted actuals and bounded
decision arrays also compiles to FlatZinc with Gecode without solving. The
immutable semantic58 linter retains the computed-output inversion limitation
at its single enforced call. The valid model and both complete receipts are
retained in `target/benchmarks/base083/semantic59-reciprocal-compiler-before/`
as `reciprocal-valid.mzn`, `compile-valid.json` and `before-native-valid.json`.
This is the compiler-valid before control for the next body-proof repair.

The semantic59 repair recognizes the complete reciprocal inverse body by its
resolved operations, two unfiltered traversals and four matching clauses. It
emits whole-array guarantees with the opposite array as a dependency; missing
halves, filtered traversals and unsearched reciprocal cycles gain no coverage.
The focused regression fails before the repair and passes afterward. Formatting,
Clippy and all 193 workspace tests pass; independent bounded review found no
defect. The source checkpoint is committed as `da383e9`.

Fresh optimized Black Hole native/library captures now clear the computed-output
inversion limitation. All six captures finish within their bounds and retain
identical paired statuses, complete diagnostics and source graphs. Selected
search and thesis return status zero, while search coverage remains Limited on
the separate table extrema/body gap. The all preset retains that gap and two
vacuous-constraint limitations. Thirteen thesis rules and twenty-four all rules
complete. Status zero does not establish full semantic acceptance. The original
inputs and tool artifacts remain unchanged; evidence is retained in
`target/benchmarks/base083/semantic59-black-hole-boundary/`. Base-083 stays open.

The same immutable semantic59 native tool also clears the limitation in the
compiler-valid reciprocal reduction, using the previous command with only the
tool path changed. It returns zero with empty diagnostic streams; all 1,039
retained input pins and the before evidence remain unchanged. The complete
receipt is in `target/benchmarks/base083/semantic59-reciprocal-native-after/`.

The remaining table gap has a compiler-valid reduction using the installed
`fzn_table_int` body, with only the predicate name changed and a concrete model
added. Gecode compilation succeeds without solving; the immutable semantic59
linter retains the same extrema limitation at bytes 390..1006. All 1,039 input
pins remain unchanged. This before control is retained in
`target/benchmarks/base083/semantic60-table-compiler-before/`. The new public
regression independently fails at the same symbolic-bound inspection gap and
requires the inspected body to export no output guarantees.

The semantic60 repair extends the existing uncertain-bound callable inspector
to the table body's rank-two index-set extrema, private integer selector and
parameter transpose array. It checks declarations in order, rejects forward or
cyclic local dependencies, and matches the constructor axes to both unfiltered
Cartesian headers. Child inspection keeps unsupported operations as limitations;
unknown bounds and membership create no values or output guarantees. The
regression passes, including the filtered negative and no-output checks.
Formatting, Clippy and all 193 workspace tests pass. Root and independent bounded
review found no defect; the source checkpoint is `910d565`.

Fresh optimized native/library Black Hole pairs now complete selected search
coverage and all fourteen thesis rules with no limitations. The all preset
completes twenty-five rules and retains the two vacuous-constraint limitations.
All six captures preserve exact paired statuses, diagnostics and source graphs;
the original inputs and artifacts remain unchanged. The compiler-valid table
reduction also clears its previous extrema limitation with empty diagnostic
streams. Evidence is retained in `target/benchmarks/base083/semantic60-black-hole-boundary/`
and `target/benchmarks/base083/semantic60-table-native-after/`. These model-level
results do not establish full-corpus acceptance; base-083 remains open.
