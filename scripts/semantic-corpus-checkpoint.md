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

The semantic61 checkpoint moves scalar alias eligibility checks ahead of cycle
tracking, avoiding a set insertion for declarations that cannot be traversed.
The source checkpoint is `7255bf5`. Formatting, Clippy and all 193 workspace
tests pass. Both complete ordered GuardedFacts entry points match semantic60
byte for byte on the two retained compiler-checked controls; all 1,039 input
pins, 90 source pins and 21 parity artifacts were independently checked.
Evidence is retained in `target/benchmarks/base083/semantic61-guarded-parity/`.
Matched Project17 allocation and native measurements remain pending. This
checkpoint establishes no measured performance benefit or task acceptance;
base-083 remains open.

The fresh semantic60 Project17 allocation baseline reached its unchanged
1,800-second cap and was reaped with status -9. Its complete retained stdout
contains a manifest and analysis-start event, with no root, drop or completion
record. The scheduled semantic61 capture continues separately. This censored
baseline supplies no completed allocation counters and cannot support a
before/after ratio or a claim about the timeout's cause. Evidence is retained
in `target/benchmarks/base083/semantic61-project17-allocation-control/`; the
valid model's processing and final-corpus acceptance remain unresolved.

The scheduled semantic61 Project17 allocation capture subsequently completed
in 1,472.89 seconds, with 1,333.95 seconds of child CPU and 4,201.61 MiB peak
RSS. All fourteen thesis rules completed with zero errors and limitations;
status 1 reflects 131,704 warnings. The loaded closure contains 484 files and
657 include edges, and dropping the analysis restores the allocation baseline.
The full diagnostic stream matches the retained earlier Project17 stream.
Independent physical checks confirm all six retained streams, 1,039 original
inputs, 90 current sources and 29 artifacts. The pair still supplies no valid
allocation ratio because semantic60 was censored. The focused allocation
comparison and matched native measurements remain pending; base-083 is open.

The focused semantic60/61 comparison now passes on compiler-valid models with
10 and 100 distinct assumptions and comparison expressions. Both public
GuardedFacts entry points preserve their full ordered output, including unknown
query truth and every original assumption range. Native diagnostics and all
fourteen rule outcomes match; each probe restores its allocation baseline.
The eligibility change avoids 380 allocation calls and 39,520 requested bytes
at size 10, and 39,800 calls and 4,139,200 bytes at size 100. Retained and peak
tracked allocations are unchanged. These measured savings support retaining
the small change; single native timings establish no CPU improvement. Evidence
is retained in `target/benchmarks/base083/semantic61-alias-growth-control/`.
Matched Project17 native measurements and final-corpus acceptance remain open.

The retained semantic45 and semantic51 source prechecks identify 22 original
corpus models rejected by the current MiniZinc compiler: six pass strings to
`int_search` annotation arguments and sixteen reference an undefined `is_output`
identifier. Their terminal receipts, complete compiler streams, source hashes
and compiler/standard-library identities were rechecked. Reuse these exact
classifications when matching final corpus rows; they do not classify other
models or turn a Zincite timeout into invalid input. The old 5,595/822 physical
availability split also does not describe current restored dependencies.

The fresh unsampled Project17 native pair completes within the unchanged
1,800-second child deadlines. Semantic60 takes 1,257.20 seconds elapsed and
1,238.68 seconds of child CPU, with 4,439.28 MiB peak RSS; semantic61 takes
1,338.35 seconds elapsed and 1,322.09 seconds of child CPU, with 4,487.58 MiB
peak RSS. Both statuses are 1 and both complete diagnostic streams match the
completed fourteen-rule semantic61 probe. The single pair shows no CPU or RSS
improvement; the focused allocation savings remain the reason to retain the
eligibility change. Independent checks preserve all 1,039 original inputs,
90 sources, 32 artifacts and six streams, with the outer process group reaped
and empty. Evidence is retained in
`target/benchmarks/base083/semantic61-project17-native-thesis-control/`.
Final expanded-corpus acceptance and whole-task verification remain pending.

The exhaustive semantic61 capture uses the same ordered 6,417 original inputs
in 124 serial batches, with large models isolated into singleton batches.
Each batch runs the native tool and library companion for both the fourteen-rule
thesis preset and the twenty-six-rule all preset. The child deadlines remain
1,800 seconds; no solver runs are involved. The first six batches captured
384 roots per preset without timeouts or unobserved rule outcomes. Their native
and companion diagnostic streams match, reported allocation drops restore the
baseline, and all six outer process groups were reaped and empty. Evidence is
retained in `target/benchmarks/base083/exhaustive-semantic61/`.

These captures still report input errors and semantic limitations. They do not
establish acceptance of those roots or completion of base-083. Reconciliation
must check the exact originals with the current MiniZinc compiler, distinguish
missing include configuration from rejected input, and repair demonstrated
coverage gaps. Early candidates include filtered numeric aggregates, named
local array index sets, relational `abs` and enum-successor expressions,
checked `trace` forwarding, and set membership. None is accepted merely because
its syntax parsed or its capture finished. The remaining corpus batches and
independent whole-task verification are pending.

Fresh compiler-only checks now validate the retained ATSP and carpet-cutting
2021 model/data pairs against MiniZinc 2.10.1 and the current Gecode backend.
Both compilations return zero within the 60-second cap and produce nonempty
FlatZinc; neither invokes solving. The corresponding local corpus model copies
are byte-identical. All 1,040 checked compiler, library and input files remain
unchanged. Receipts and complete streams are retained in
`target/benchmarks/base083/semantic61-current-pair-prechecks/`. These two short
checks overlapped the serial exhaustive capture; affected timing measurements
must not be presented as isolated performance controls. Their success confirms
compiler validity, not Zincite rule completion or formatting equivalence.

The exact local ZipQueens and Chinese jobshop originals now pass current
MiniZinc source checks with empty diagnostic streams. The self-contained English
count1 original compiles with Gecode to nonempty FlatZinc without solving; its
compiler stream contains the model's trace messages. All three checks complete
within the 60-second cap and preserve 1,039 compiler, library and input pins.
Evidence is retained in
`target/benchmarks/base083/semantic61-current-source-prechecks/`. Source-only
checks establish parsing and typing, not instance compilation or formatting
equivalence. These checks also overlapped the exhaustive capture. The located
`abs`, enum-successor and `trace` gaps still require Zincite repairs.

Fresh Gecode compilations also accept the exact English meal and social-golfers
models with their existing data files, producing nonempty FlatZinc without
solving. Meal emits enum-to-integer coercion warnings, including its three
membership expressions; the current compiler accepts that coercion, so the
Int/Enum difference is not an input rejection. Depot-placement 2011 passes a
fresh source check with empty streams. All three checks preserve 1,041 compiler,
library and input pins and finish within the 60-second cap. Evidence is retained
in `target/benchmarks/base083/semantic61-scalar-membership-prechecks/`; the
overlap with the exhaustive capture is recorded. Their set-membership and
scalar-maximum Zincite gaps remain unresolved.

Current compiler source prechecks now cover all 263 complete, dependency-resolved
models with thesis limitations in the first eight captured batches. Three
parallel workers use the same compiler, Gecode configuration and standard
library, with a 60-second cap per model and no solving. All checks finish
without timeouts: 239 pass, while eight reject string arguments to `int_search`
and sixteen reject undefined `is_output`. Every rejected stream was inspected;
these classifications apply only to the exact checked originals. All 526
retained streams and the ordered selection were checked, and 1,527 compiler,
library and closure files remain unchanged. Evidence is retained in
`target/benchmarks/base083/semantic61-first-eight-source-prechecks/`. These
overlapping source checks establish neither instance compilation nor formatting
equivalence. The compiler-accepted models' Zincite limitations remain work for
base-083, alongside the uncaptured remainder of the corpus.

The ninth batch's native thesis check reaches the unchanged 1,800-second cap
and is reaped with status -9 after 1,800.02 seconds. Its retained stdout is
empty and stderr contains 287,741 bytes; both complete stream hashes were
checked. The native interface provides no completed-root count, so this receipt
does not identify which input caused the long run. The scheduled companion and
all-rule captures continue separately. Preserve this processing failure and any
unobserved outcomes; neither a timeout nor partial diagnostics establish that
the corresponding original model is compiler-invalid. Evidence remains in
`target/benchmarks/base083/exhaustive-semantic61/shard-008/`.

The companion's current `sysadmin_4_2s.mzn` input from the 2020 BNN-planner
submission passes a fresh Gecode compilation with MiniZinc 2.10.1. Compilation
returns zero in 0.436 seconds, produces 302,666 bytes of FlatZinc, and has empty
stdout and stderr; no solving occurs. The 1,037 compiler, library and input pins
remain unchanged, and both output artifacts match their retained hashes.
Evidence is retained in
`target/benchmarks/base083/semantic61-bnn-source-precheck/`. This bounded check
overlapped the exhaustive capture. The model is compiler-valid; the companion's
observed analysis phase does not establish the cause of the earlier native
timeout. Corpus capture and Zincite repairs remain pending.

The ninth batch's thesis companion also reaches its unchanged 1,800-second
deadline and is reaped with status -9. Its complete retained streams match the
receipt hashes. It records 52 roots and 52 restored drop snapshots; each of the
14 rule partitions accounts for all 64 selected roots, including 12 explicitly
Unobserved roots. Analysis begins for the compiler-valid BNN planner model and
does not finish before the cutoff. This is an unresolved valid-processing
failure, not an invalid-input exclusion; the following eleven roots remain
unobserved too. The scheduled all-rule native capture continues, without a
retry or deadline change. The ninth batch is not complete or accepted.

Current MiniZinc source prechecks accept all 44 newly captured complete,
dependency-resolved models with thesis limitations in the ninth batch's observed
prefix. Three parallel compiler workers use Gecode and the current standard
library, with the same 60-second cap and no solving. All checks return zero
without timeouts; 88 complete streams were verified and 1,080 compiler, library
and closure files remain unchanged. Evidence is retained in
`target/benchmarks/base083/semantic61-ninth-prefix-source-prechecks/`. These
overlapping source checks establish parsing and typing, not instance compilation
or Zincite semantic completion. Their located limitations remain required work.

A single one-second stack sample of the ninth batch's live all-rule native
process captures 80 main-thread samples. All 80 collapsed top frames are
`callables::operation_fact`, reached through `resolve_integer_bounds` and
recursive `Bounds::walk`/`Bounds::expression`. Source inspection confirms that
the lookup linearly scans the call vector for each operation. This directly
supports a bounded experiment with a first-match call index in the existing
bounds evaluator; it does not identify the native process's current root or
attribute its whole run or the earlier timeout. The sample and receipt remain
in `target/benchmarks/base083/semantic61-shard008-all-native-sample*`; 90 source
pins and the native binary remain unchanged. The sampled run's timing is not
an isolated performance control. Corpus scheduling and deadlines are unchanged.

Two additional matching gaps are located in the compiler-accepted ninth-batch
prefix. `rel2onto.mzn:110:19` supplies a parameter Int set to `sum`, while
`ecp.mzn:30:33` supplies a rank-two parameter Int array to `length`; Zincite
reports no matching declaration at both original call heads. Current matching
has neither general parameter-set-to-array correspondence nor the required
multidimensional `length` view. MiniZinc's retained type checker inserts
`set2array` for present parameter sets, rejecting decision/optional sets and
targets above rank one. These are matching repairs to investigate, with actual
argument types, enum identities and ambiguity preserved. A matching view alone
must not establish nonemptiness, index membership or whole-array definitions.

The ninth batch's all-rule native attempt also reaches its unchanged
1,800-second cap and is reaped with status -9 after 1,800.013 seconds. Its
complete streams match the receipt: empty stdout and 1,054,780 bytes of stderr.
The earlier stack sample applies only to its sampled interval; neither that
sample nor the timeout identifies a native current root or proves whole-run
cost attribution. The scheduled all-rule companion continues separately.
Both native cutoffs remain unresolved processing failures, with no deadline
relaxation, invalid-input waiver or task-completion claim.

A second one-second sample targets the all-rule companion while its exact BNN
input is in analysis. Before and after sampling, the stream has 52 root/drop
records followed by that model's load/analyze begin events, with no finish.
Of 80 main-thread samples, 78 collapsed top frames are `operation_fact`, reached
through integer-bounds analysis. This corroborates the lookup cost on a known
compiler-valid input, without attributing the whole run or any native current
root. The full sample and receipt are retained in
`target/benchmarks/base083/semantic61-shard008-bnn-companion-sample*`; source
pins and the companion binary remain unchanged. Sampling overlap is explicit,
and the current capture continues under its original deadlines.

An unapplied lookup repair is prepared in
`target/benchmarks/base083/semantic62-bounds-call-index.patch`. It adds one
first-match `(file, operation-head start)` index inside `Bounds`, retaining
the existing outcome interpreter and diagnostic order. The revised patch passes
`git apply --check` and a Rustfmt check of its temporary candidate; all 90 actual
source pins remain unchanged. Compilation, allocation/native measurements and
behavioral validation remain pending. This prepared patch is not an applied
repair or whole-task acceptance.

The disposable candidate workspace now passes an offline all-target Cargo check
in 6.225 seconds, returning zero without a timeout. The first attempt failed
because the temporary copy omitted fixtures referenced by `include_str!`; its
receipt remains retained. Adding the eleven tracked fixtures corrects that copy
boundary. The second attempt's full streams were checked against its receipt,
and all 106 copied files were checked: only the intended `domains.rs` candidate
differs from the originals. All 90 actual sources, twelve capture helpers and
five gate binaries remain unchanged. Evidence is retained in
`target/benchmarks/base083/semantic62-draft-workspace-validation2/`. This is
compilation of an unapplied candidate, with corpus overlap, not an authoritative
workspace gate, behavioral proof or performance result.

The ninth batch is now closed. Its outer driver returns one after 7,201.088
seconds, without reaching its outer deadline, and is reaped with its process
group empty. The raw cleanup EPERM remains recorded. All four scheduled
attempts reached their own unchanged 1,800-second caps. The final all-rule
companion is reaped with status -9 after 1,800.013 seconds; its full streams
match the receipt, including 1,674,878 bytes of stdout and 1,054,780 bytes of
stderr. Both companions report 52 complete, dependency-resolved root/drop pairs
and twelve unobserved roots, with every selected-rule partition accounting for
all 64 roots. The last unfinished phase remains the compiler-valid BNN model's
analysis. The batch rechecks 1,086 original files with no changes and unchanged
binaries; capture acceptance is false and semantic acceptance remains unset.
The original coordinator has started the tenth batch. No process was stopped,
deadline changed or unresolved root excluded.

The same disposable candidate also passes the six existing `domains` and
`numeric_facts` public tests, with zero failures or ignored tests. The bounded
offline command returns zero in 9.884 seconds without a timeout and is reaped
with an empty process group. Full stdout/stderr match the retained receipt in
`target/benchmarks/base083/semantic62-draft-workspace-focused-tests/`; all 106
copied inputs and 90 actual Rust sources were checked unchanged afterward.
These tests cover conservative bounds, cycles, unsafe definitions, unknown
domains and retained locations. They support the prepared lookup change, but
do not establish full BNN processing, corpus parity or performance improvement.

The tenth batch closes normally: the outer driver is reaped with status zero
after 708.492 seconds and an empty process group. Both native and both companion
attempts finish within their original caps. Both companions retain all 64
ordered roots and restored drop baselines, with no unobserved roots; every
14/26-rule partition totals 64 and the common fourteen rule rows agree exactly.
There are 63 structurally complete, resolved roots. Eight complete roots have
every thesis rule Completed; none has every all-preset rule Completed. Thesis
and all retain 2,732 and 5,064 limitations respectively, so successful capture
does not establish semantic acceptance. Full streams match all four receipts,
and the 1,086 original-file hashes were independently rechecked unchanged.
Evidence remains in `target/benchmarks/base083/exhaustive-semantic61/shard-009/`.

The lookup-only disposable workspace produces release native and companion
binaries in a bounded offline build, returning zero after 15.027 seconds.
Full build streams, Cargo artifact paths and binary hashes were checked; the
106 copied inputs remain unchanged. Evidence is retained in
`target/benchmarks/base083/semantic62-draft-workspace-release/`. These binaries
are reserved for candidate controls and do not replace the live capture's gates.

A separate unapplied proposal in
`target/benchmarks/base083/parameter-set-array-matching-proposal.patch` supplies
a candidate-local rank-one array view for present parameter Int/Enum sets.
It retains enum identity and an Int axis, including competing user overloads;
existing ambiguity and unknown-candidate handling remain in place. It adds no
cardinality, membership or output facts. The patch passes `git apply --check`;
compilation and behavioral checks remain pending. Other element types remain
explicitly outside this bounded proposal, without a compiler-invalidity claim.

The lookup-only candidate's single all-rule BNN control reaches its unchanged
300-second cap and is reaped with status -9 after 300.019 seconds, with an empty
process group. Its full streams match the receipt: 2,142 bytes of stdout contain
only the sole-model manifest and load/analyze begin records; stderr is empty.
There is no root, drop or completion record, so this is an unresolved processing
cutoff, not behavior success or a before/after performance result. All 1,037
compiler/library/model inputs, actual and copied sources, helpers, original gates
and candidate binaries were independently rehashed unchanged. Evidence remains
in `target/benchmarks/base083/semantic62-draft-bnn-all-control/`.

A one-second sample of that candidate captures 88 main-thread frames in callable
definition/body analysis; collapsed top frames include `expression_type` and
`operation_fact`, with 36 samples each. The sample is retained in
`target/benchmarks/base083/semantic62-draft-bnn-all-sample*`. It identifies residual
lookup work in this window without attributing whole-run cost. Source inspection
shows the public callable-definition resolver constructs `Producer` without its
existing direct-safety lookup maps, while its root lookup helpers already have
indexed paths. Reusing those paths is the next bounded cost repair to evaluate.

Another unapplied type proposal in
`target/benchmarks/base083/full-axis-slice-proposal.patch` recognizes literal
inclusive `..` array selectors, retaining each selected axis and element type.
Mixed decision selectors remain unsupported when any axis is sliced, matching
the current compiler's type checker. Scalar-only selection retains its existing
behavior. The standalone candidate passes Rustfmt and `git apply --check`;
compilation, required access-safety support and behavior remain pending. This
shape proposal establishes no membership, cardinality or output guarantee.

Three further closed batches, indices 010–012, each retain all 64 roots and drop
records in both presets, with no unobserved roots. Their 14/26-rule partitions,
common-rule rows, twelve full stream pairs and original-file hashes were checked
against the retained reports. Syntax errors and unavailable dependencies remain
explicit; complete capture does not establish semantic acceptance. Audit totals
remain in `target/benchmarks/base083/exhaustive-semantic61/shards010-012-root-audit.json`.

Current MiniZinc source checks reject two newly located parser failures at the
same syntax: `2024/train-scheduling/trains.mzn:47:74` has a comma inside written
string interpolation, and the older local time instance has a missing comma
before line 88. Both parallel Gecode source checks return one without timeouts,
are reaped with empty process groups, and retain raw cleanup EPERM records.
All four complete streams and 1,040 input/helper hashes were checked; no solve
was run. Evidence is in
`target/benchmarks/base083/semantic61-new-parser-source-prechecks/`. These current
compiler syntax rejections do not require Zincite to accept the broken originals.
They do not classify the other errors or unavailable includes in those batches.

The next disposable candidate reuses the existing callable-definition lookup
maps in its public resolver and routes its call inspections through the existing
indexed helper. First-row matching, file/range keys and instance-view fallback
remain unchanged. In the copy, all-target compilation and eleven existing
callable, definition and search tests pass under separate 300-second caps,
without timeouts or failures. Full streams, before/after pins and retained
Bounds-only binary snapshots were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-lookup-{check,callables,definitions,search}/`.
Only the copy's two lookup repairs are included; these checks establish neither
authoritative workspace validation nor final corpus acceptance.

The full-axis type draft's v2 also assigns the written `..` marker its owning
axis's set type before child inspection. This avoids a spurious zero-argument
range call while preserving Enum identity and instance-local typing. The initial
draft is retained, and v2 passes standalone Rustfmt and patch applicability checks.
Consumer inspection and guarded membership remain separate required repairs.
The prepared parameter-set aggregate inspector likewise retains Unknown after
inspection, without output guarantees; rank-two Boolean filters and additional
aggregate forms remain located support gaps rather than implicitly supported.


The public-lookup candidate's second BNN control also reaches its unchanged
300-second cap, is reaped with status -9 after 300.028 seconds and leaves an
empty process group. Its full streams again contain only the model manifest
and load/analyze begin records: no root, drop or completion record. All 26
selected rules remain Unobserved. The original failed outer accounting receipt
is retained separately from its corrected saved accounting; no retry occurred.
The compiler/model, authoritative source and helper pins were independently
rehashed unchanged. Evidence is in
`target/benchmarks/base083/semantic62-draft-bnn-all-lookup-control/`.

A one-second sample locates repeated source-location construction in expression
lookup predicates. A third cost draft uses the existing indexed expression-type
helper and computes byte ranges directly where only those ranges are needed.
In the disposable copy, all-target compilation and seven existing definition
and search tests pass. Their complete streams and 317 source, copy, helper and
patch pins were independently checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-range-{check,definitions,search}/`.
These copy checks do not establish authoritative workspace validation or a
performance improvement; the next bounded control result is pending.

Six additional closed batches, indices 013–018, contain respectively
64, 64, 39, 1, 64 and 22 roots. Both presets retain all 254 ordered root and drop
records, with no Unobserved roots. Their 14/26-rule partitions, common-rule rows,
24 full stream pairs and 1,276 unique original-file hashes were independently
checked. Audit totals remain in
`target/benchmarks/base083/exhaustive-semantic61/shards013-018-root-audit.json`.
Three reported errors still need current-compiler classification; semantic
acceptance remains unproven.


The three-cost-patch BNN candidate completes its bounded all-rule control in
163.633 seconds, without timeout, with its child reaped and process group empty.
It captures one complete, dependency-resolved root, all 26 rule rows and a drop
back to baseline. Twenty-three rules complete; unbounded-variable,
search-coverage and vacuous-constraint remain Limited. There are 1,094 warnings,
no errors and 561 limitations. Full streams and 1,353 compiler, model, original,
copy, helper and candidate-binary pins were independently checked. Evidence is
in `target/benchmarks/base083/semantic62-draft-bnn-all-range-control/`.
The sampled overlapping run does not establish a performance ratio or semantic
parity with the earlier timed-out candidates. The newest one-second sample
locates remaining work in index-set checking and its obligation lookup.

Current MiniZinc rejects the newly reported tuple-interpolation comma syntax.
The parameter-file model also lacks its required include in this configuration,
which remains an unavailable dependency rather than invalid syntax. The
crossword data passes Gecode instance checking with its companion model, without
a solver run. Its non-UTF-8 bytes remain outside Zincite's explicit UTF-8 input
boundary in the brief; this is an encoding exclusion, not compiler invalidity.
The initial data-only and empty-model checks are retained as inconclusive.
Evidence remains in `target/benchmarks/base083/semantic61-shards013-018-source-prechecks/`
and `semantic61-shard017-data-{parser,instance}-precheck/`.


A small BNN reduction isolates the remaining array-access definition limitation.
Both present Boolean and bounded integer arrays compile to FlatZinc with current
Gecode settings, without solving. For `objective = 1*x[1]` in a `1..4` array,
phase-61 analysis reports two definition limitations for the Boolean case and
none for the integer case. The out-of-bounds Boolean control is also compiler
accepted with a partiality warning and retains Zincite's findings; it is not
classified invalid. Six complete compiler/probe stream pairs and 1,041 input
pins were checked. Evidence is in
`target/benchmarks/base083/semantic62-bnn-array-access-reduction/`.
This isolates a required support investigation without relaxing membership or
partiality checks.


The disposable copy now combines the six reviewed set/full-axis drafts with the
three cost repairs. All-target compilation and formatting pass. Sixteen existing
callable, definition, guarded and index-policy tests pass, including the small
new overload ambiguity/unknown-veto and full-axis Enum/rank/decision-selector
checks added within existing groups. Their complete successful streams were
checked; 314 current original, copy and helper pins were independently rehashed.
Intermediate diagnostic-wording and fixture arrangement/private-constructor
failures remain retained; the final cases preserve the original negative
expectations. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-semantic-*`.
No authoritative Rust source has changed. A bounded six-model all-rule control
is the next check; other aggregate, membership and local-axis gaps remain open.


The six-model candidate control completes in 8.046 seconds with all ordered
root/drop records, 26-rule partitions and no missing roots or errors. Its full
streams and 1,908 original, copy, compiler, helper and patch pins were checked.
There are still 538 limitations. Exact phase-61 row joins show lower counts in
three slice-focused cases, an unchanged count in the fourth, and higher counts
in the two set cases; this is not blanket behavior acceptance. Evidence is in
`target/benchmarks/base083/semantic62-draft-slice-set-six-control/`.

A compiler reduction confirms a matching regression in the set-to-array draft:
with `choose(set of int)=1` and `choose(array[int] of int)=2`, MiniZinc compiles
`choose({1,2})` with output value one, while the retained candidate reports
ambiguity. The matching view fails to distinguish the conversion from a direct
set match. The candidate and all successful/failed checks are retained while
ranking is repaired. Evidence is in
`target/benchmarks/base083/semantic62-set-array-overload-control/`.

The disposable copy also passes the focused Boolean-array and scalar integer
min/max public cases in the existing numeric and search groups (four tests per
group). In-bounds Boolean selections retain unknown numeric values and search
dependencies; out-of-bounds or symbolic axes retain unsupported outcomes.
Scalar extrema preserve both input dependencies and reject partial children.
Searching one array does not cover a separate unsearched array or its selected
value. Complete streams, equal before/after maps and 468 original, source,
helper and patch pins were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-minmax-bool-public-v2-{numeric_facts,search}/`.
These checks do not establish corpus acceptance or complete base-083. The
matching correction remains under separate copy-only validation.

The corrected matching copy passes formatting, all-target compilation and all
four callable tests. The added direct-Set case now resolves to the Set formal;
the existing crossed-array ambiguity and unknown-signature veto still pass.
Complete streams and their hashes, terminal process accounting and equal
before/after maps were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-set-ranking-v2-{fmt,check,callables}/`.
Two further current MiniZinc/Gecode compile-only controls select the direct Set
family for SetFloat versus ArrayInt and var SetInt versus par ArrayInt. Both
generated output models retain `chosen = 1`. The full streams, generated
artifacts and 1,041 unchanged input pins were checked. Evidence is in
`target/benchmarks/base083/semantic62-set-array-ranking-compiler-controls/`.
Fresh candidate model outcomes remain pending.

The release-five candidate completes the seven-root comparison with all roots,
ordered drops and 26-rule rows accounted for. The direct-Set reduction has no
limitations. The same six original models retain 28/54/11/98/48/215 limitations,
versus 32/61/11/101/67/266 in the earlier candidate. Rel2onto and routing now
complete unused-declaration and element-predicate analysis; routing also
restores constant-variable completion. All 727 diagnostics, including multiline
messages, complete streams and 1,934 input/helper/patch pins were checked.
Evidence is in
`target/benchmarks/base083/semantic62-draft-release5-ranking-slice-set-control/`.

The separate four-root comparison accounts for all roots and 26-rule rows.
Both in-bounds Boolean and integer reductions complete every rule with no
warnings or limitations. The out-of-bounds Boolean case retains three warnings
and two limitations. Depot2011 retains 59 warnings and eight limitations,
including numeric and guarded inspection of its scalar `max(ADist,BDist)`.
Complete streams, all 72 diagnostics and 1,951 input/helper/patch pins were
checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-release5-minmax-bool-control/`.
These overlapping bounded runs establish behavior only; remaining support gaps,
performance attribution and full corpus acceptance remain open.

The frozen release-five candidate completes the original `sysadmin_4_2s.mzn`
BNN control in 106.386 seconds under the unchanged 300-second cap. Its complete
resolved root, all 26 rule rows and restored drop are accounted for: 25 rules
complete; vacuous-constraint retains one limitation for `show` in the output
condition at 568:62. There are 1,110 warnings and no errors or missing roots.
Complete streams, all 1,111 diagnostics and 1,934 input/helper/patch pins were
checked. Compared with the earlier three-cost candidate, 561 limitations become
one and 16 vacuous-constraint warnings are added at distinct written constraints.
Those source expressions occur in duplicate pairs in the original input; the
warnings retain their contextual conditions and prohibit automatic removal.
Evidence is in
`target/benchmarks/base083/semantic62-draft-release5-bnn-all-control/`.
The candidates differ semantically and overlap the original sweep; their wall
times do not establish a performance ratio or matched cost-only result.

The current candidate code and public cases are saved in
`semantic-candidate.patch`, relative to the tracked Rust source at commit
`50b9288`. The patch contains all ten changed Rust files in the disposable copy
and passes `git apply --check`; its SHA-256 is
`53f21209032d44414ea5f563a7a86ed49e5e6accb325aa42ac6e483e922b2b3f`.
This checkpoint preserves the code while the original corpus sweep continues
to check its frozen source files. Apply it only after that run no longer depends
on the original source. It does not mark base-083 complete.

The latest length and scalar min/max inspection changes pass formatting,
all-target compilation and the existing guarded, numeric and search groups
(14 tests). They preserve unknown values, reject unsafe operands and retain
annotation restrictions. Complete check streams, terminal process accounting,
equal before/after maps and 358 input, copy, helper and patch pins were checked.
Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-length-minmax-v4-{fmt,numeric_facts}/`
and `semantic62-draft-workspace-length-minmax-v2-{check,guarded,search}/`.
Earlier failed checks remain retained; the final fixture correction supplies
the range signature only in the new fixture context. Release-model comparisons,
whole-workspace validation and independent task verification remain pending.

The root audit of original-sweep batches 019–023 reads all 50 capture and driver
streams, verifies hashes for the 40 receipt-bound native/companion streams and
rehashes 1,279 original inputs. Native and companion diagnostics match; observed
roots, ordered drops and the common 14 rule rows agree. Batch 019 remains
unobserved after its four 1,800-second timeouts on compiler-valid
`inventory_4_8s.mzn`; batches 020–023 account for all 64 roots each.
Evidence is in `target/benchmarks/base083/exhaustive-semantic61/shards019-023-root-audit.json`.

Batch 022's 38 parser errors come from two inputs: one diagnostic in
`enum-type-coercion.mzn` and 37 in the MiniZinc documentation's `grammar.mzn`.
Two current MiniZinc 2.10.1/Gecode compile-only checks reject both for syntax:
the first is missing a closing parenthesis; the second contains grammar
notation rather than a model. Both checks terminate without a timeout, preserve
all 1,039 compiler, standard-library, helper and original-source pins, and run
no solver. These inputs are outside required valid-input compatibility under
the compiler pre-check rule. The errors remain in the original capture.
Evidence is in `target/benchmarks/base083/semantic61-shard022-parser-prechecks/`.

The candidate patch now includes present scalar Boolean `show` inspection,
removal of adjacent identical input-precondition limitations, and early length
identity checks using the existing call indexes. Rendering preserves child
partiality and supplies no string or truth value. The diagnostic repair retains
both dimension obligations and the Limited outcome; it changes only identical
adjacent limitation messages. Length classification keeps its semantic guards.
Formatting, all-target compilation and the guarded, numeric, vacuous-constraint
and input-precondition groups pass (16 tests). Complete streams, terminal
receipts, unchanged maps and the source/helper/patch and frozen-binary pins were
checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-show-adjacent-*/`.
The refreshed `semantic-candidate.patch` contains 13 Rust files, passes
`git apply --check` and has SHA-256
`545be5d1469de7144a1e5b2f45f0ec618b4160a0590ff88c5b1851e91292c6a8`.
Full workspace gates and final release/corpus checks remain pending.

The frozen candidate passes all required workspace gates: formatting, Clippy
with warnings denied, and 193 tests across 42 successful suites, with no failures
or ignored tests. Complete Clippy and test streams, terminal accounting and
unchanged source/helper/patch maps were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-show-adjacent-final-{clippy,workspace}/`.
Its release-six build succeeds and freezes the native linter and companion
probe in `semantic62-draft-length-minmax-show-binaries/`; both Cargo artifact
events use optimization level three. Release-model outcomes remain pending.

The root audit of batches 024–026 accounts for all 94 roots per preset, reads
30 complete streams, verifies 24 receipt-bound streams and rehashes 1,116
original inputs. There are no parser errors. The generated
`ProjectPlannertest_15_6.mzn` compiles successfully with current MiniZinc/Gecode,
without solving, and all 1,038 compiler/source/helper pins remain unchanged.
The original checker completes all 14 thesis rules on this model. Its all-rule
preset retains 855 limitations: 254 suspicious-domain, four index-set-mismatch,
four partial-expression and 593 vacuous-constraint. Its 32,908
decision-variable-operator warnings have 32,908 distinct written source ranges;
this capture does not show analysis multiplying that advice. Fresh candidate
outcomes for this compiler-valid model remain pending. Evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shards024-026-root-audit.json`,
`shard026-diagnostic-review.json` and
`target/benchmarks/base083/semantic61-shard026-generated-source-precheck/`.

Release six completes all 26 rules on the original `sysadmin_4_2s.mzn` BNN
control. Its 1,110 warning messages and order are byte-identical to release five;
only the single `show` limitation at 568:62 is removed. All other 25 rule rows
are unchanged. The complete resolved root, ordered drop and final record agree;
live bytes return to 2,378. The child terminates without a timeout under the
unchanged 300-second cap. Evidence is in
`target/benchmarks/base083/semantic62-draft-release6-bnn-all-control/`.

The Depot/routing pair completes both original roots. Depot retains 59 warnings
and six limitations, down from eight after scalar extrema inspection. Routing
retains 180 warnings and 215 limitations; its warning messages/order and
limitation locations are unchanged, with three limitation reasons becoming more
specific. The generated `ProjectPlannertest_15_6.mzn` retains the original
34,581 warnings and 855 limitations. Its full source and analysis JSON match the
original sweep, and its warning messages/order and limitation locations/order
match. Of its limitation reason messages, 507 change from generic integer
interpretation to the scalar-extremum fallback wording. This is no support
improvement; scalar `bool2int` remains a located missing numeric inspection seam.

All six comparison streams, terminal receipts, exact selected IDs, complete
roots, restored drops and unchanged input/source/helper/patch maps were checked.
The three groups ran concurrently with the original corpus sweep; their wall
times establish no performance ratio. Evidence is in
`target/benchmarks/base083/semantic62-draft-release6-depot-routing-control/`
and `semantic62-draft-release6-generated-all-control/`.
Remaining valid-model limitations keep base-083 acceptance open.

The saved candidate now inspects exact selected present scalar core `bool2int`
without claiming a value. Its source check follows initializers, written domains,
annotations and cycles; opaque initialized Boolean references remain unsupported.
Successful inspection yields an unknown numeric value, including literal inputs.
Optional inputs, user overloads, unsafe children and declaration/call annotations
retain their vetoes. Independent integer-bounds APIs remain unchanged.
Formatting, all-target compilation and the existing numeric, guarded and search
groups pass (14 tests). The first formatting failure and its line-wrap correction
are retained. Complete streams, terminal receipts, equal maps and 369 current
source/helper/patch pins were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-bool2int-v2-*/`.
The refreshed `semantic-candidate.patch` passes `git apply --check` and has SHA-256
`3e6895086f36d819b5d28fa76f628c79c8820e58aae70c94876b91718efbf4f3`.
Full workspace and original-model outcomes for this change remain pending.
