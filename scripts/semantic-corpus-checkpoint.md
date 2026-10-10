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

The full-axis slice binding repair is integrated. Bare `..` array selectors no
longer create callable demands; subjects, other selectors and bounded ranges
retain their normal binding traversal. The focused regression fails on the
original source and passes with the repair, including reachable dependencies,
one genuinely unused declaration and a located unresolved-selector refusal.
Both private builds used separate fresh Cargo targets. Source-only MiniZinc
controls pass. The original AssignmentOffers comparison removes its
unused-declaration limitations while preserving separate search/domain gaps.
Integrated-main formatting, Clippy and workspace tests pass in
`target/benchmarks/base083/semantic67-full-axis-main-gates-v1`.
This Rust change requires fresh final evidence; base-083 remains open.

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

The Boolean-conversion candidate also passes Clippy with warnings denied and
all 193 workspace tests across 42 successful suites. Full streams, terminal
receipts and unchanged maps were checked. Release seven freezes its native
linter and companion probe; the generated-model recapture is still running.
Evidence is in `target/benchmarks/base083/semantic62-draft-workspace-bool2int-final-{clippy,workspace}/`
and `semantic62-draft-bool2int-binaries/`.

The root audit of original batches 027–028 accounts for 86 roots per preset,
reads all 20 streams, verifies 16 receipt-bound streams and rehashes 1,108
original inputs. The one input error is a non-UTF-8 crossword data file at byte
1,510,046. It remains outside the brief's UTF-8 boundary and is not classified
as compiler-invalid. Separately, original `sysadmin_5_4s.mzn` compiles successfully
with current MiniZinc/Gecode without solving; 1,038 pins remain unchanged.
Its old native thesis command times out at 1,800 seconds with no completion
observation. This compiler-valid processing failure remains open.
Evidence is in `target/benchmarks/base083/exhaustive-semantic61/shards027-028-root-audit.json`
and `target/benchmarks/base083/semantic61-shard029-bnn-source-precheck/`.

The isolated cost-four lookup repair is reconstructed against exact cost-three
source, with only the reviewed index-set-mismatch patch differing. Its first
matched pair completes with identical source metadata, entire analysis JSON,
all 26 selected IDs and all 375,901 stderr bytes. Both retain 1,094 warnings and
561 limitations, with restored drops and unchanged pins. Analyze allocation
calls rise by 16 and requested bytes by 23,723,964; retained and peak allocation
deltas remain 975,316 and 583,963,393 bytes. Instrumented child CPU is
153.639 versus 115.082 seconds and peak RSS 815.094 versus 814.297 MiB.
Original-sweep and Cargo overlap are retained. These measurements establish no
production-native benefit or growth result; the native pair is the next check.
Evidence is in `target/benchmarks/base083/semantic62-cost4-sysadmin-first-stage/`.

Release seven's generated-model recapture hits its fixed 300-second deadline
during analysis. The child is reaped with return code -9 after 300.023 seconds;
child CPU is 286.805 seconds and peak RSS is 726.625 MiB. The full 2,203-byte
stdout contains the all-rule manifest and load/analyze beginnings, with no
root, drop or completion record; stderr is empty. All 26 rule outcomes remain
unobserved. Stream hashes and equal before/after records were checked. This
cutoff is an open processing failure, not a successful semantic comparison.
No retry or deadline change was made. Evidence is in
`target/benchmarks/base083/semantic62-draft-release7-generated-all-control/`.

The matched ordinary native CLI pair for the isolated cost-four repair also
completes within 300 seconds. Both status-1 commands produce empty stdout and
the same 375,901 stderr bytes, matching the prior instrumented probes.
Cost-three/cost-four child CPU is 174.964/129.669 seconds, wall time
177.902/133.411 seconds and peak RSS 894.500/893.766 MiB. Complete streams,
commands, terminal receipts, equal maps and 358 distinct current file pins
were independently checked. Original-sweep and generated-behavior overlap
preclude an isolated speedup claim. Growth remains unverified; a matched
instrumented pair on the accepted larger inventory model is the next check.
Evidence is in `target/benchmarks/base083/semantic62-cost4-sysadmin-native-pair/`.

The saved candidate now lends Bounds' existing call and first-expression maps
to the initialized-source safety inspector. Per-file Value-reference rows
preserve containment and original first-row selection; a separate sparse Float
location set preserves the any-float veto without duplicating the full
expression map. Initializer, domain, annotation, optionality and cycle checks
are unchanged. Formatting, all-target compilation and 14 existing numeric,
guarded and search tests pass. All ten streams, terminal receipts, equal maps
and 302 distinct current pins were independently checked. Declaration syntax
traversal remains a possible residual cost; no timing improvement is claimed.
Evidence is in `target/benchmarks/base083/semantic62-initialized-source-lookups-preparation/`
and `semantic62-draft-workspace-initialized-source-lookups-*/`.
The refreshed `semantic-candidate.patch` passes `git apply --check` and has
SHA-256 `7c2d98ee621e9b4bd381a9546a80ace4f12f56856e6ff6db618cd807544cf390`.
Full workspace gates and the generated-model recapture remain pending for
this lookup repair.

The lookup candidate passes Clippy with warnings denied and all 193 workspace
tests across 42 successful suites. Its frozen release-eight generated-model
control completes within the unchanged 300-second deadline (238.277 seconds).
All 26 selected rules are observed: 23 Completed and three Limited, including
all fourteen thesis rules Completed. The root is complete and resolved, with
34,581 warnings, zero errors and 348 limitations; live bytes return to 2,408.

The full source metadata matches release six. Every warning block is byte
identical and in the same order. All 348 remaining limitation blocks also
retain their exact bytes and original order. The 507 removed limitations are
254 from suspicious-domain and 253 from vacuous-constraint, following the
Boolean-conversion inspection repair. Only suspicious-domain's rule row changes
from Limited to Completed; the other 25 rows match. Four complete streams,
terminal receipts, equal maps and 1,893 distinct current pins were checked.
The earlier release-seven cutoff remains retained. This single control does
not establish full corpus acceptance or an isolated performance improvement.
Evidence is in `target/benchmarks/base083/semantic62-draft-release8-generated-all-control/`
and `semantic62-draft-workspace-initialized-source-lookups-final-{clippy,workspace}/`.

The current Depot candidate is saved before further repair. It adds guarded
inspection for supported integer-array concatenation and selected standard
all-different calls. Formatting, all-target compilation, four search tests and
four callable tests pass in the isolated copy. The guarded group has five
passing tests and one failing new fixture; diagnostic assertion text is retained
in this checkpoint. This candidate has not passed final workspace validation.

The complete 14-file candidate is saved in `semantic-candidate.patch` (130,546
bytes, SHA-256 `fb86b25d66acceec8a15bf564514f5f4b989a4f5853857650b9b48aedb44aab3`).
The patch was independently reconstructed from the frozen copy and checked with
`git apply --check --whitespace=error`. All original copied-file and actual Rust
source pins remain unchanged. Evidence is in
`target/benchmarks/base083/semantic62-draft-depot-v2-merge/` and
`semantic62-draft-workspace-depot-v2-{fmt,check,guarded,search,callables}/`.
Actual Rust integration and task completion remain pending.

The Depot guarded fixture is corrected locally: its additional `index_set`
signature now accepts present `var int` arrays, consistent with the installed
standard library's `any` element contract. Existing shared fixtures and
production code are unchanged. Temporary diagnostic text is removed, and the
expected unknown Boolean result is retained. All six guarded tests pass;
complete streams, terminal receipt, equal maps and original source pins were
checked. The earlier failure remains retained. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-depot-v2-fixture-v2-guarded/`.
The refreshed candidate is 129,739 bytes, SHA-256
`44d94b6ab2503f9cca1a018d5d91e828beb8ae1afd0edf33e990933f5b90c5e0`,
and passes `git apply --check --whitespace=error`. Final workspace gates and
original Depot-model outcomes remain pending.

The corrected Depot candidate passes all final copy-workspace gates:
formatting, Clippy with warnings denied, and 193 tests across 42 successful
suites. Complete streams, successful terminal receipts, equal maps and the
106 copied-file plus 90 actual-source pins were checked. The workspace test
command completed in 41.129 seconds. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-depot-v2-fixture-v2-{fmt,final-clippy,final-workspace}/`.
The original Depot-model control, actual Rust integration and fresh final
corpus acceptance remain pending.

Frozen release nine builds successfully from the Depot candidate. Its native
linter and companion artifacts match the optimized Cargo events and copied
build outputs; source pins remain unchanged. The original Depot model completes
within the unchanged 300-second deadline (2.493 seconds), with all 26 rules
Completed, including all fourteen thesis rules. It retains 59 warnings, zero
errors and zero limitations; live bytes return to the 2,390-byte baseline.

Source metadata matches release six, and all 59 complete warning lines are
byte identical and retain their order. The six prior limitations are removed;
only search-coverage, expensive-comprehension and vacuous-constraint rule rows
change, from Limited to Completed. Complete streams, terminal receipts, equal
maps and 1,777 distinct current file pins were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release9/`,
`semantic62-draft-depot-inspection-binaries/` and
`semantic62-draft-release9-depot-control/`. This single-root result does not
establish full corpus acceptance or a timing improvement. Actual Rust
integration and fresh final corpus evidence remain pending.

The original sweep's shard 029 now has three checked terminal captures for
compiler-positive `sysadmin_5_4s.mzn`: thesis native, thesis companion and all
native each time out at the unchanged 1,800-second deadline. Both native
commands emit no output; the thesis companion emits only its manifest and
load/analyze beginnings, with no completed root or rule outcomes. All fourteen
thesis outcomes remain Unobserved. Complete receipt-bound streams were checked,
and the retained successful MiniZinc/Gecode no-solve precheck produces nonempty
FlatZinc with equal before/after maps. The all companion is still running;
this is partial shard evidence, not a completed shard or full sweep. These
valid-model processing failures keep acceptance open. Evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shard-029/` and
`semantic61-shard029-bnn-source-precheck/`.

Release nine's original routing-model baseline completes in 1.873 seconds under
the unchanged 300-second deadline. All 26 outcomes are observed, with 180
warnings, zero errors and 215 limitations. Complete source metadata and all rule
rows match release six. The entire 86,450-byte routing diagnostic stream,
including multiline limitations, is byte identical and in the same order.
Live bytes return to the 2,394-byte baseline. Full streams, terminal receipt,
equal maps and 1,777 current file pins were checked. This remains incomplete
semantic coverage; the named-range collection-length gap is under repair.
Evidence is in `target/benchmarks/base083/semantic62-draft-release9-routing-control/`.

The isolated cost-four inventory run completes under the unchanged 1,800-second
deadline: wall 1,616.910 seconds, child CPU 1,604.811 seconds and peak RSS
2,917.391 MiB. The complete resolved root observes all 26 rules, with 4,233
warnings, zero errors and 2,148 limitations; live bytes return to 2,380.
Analyze records 1,022,542,538 allocation calls and 76,538,435,123 requested bytes,
with retained/peak deltas of 3,581,673/2,056,316,089 bytes.

Cost three times out after 1,800.052 seconds without a root, drop or completion
record. The manifests match, but complete source/analysis/allocation comparisons
and matched growth remain unavailable. This pair does not establish semantic
parity, an isolated speedup or a completed growth check. Both full captures,
terminal receipts, equal maps and 1,148 distinct current absolute file pins were
checked. Original-sweep and observed peer-job overlap are retained. No extra
native run or deadline change was made. Evidence is in
`target/benchmarks/base083/semantic62-cost4-inventory-growth-probes/`.

All four original shard-029 captures are now terminal: thesis/all native and
thesis/all companion each time out at the unchanged 1,800-second deadline and
are reaped. All eight receipt-bound streams were checked. Both companions stop
after manifest/load/analyze beginnings, so all fourteen thesis and 26 expanded
outcomes remain Unobserved, with no root/drop/completion record. No native
completed-root count is inferred from empty output. All 1,038 retained compiler,
model and standard-source pins were rehashed unchanged. This remains a valid
processing failure; it does not close acceptance. Root evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shard029-root-terminal-audit.json`.

The routing named-range candidate is checkpointed in `semantic-candidate.patch`:
145,159 bytes, SHA-256
`23f2953252d20ad40c0662b08cec4ac5987deca6008d526a6469eb6f232975e1`,
covering 17 Rust files. The complete patch was reconstructed independently from
the frozen copy and passes `git apply --check --whitespace=error`.

The new array-index inspection handles the checked collection-length quotient
without granting a numeric bound or nonempty-domain fact. The focused domain
suite passes both tests, retaining limitations for annotated, opaque, optional,
partial and user-overloaded cases. Test cleanup removes internal-field assertions
while keeping public outcomes. Full streams and their hashes, the successful
terminal receipt, equal capture maps, 106 current copy-file pins and 90 actual
Rust source pins were checked. Evidence is in
`target/benchmarks/base083/semantic62-routing-index-start-preparation/full-candidate-v3.diff`
and `semantic62-draft-workspace-routing-index-v3-domains/`.

The actual Rust files remain unchanged while the original sweep runs. This is a
candidate checkpoint: final workspace gates, an original routing-model control,
actual integration and fresh full corpus acceptance remain pending.

Original sweep shards 030–036 have checked terminal native and companion
captures for 293 roots. All 56 receipt-bound streams and hashes were checked;
companion manifests, ordered root/drop records, final statuses and every rule
partition agree with their reports. All 1,315 distinct original input/dependency
pins remain unchanged. Evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shards030-036-root-audit.json`.

Shard 032 retains five unsupported data-item errors in `wedding_test.dzn` and
one unresolved include in `assignment-pool/tmp/analyze.mzn`. These are captured
errors, with compiler validity still unclassified. Other batches retain semantic
limitations; native completion counts remain unavailable. These original
phase-61 captures do not establish acceptance for the current candidate.

The routing candidate passes formatting, Clippy with warnings denied and all
193 workspace tests across 42 successful suites. Clippy first found three
needless borrows in the new test fixture; only those borrows were corrected,
and the failed capture is retained. Complete final streams, terminal receipts,
equal maps and 106 original/106 current-copy/90 actual-source pins were checked.
Workspace tests completed in 41.323 seconds. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-routing-index-final-v2-{fmt,clippy,workspace}/`.
The corrected 17-file patch is 145,156 bytes, SHA-256
`40e0405e92e45137ad7c915797bf3f90dba0fcb22fbfa6fb0abba9269db9be00`,
and passes `git apply --check --whitespace=error`. Original-model validation,
actual integration and fresh final corpus acceptance remain pending.

Original shards 037–038 have checked terminal captures for another 35 roots.
All 16 receipt-bound streams/hashes, ordered companion root/drop records,
manifest IDs, final statuses and exact partitions were checked, with 1,057
current original input/dependency pins unchanged. The large shard-037 model
completes all fourteen thesis and all 26 expanded rules with zero limitations,
retaining 94,878 thesis and 122,040 expanded warnings. Shard 038 retains two
thesis and 87 expanded limitations. Evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shards037-038-root-audit.json`.
These are original phase-61 captures; final-source acceptance remains pending.

Current MiniZinc no-solve checks classify the two shard-032 error inputs.
The actual wedding model/data pair rejects all five `.dzn` declaration items
as syntax errors. The assignment analysis model fails because
`model/instance-10-30.dzn` is unavailable; this does not establish syntax
invalidity. Both full streams, terminal receipts, equal maps and 1,043 current
compiler/original/standard/helper pins were checked. Exactly two compiler
children ran, without solving. Evidence is in
`target/benchmarks/base083/semantic62-shard032-compiler-classification/`.

Release ten builds successfully from the frozen routing candidate in 13.524
seconds. Full Cargo streams, successful terminal receipt, equal maps, optimized
non-test artifact events, frozen binary/origin equality and current source pins
were checked. Its frozen linter is 2,901,232 bytes, SHA-256
`148e865b17abd989fae752035fb601b7416b65933bf0198ff65f1d0283af9b52`;
the companion is 2,657,056 bytes, SHA-256
`6c0a38cc54d0b202ff70311867538f41121108083eabbd9727d5659145054854`.
Evidence is in `target/benchmarks/base083/semantic62-draft-workspace-semantic-release10/`
and `semantic62-draft-routing-index-binaries/`. Original routing validation,
actual integration and final corpus acceptance remain pending.

Release ten's original routing control completes in 2.404 seconds under the
unchanged 300-second deadline. All 26 outcomes are observed, with 180 warnings,
zero errors and 208 limitations. Source metadata is identical to release nine,
and live bytes return to the 2,394-byte baseline. Only `array-index-start`
changes its rule row, from Limited to Completed.

All 180 complete warning blocks are byte identical in the same order. Six
array-index limitations and one vacuous-constraint limitation are removed.
At line 129, search coverage changes its limitation from unsupported numeric
arithmetic to unproved division/remainder operands; it remains Limited.
The other 207 limitation blocks are unchanged and retain their order. Full
streams, terminal receipt, equal maps and 1,910 current distinct physical pins
were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-release10-routing-control/`, including
`root-block-audit.json`. This is a single-root partial-coverage result;
actual integration and final corpus acceptance remain pending.

The final bounded cost-four native diagnostic completes in 130.303 seconds,
with status and both full streams identical to the prior native control
(375,901-byte diagnostic stream). Its one authorized macOS sampling attempt
fails with status 255: the sampler cannot inspect the owned process. It produces
no sample file or call graph. Both receipts/full streams and equal maps were
checked; no retry or privilege escalation occurred. All eight protocol child
slots are consumed. Evidence is in
`target/benchmarks/base083/semantic62-cost4-sysadmin-native-attribution/`.
This supplies no new CPU attribution, isolated performance or native growth
claim; the proposed Bounds cost remains unmeasured.

The large shard-037 expanded diagnostic stream contains 122,040 warnings:
27,162 missing-constraint-label and 94,878 decision-variable-operator. All
122,040 file/range/rule keys are distinct, with zero duplicate excess. These
warnings arise at distinct written source locations; this capture provides no
reason to change grouping or discard warnings. The source ranges remain useful
for suppressions and individual advice. Evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shard037-diagnostic-scaling-root-audit.json`.
The final native diagnostic's 367 distinct mapped original/cost-three/cost-four,
dependency, metadata and frozen binary files were also rehashed unchanged.

The copy-only rank-two `array2d` repair admits only the selected core set-axis
reshape with present Int types and bare top-level sources. Existing initialized
source inspection checks actual axes/source headers, initializers, annotations
and cycles before returning Unknown for unproved reshape cardinality. It grants
no boundedness, output dependency or search coverage. Raw definition fallback
is restricted to that recognized reshape.

The focused search suite first fails at the new symbolic-reshape public case
with unsupported definition safety; after the repair all four groups pass.
The case retains Unknown coverage, and unsafe, cyclic, annotated, optional and
user-overloaded negatives remain Limited. Both full captures, terminal receipts,
equal maps and 106 original/106 formatted-copy/90 actual-source pins were checked.
The complete 18-file candidate is saved in `semantic-candidate.patch`, 154,586
bytes, SHA-256 `8e8eef4ba86b0c12742b6363962bdab7fba7711bea5d0590693e365ab7eae057`,
and passes `git apply --check --whitespace=error`. Evidence is in
`target/benchmarks/base083/semantic62-rank2-array2d-preparation/` and
`semantic62-draft-workspace-rank2-array2d{-red,}-search/`.
Final workspace gates, original-model control, actual integration and fresh
final corpus acceptance remain pending.

Original shards 039–041 have checked terminal captures for another 87 roots.
All 24 receipt-bound streams/hashes, manifest IDs, ordered root/drop records,
final statuses and exact partitions were checked; 1,109 distinct original
input/dependency pins remain unchanged. Shard 039 completes all fourteen thesis
rules, while its expanded preset retains 959 limitations. Shards 040–041 retain
thesis and expanded limitations.

Shard 040's single input error is `grid-puzzle06_dict-55.dzn`: the original fails
UTF-8 decoding at byte 1,510,094, matching the diagnostic. This is outside the
brief's UTF-8 boundary, not a compiler-invalid classification. Its error and
explicit unavailable outcomes remain accounted for. Evidence is in
`target/benchmarks/base083/exhaustive-semantic61/shards039-041-root-audit.json`.
These phase-61 captures do not establish final-candidate acceptance.

The rank-two reshape candidate passes all final copy-workspace gates:
formatting, Clippy with warnings denied, and 193 tests across 42 successful
suites. Workspace tests complete in 43.008 seconds. Full streams/hashes,
successful terminal receipts, equal maps and 106 original/106 copy/90 actual
source pins were checked. The saved 18-file patch remains unchanged. Evidence is
in `target/benchmarks/base083/semantic62-draft-workspace-rank2-array2d-final-{fmt,clippy,workspace}/`.
Original-model validation, actual integration and fresh final corpus acceptance
remain pending.


Release eleven builds successfully from the frozen reshape candidate in 11.860
seconds. Complete Cargo streams, successful terminal receipt, equal maps,
optimized non-test artifact events, frozen binary/origin equality and current
source pins were checked. Frozen linter SHA-256 is
`7f83ddb3c57b5fa07d6cada5bd138d0cbb56f12dd8864940adc1efb994a62227`;
companion SHA-256 is
`66a3bedb751d810c92b79fdab5573dcc76694bcf1d3f29d28a774615f674b9e9`.

Its original routing control completes in 2.220 seconds under the unchanged
300-second deadline, with all 26 outcomes observed, 180 warnings, zero errors
and 206 limitations. Complete source metadata and all rule rows match release
ten; live bytes return to 2,394. The search and unbounded-variable limitations
for the line-167 reshape are removed. All 180 warning blocks and all 206
remaining limitation blocks are byte identical and retain their order. Full
streams, terminal receipt, equal maps and 1,946 current distinct physical pins
were checked. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release11/`,
`semantic62-draft-rank2-array2d-binaries/` and
`semantic62-draft-release11-routing-control/root-block-audit.json`.
This grants no new coverage or boundedness finding; actual integration and
fresh final corpus acceptance remain pending.

The isolated candidate now follows one separately assigned search annotation
through its resolved declaration identity and original RHS file. It requires
a present scalar annotation and retains duplicate-assignment, cycle and
annotation vetoes. The existing coverage walker inspects the body; this adds
no general definition fallback or comprehension support.

The focused regression fails before the repair because the assigned annotation
is opaque, then all four search test groups pass after it. The positive case
marks the searched variable covered and the unrelated variable uncovered;
multiple assignments remain Unknown and Limited. Both complete captures,
terminal receipts and equal source maps were checked. The candidate is saved
in `semantic-candidate.patch`: 19 files, 161,063 bytes, SHA-256
`8fad8abdc459cf950f1376d79bdd7e89a95ac8e2ebb89941d7f35107eb389fa5`.
It passes `git apply --check --whitespace=error`. Evidence is under
`target/benchmarks/base083/semantic62-search-assigned-annotation-preparation/`
and `semantic62-draft-workspace-assigned-annotation-{red,green}-search/`.
Workspace gates, original-model control, integration and fresh final corpus
acceptance for this revision remain pending.

The assigned-annotation candidate passes the required copy-workspace gates:
formatting, Clippy with warnings denied, and all 193 tests across 42 successful
suites. Workspace tests take 43.101 seconds. Complete streams, successful
terminal receipts, equal maps and current 106 original/106 copy/90 actual
source pins were checked. The saved candidate is unchanged. Evidence is in
`target/benchmarks/base083/semantic62-draft-workspace-assigned-annotation-final-{fmt,clippy,workspace}/`.
Original-model validation, integration and fresh final corpus acceptance
remain pending.

The original shard-042 navigation model is accepted by the current MiniZinc
Gecode compile-only check in 1.368 seconds, with status zero and no diagnostics.
The unchanged 3,012,171-byte original produces a 1,203,497-byte FlatZinc file;
no data was invented and no solver ran. Complete streams, terminal receipt,
artifacts and 1,041 current compiler/source/dependency pins were checked.
Evidence is in `target/benchmarks/base083/semantic62-navigation-compile/`.

The original phase-61 thesis native run on that same model timed out after
1,800.067 seconds, with empty streams and no observed completion count. This
is a valid-input processing failure that remains open, not an invalid-model
exclusion. The scheduled thesis companion also times out after 1,800.048 seconds.
Its complete capture reaches the analyze phase but contains no root, drop or
completion row; all fourteen rules remain Unobserved. Full streams and
partitions were checked against the retained terminal receipt. The original
all-preset native run also times out after 1,800.068 seconds, with empty
streams and no observed completion count. Its terminal receipt, streams and
frozen binary were checked. The scheduled all-preset companion remains
pending; shard 042 is not closed. No restart,
deadline increase or formatted-FlatZinc equivalence is claimed.

Release twelve builds from the frozen annotation candidate and completes the
original routing control in 2.347 seconds under the unchanged 300-second cap.
All 26 rule rows are observed, source metadata is unchanged and live bytes
return to 2,394. Complete streams, terminal receipt and equal maps were checked.
The control reports 213 warnings, no errors and 205 limitations. The 180 prior
warning blocks and 205 remaining limitation blocks are unchanged in order;
the opaque assigned-annotation limitation is removed.

Search remains Limited and adds 33 warnings on scalar coordinates contained
in `nXNd_1D`, reshaped into `nXNd` and searched through `array1d(nXNd)`.
Independent code review confirms the transitive coverage gap: the search
walker records the named array, while definition closure only propagates
from dependencies to their target. It never follows the searched reshape
back through its source array to these scalar constituents. The new warnings
are not accepted as correct advice; a source-identity repair remains pending.
Evidence is in
`target/benchmarks/base083/semantic62-draft-release12-routing-control/`.
Integration and fresh final corpus acceptance remain pending.

The isolated candidate now follows searched array initializers through pure
aliases, literals and the supported standard reshape. It checks initializer
safety before following source values; computed cells do not grant coverage
to their inputs. Unproved reshape cardinality remains Unknown in definition
facts, while unsafe axes keep scalar coverage Unknown and search Limited.

The focused regression fails before the repair and all four search groups
pass afterward. The positive case covers both literal scalar constituents
and leaves an unrelated variable uncovered. Complete successful terminal
receipts, streams and current 106 original/106 copy/90 actual source pins
were checked. The full candidate was independently reconstructed and saved
in `semantic-candidate.patch`: 19 files, 168,944 bytes, SHA-256
`26aa1db6d70cbef5fd6144fe74c8624609e3be7b3349de938cc6e548369abfd9`.
It passes `git apply --check --whitespace=error`. Evidence is under
`target/benchmarks/base083/semantic62-search-array-alias-preparation/` and
`semantic62-draft-workspace-array-alias-{red,green}-search/`.
Workspace gates and original routing validation for this revision remain
pending, along with integration and fresh final corpus acceptance.

A release-twelve probe on the original navigation model completes the public
load/bindings/domains/callables/integer-bounds prerequisites in 24.676 seconds
under a 300-second cap. Integer bounds takes 18.301 seconds and retains
939,681 expression rows; callables takes 4.705 seconds. The counting allocator
adds overhead, and this omits definition-aware numeric analysis and other
selected producers. It does not explain or resolve the full-analysis cutoff.

The isolated helper compilation and diagnostic both return zero, with full
streams, reaped children and empty process groups. All 1,074 current physical
pins were rechecked; the only added post-run pin is the compiled helper.
The probe finishes before the first sampler inspection, so no sample is
attempted. Evidence is under
`target/benchmarks/base083/semantic62-navigation-bounds-diagnostic/`.
No whole-analysis acceptance, before/after ratio or cutoff exclusion follows.

The searched-array alias candidate passes the required copy-workspace gates:
formatting, Clippy with warnings denied and all 193 tests across 42 successful
suites. Workspace tests take 43.803 seconds. All three owner handles are
consumed with status zero; children are reaped without timeouts, process
groups are empty and verification errors are absent. Complete streams, equal
maps and current 106 original/106 copy/90 actual source pins were checked.
The saved candidate is unchanged. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-array-alias-final-{fmt,clippy,workspace}/`.
Original routing control, integration and fresh final corpus acceptance
remain pending.

The release-twelve public numeric-facts diagnostic on the original navigation
model completes in 47.690 seconds under the same 300-second cap. Its
instantiations and definitions prerequisites complete; numeric fact
construction takes 36.949 seconds, retaining 939,681 expression rows and
no numeric limitations. This excludes private selected-analysis typed-domain
and local-safety coordination, other producers and rule checking. It does
not explain or resolve the original full-analysis timeout.

The owner returns zero, both compilation and diagnostic children are reaped
without timeouts, and their groups are empty. Complete streams and all 1,074
physical post-run pins were checked; only the compiled helper is added to
the preserved before map. The diagnostic is terminal at the sampler's first
guard, so no sample is attempted or retried. Evidence is under
`target/benchmarks/base083/semantic62-navigation-numeric-diagnostic/`.
A full selected-analysis control with the current candidate remains needed.

Release thirteen builds from the frozen searched-array alias candidate and
completes the original routing all-rule control in 2.233 seconds under the
unchanged 300-second cap. Both owner handles return zero; all 26 rule rows
are observed on the complete resolved original, and live bytes return to
2,394. Build events, frozen binary origins, full control streams, equal maps
and all 1,976 current physical control pins were checked.

The repair removes exactly the 33 incorrect coordinate search warnings and
one search limitation for ordinary unproved division at line 129. The
remaining 180 warnings and 204 limitations are unchanged in order. Search
still reports Limited, with zero findings; every other rule row is unchanged.
Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release13/`,
`semantic62-draft-array-alias-binaries/` and
`semantic62-draft-release13-routing-control/`.
The remaining support gaps, integration and fresh final corpus acceptance
keep base-083 open.

The frozen release-thirteen navigation all-rule companion control times out
at 300.128 seconds. Its full capture reaches analyze but has no root, drop
or completion row. All 26 rules, including the fourteen thesis rules, remain
Unobserved. The child is reaped, its group is empty, the owner returns one
and no verification error is reported. Complete streams, equal source maps
and all 1,388 current physical control pins were checked. No sampler runs.

This current-candidate full-analysis failure remains open despite the shorter
completed public integer-bounds and numeric-facts probes. Those probes omit
selected coordination and other producers, so they do not establish the
remaining cause. The current and original cutoffs have different binaries
and caps; no timing ratio, invalid-input exclusion or fresh corpus acceptance
is claimed. Evidence is under
`target/benchmarks/base083/semantic62-draft-release13-navigation-control/`.

The combined routing producer regression is now retained in the candidate.
Its present standard overload, set-to-array matching and written Set/Bool
type checks pass. It keeps inspected arrays Unknown, with no supported
whole-array output proof, and fails at the public completion check because
inside, not_pref and cost each retain unsupported-definition limitations.
The other three existing search test groups pass. This establishes a focused
failing support case before production changes; it is not a passing repair.

The RED owner returns one and Cargo returns 101, without timeout or surviving
process group. Complete streams, equal maps and current 106 original/106
copy/90 actual source pins were checked. The saved candidate was independently
reconstructed; only its search test differs from the preceding candidate:
19 files, 178,845 bytes, SHA-256
`c02bc7224933c62bbffc6611385bb5aaf0ccf249439b8caeafbbb8b35e59fa99`.
It passes the patch application check. Evidence is under
`target/benchmarks/base083/semantic62-routing336-red-preparation/` and
`semantic62-draft-workspace-routing336-red-search/`.
Implementation, passing gates, integration and final acceptance remain pending.

The frozen release-twelve navigation public producer chain now also completes
callable definitions, optional facts and guarded facts. The diagnostic returns
zero in 63.247 seconds under a 300-second cap; those phases take 12.800,
1.944 and 4.440 seconds, respectively, and numeric facts takes 34.458 seconds.
All ten measured phases complete. Guarded facts retains four limitations.
Complete diagnostic streams, equal preserved maps and all 1,074 current
physical pins were checked. The agent confirms both owned children are reaped
and their process groups are absent. No sampler is attempted.

This public chain excludes private selected-analysis coordination and rule
checking. It does not identify the full-analysis timeout's cause or establish
semantic acceptance. Evidence is under
`target/benchmarks/base083/semantic62-navigation-guarded-diagnostic/`.

The routing inspection implementation is saved as a separate candidate
checkpoint. It adds guarded inspection for the combined test's parameter
tests, conditionals, selectors, Boolean operations, comprehensions, inline
reshapes and Boolean-to-integer coercion. Inspected symbolic values remain
Unknown; the existing strict dependency and output proof paths are retained.
Only callable_definitions.rs changes from the preceding candidate.

The first focused attempt fails to compile because one caller retained the
old helper name. That caller is corrected in this checkpoint; the focused
rerun is still pending. No passing test or completed repair is claimed.
The reconstructed candidate contains 19 files and 209,057 bytes, SHA-256
`a1da06169807944058e59aa9df94ef86be485668cc766e20519f9c816c59de4b`.
It passes `git apply --check --whitespace=error`, and the original source pins
remain unchanged. Evidence is under
`target/benchmarks/base083/semantic62-routing336-inspection-preparation/`.
Passing focused checks, workspace gates, integration and final acceptance
remain pending.

The corrected routing checkpoint compiles, but its focused behavioral rerun
still fails at the same completion assertion: inside, not_pref and cost retain
the three direct-definition limitations. Three other search test groups pass.
The owner returns one and Cargo returns 101 in 22.295 seconds, without timeout;
the child is reaped and its process group is empty. Full streams, equal maps
and all 90 actual, 106 original and 106 copy source pins were checked.
Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing336-green-v2-search/`.
The directory name does not imply a passing result. The saved implementation
is still incomplete; its concrete failing inspection paths need diagnosis
before further repair or workspace gates.

Temporary routing instrumentation identifies the first rejection at
`diam[p] div 2`: initialized-source inspection retains unknown membership,
then the definitionless integer evaluator rejects the array selection.
That rejection propagates through radius into inside, not_pref and cost.
All three definitions are Enforced; no aggregate safety rejection is observed.
The parameter-Boolean guard hypothesis remains masked and unconfirmed.

The diagnostic returns Cargo 101 with three passing search groups and the
same failing completion assertion, in 24.727 seconds without timeout. Full
streams, equal maps, instrumented copy sources, original sources and retained
binaries were checked before restoration. The temporary instrumentation is
retained only under
`target/benchmarks/base083/semantic62-routing336-cause-diagnostic-preparation/`;
the copy is restored to the exact saved candidate. This confirms a local
repair target, not a passing repair. The next change will retain unknown
values for already-inspected parameter integer selections while preserving
invalid-source, selector, zero-divisor and checked-arithmetic handling.

The narrow selection/division change is now saved. It skips only constant
evaluation of a known present parameter-integer array selection whose complete
source inspection returned Unknown. Unsupported operands, sibling inspection,
literal-zero divisors and checked arithmetic retain their existing paths.
No value, membership or output proof is granted. The candidate is 210,513 bytes,
SHA-256 `6f6502ad9ffd63b0ff5ba2268e63c8c58970d4be445cd8d058bcccca5bac4f66`,
and passes the patch application check.

The focused rerun compiles but still has three passing groups and the same
failing completion assertion. Cargo returns 101 in 22.292 seconds without
timeout; the child is reaped and its process group is empty. Full streams,
equal maps and current 90 actual/106 original/106 copy sources were checked.
Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing336-selection-green-search/`.
The next internal rejection is not yet observed. Source inspection finds an
existing parameter-Boolean fallback, so the earlier Boolean hypothesis does
not justify a repair. Passing gates and final acceptance remain pending.

The refreshed routing trace shows that the Unknown-only selection gate does
not fire: `diam[p]` still reaches the definitionless evaluator, radius remains
Unsupported, and the same rejection reaches all three arrays. Its 37 distinct
diagnostic lines match the preceding cause capture. Cargo returns 101 in
24.090 seconds without timeout; the child is reaped and its group is empty.
Full streams, equal maps and all 348 distinct physical pins were checked.
Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing336-cause-diagnostic-v2-search/`.
The saved selection gate has not repaired the observed case. Its operand
inspection result and type need to be observed directly before changing the
gate; no additional production change is accepted from this trace alone.

The operand-state diagnostic confirms that `diam[p]` is a known present
parameter integer and its source inspection returns Supported. The original
Unknown-only gate therefore excluded it even though its numeric value was
unproved. The corrected gate admits either inspected result and still pushes
an unknown operand value; Unsupported remains a veto.

The unchanged focused search test now passes all four groups. The combined
routing case completes while inside, not_pref and cost remain Unknown and
have no supported whole-array output proof; its existing search and negative
checks also pass. Cargo and the owner return zero in 26.671 seconds without
timeout, with a reaped child and empty process group. Full streams, equal maps
and all 349 resolved physical pins were checked. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing336-supported-selection-green-search/`.
The reconstructed 19-file candidate is 210,518 bytes, SHA-256
`55b847d8c125c94f1d4012ce1d83c1cd0340fd3207ff597bb8321105c3d5b308`,
and passes the patch application check. Required workspace gates, original
routing validation, integration and fresh final corpus acceptance remain pending.

The frozen release-twelve navigation public iteration probe completes in
52.496 seconds under a 300-second cap. Iteration facts takes 1.278 seconds,
producing two array facts, five index sets, one iteration and no limitations.
Its required numeric, optional and option-aware guarded prerequisites run
once; callable definitions and unrelated producers are omitted. All ten
measured phases complete. Compilation and diagnostic return zero, their
children are reaped, and the agent confirms both process groups are absent.
Four full streams and all 1,074 current physical pins were checked; only the
compiled helper is added to the preserved before map. No sampler is attempted.
Evidence is under
`target/benchmarks/base083/semantic62-navigation-iteration-diagnostic/`.
The omitted private coordination and rule work still prevent attribution of
the selected-analysis cutoff; no semantic acceptance or timing ratio follows.

The routing candidate passes formatting and Clippy, but its required workspace
test run fails. It reaches 26 suites with 134 passing tests and one failure;
later suites are not run. The existing numeric-facts test expects `flags[5]`
to be Unsupported for an array indexed 1 through 4, but the candidate reports
Unknown. Its subsequent symbolic-index expectation must also remain intact.
The test will not be rebaselined. This candidate is not ready for integration.

Formatting, Clippy and workspace children are reaped without timeout, with
empty process groups. Their return codes are 0, 0 and 101 respectively. Full
streams, equal before/after maps and all 350 resolved physical pins for each
gate were checked. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing336-final-{fmt,clippy,workspace}/`.
The copy remains frozen while the membership regression is diagnosed.

The frozen release-thirteen full navigation diagnostic reaches its 300-second
cap during analysis. The child is reaped, its process group is empty, and all
26 rules remain Unobserved: no root, drop or completion record is emitted.
Full streams, equal maps and all 1,069 immutable physical pins were checked.
Evidence is under
`target/benchmarks/base083/semantic62-draft-release13-navigation-sampled-control/`.
This diagnostic does not provide semantic acceptance or a timing ratio.

One successful one-second sample was taken from the verified analysis child
at an age of 194 seconds. All 84 main-thread samples pass through
`resolve_unused_declarations` and `Producer::body_dependencies` into
`_platform_memmove`. Independent disassembly places the sampled return address
at the vector-removal shift of the remaining 72-byte demand entries. The
current module matches the frozen source. This identifies the removal cost
in that sample window, without establishing its share of total runtime.
The sampler returns zero without timeout, is reaped, and has no remaining
process group. Its complete raw sample and streams were checked.

A private queue per declaration owner is being prepared for review. It will
retain the existing matching-owner FIFO order and recursive append behavior.
No queue repair has been applied or validated yet. The navigation model's
successful current MiniZinc Gecode compilation remains the input-validity
check; the analysis timeout does not make the model invalid.

The original frozen sweep's batches 042 through 046 are now closed. Batch 042
retains four reaped 1,800-second navigation timeouts and no observed root.
Batches 043 through 046 retain 64, 64, 64 and 63 ordered roots respectively,
with matching drop records and explicit completion records in both presets.
Their source states distinguish data, fragments and complete models; status-2
processing errors in 044 and 045 are retained without an invalid-input claim.

All 50 streams were checked physically, the raw companion rows reproduce
every selected-rule outcome partition, and all 1,278 distinct original paths
still match their retained hashes. This closes the capture accounting review
for these batches, not compiler-validity reconciliation or semantic acceptance.
Evidence remains under `target/benchmarks/base083/exhaustive-semantic61/`.

Closed batches 047 and 048 retain one and 25 ordered roots respectively.
Their complete companion streams reproduce all 14/26 rule partitions and
matching drop records; 16 stream hashes and 1,048 distinct original paths were
checked. Batch 047's complete resolved planner emits 66,106 thesis warnings
with no thesis limitations; all emits 67,563 warnings and 959 limitations.
Batch 048 distinguishes 24 data roots from its one complete model. These
frozen-sweep captures remain separate from final-candidate acceptance.

The existing numeric selection regression now also checks a composed Boolean
operand, `1*(not symbolic[1])`, with the exact core `not` signature available.
The test-only run fails first on this new case: it reports Unknown rather than
the required Unsupported membership result. Three other numeric groups pass.
Cargo returns 101 without timeout in 1.319 seconds; its child is reaped and
its process group is empty. Both complete streams, equal maps and all 353
resolved physical pins were checked. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-boolean-membership-red-numeric_facts/`.

The reconstructed 19-file candidate is 210,642 bytes, SHA-256
`2787c1c39d4957c51a5b5086d96fd9be67ee76837b06c0049e4a80a8dc871389`,
and passes the patch application check. The reviewed repair will put both
negative membership guards on the selection itself after complete child
inspection, covering nested Boolean operations while leaving initialized
routing arrays Unknown. It has not yet been applied or validated.

The reviewed membership repair is now applied in the copy. Complete child
inspection precedes a negative-only closed-index-domain check and the existing
strict membership check for original uninitialized Boolean arrays. Admission
still returns Unknown; initialized routing arrays bypass the original-array
restriction. Multiplication inspection is unchanged.

All four numeric groups pass, including the direct out-of-range, unproved and
nested unproved selection assertions. Cargo returns zero without timeout in
3.036 seconds; its child is reaped and its process group is empty. Both full
streams, equal maps and all 355 physical paths were checked. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-boolean-membership-green-numeric_facts/`.
The reconstructed 19-file candidate is 212,769 bytes, SHA-256
`f25f42a7cc05a3eaadc325b897e6f1539f8918abd07c16da578399310db74a36`,
and passes the patch application check. Routing and required workspace gates
are still pending; the candidate has not been integrated.

The repaired candidate now passes routing and every required copy gate.
The four search groups, including combined routing, pass in 18.911 seconds.
Formatting and Clippy pass in 1.036 and 2.645 seconds. Workspace tests pass
all 193 tests across 42 suites in 41.783 seconds, with no failures or ignored
tests. All children are reaped without timeout, their process groups are empty,
and their group-verification errors are null. Complete streams and matching
before/after maps for all five focused/gate captures were checked, followed by
physical rechecking of their 355 shared paths.

Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-boolean-membership-green-{search,fmt,clippy,workspace}/`.
The saved candidate hash remains `f25f42a7cc05a3eaadc325b897e6f1539f8918abd07c16da578399310db74a36`.
Original-model controls, queue measurements, integration and fresh complete
final-corpus acceptance remain pending; this is not task completion.

Release fourteen is built from that unchanged, passing copy. Cargo returns
zero in 12.909 seconds without timeout; its child is reaped and its process
group is empty. Both streams, equal maps, all 16 Cargo events and 408 distinct
physical paths were checked. Native and probe snapshots match their optimized,
non-test Cargo origins. The legitimate unoptimized custom-build event is
separate from the selected libraries and consumers.

The native snapshot is 2,941,904 bytes, SHA-256
`936daaf195f49cd33d154176675748593d73d425f1bad3dc4c892f92adcfe609`;
the probe is 2,680,528 bytes, SHA-256
`a800d48abe8edfe2370d51a98a39fca7be5ce4fdf7968882a7c678361adbfc81`.
The 22 saved library files match their Cargo origins, total 28,185,465 bytes,
and retain the exact 106-file copy source map and compiler identity. They form
the immediate-before queue baseline; no queue change is included.

Evidence is under `target/benchmarks/base083/semantic62-draft-workspace-semantic-release14/`,
`semantic62-draft-boolean-membership-binaries/` and
`semantic62-unused-owner-queue-before-libraries/`. This build does not establish
original-model support, performance improvement or final corpus acceptance.

The private unused-declaration demand store now keeps a FIFO queue per owner.
It preserves each owner's original append order and the unchanged owner
scheduling, body deduplication, recursion and uncertainty handling. The queue
lookup is repeated after each popped demand so recursive appends are consumed.
Only this module changes from the release-fourteen baseline; the other 105
copy files and all actual/original sources remain unchanged.

The four existing unused-declaration groups pass in 4.036 seconds. Formatting,
Clippy and all 193 workspace tests across 42 suites pass; workspace takes
42.904 seconds. Every capture is reaped without timeout, has an empty process
group and null group-verification error, and retains equal before/after maps.
Full stream hashes, the complete passing test-name set and all 387 shared
physical paths were checked. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-unused-owner-queue-{unused_declarations,fmt,clippy,workspace}/`.

The reconstructed 20-file candidate is 216,139 bytes, SHA-256
`591673aaf53c4a4bfb6b5d7842bdc782f8ecc522c012590036925494037bdb9f`,
and passes the patch application check. Matched public facts, allocations,
native behavior and growth measurements remain pending. Passing tests do not
establish the queue's performance benefit or final corpus acceptance.

Release fifteen is built from the passing queue candidate. Cargo completes in
13.874 seconds without timeout; its child is reaped and its process group is
empty. The full 16-event Cargo stream, equal source maps, optimized native and
probe origins, and both frozen library sets were checked. The audit covers
439 physical paths including the 22 before-library snapshots. Across the 106
copy sources, only `unused_declarations.rs` differs from release fourteen;
only the lint library and metadata differ among the 22 library files. The
compiler identity is unchanged. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release15/`,
`semantic62-draft-unused-owner-queue-binaries/` and
`semantic62-unused-owner-queue-after-libraries/`. This establishes a matched
build pair, not facts parity or performance improvement.

The original routing model also passes current Gecode FlatZinc compilation
without data or solving in 0.354 seconds. All 1,041 unchanged input paths,
empty streams and the resulting FlatZinc/output artifacts were checked.
Evidence is under `target/benchmarks/base083/semantic62-routing-compile/`.
This strengthens the older model-check-only evidence; it does not establish
Zincite support or formatting equivalence. The frozen release-fourteen
routing control and matched public queue comparison are now authorized;
results and final corpus acceptance remain pending.

Original sweep batches 049 and 050 are closed. Their 24 complete stream hashes,
ordered roots and drop records, selected 14/26-rule rows, every reported
outcome partition and 1,057 physical original hashes were checked. Batch 049
contains the generated 7-cube model: its companion completes every thesis and
all rule, with 110,241 and 141,768 warnings respectively and no limitations.
Native thesis/all processes also finish, in 33.880 and 409.447 seconds; native
commands do not report per-root completion. Batch 050 accounts for 31 standalone
data files and three complete resolved models. Its thesis companion reports
45 warnings and two limitations; all reports 277 warnings and 103 limitations.
Standalone data remains distinct from model analysis. These are original frozen
phase-61 observations, not fresh final-candidate or compiler-validity acceptance.
Evidence is under `target/benchmarks/base083/exhaustive-semantic61/shard-{049,050}/`.

The frozen release-fourteen original routing control finishes in 162.796
seconds at the unchanged 300-second cap. Its companion is reaped with status
one, one complete resolved root, all 26 ordered rule rows, matching drop
baseline and no Unobserved root. All 1,684 physical paths, matching before/after
maps and complete streams were checked. Compared with release thirteen, every
complete multiline warning/limitation block preserves order: exactly six
limitation blocks disappear, with none added. Warnings remain 180; limitations
fall from 204 to 198. `suspicious-domain` changes from Limited to Completed;
all other rule outcomes and finding counts remain equal. Five removed blocks
belong to search coverage or unbounded-variable analysis, which still remain
Limited overall. The valid original therefore still needs concrete support
repairs. This is companion evidence with explicit sweep overlap, not a native
comparison, isolated speed measurement or final acceptance. Evidence is under
`target/benchmarks/base083/semantic62-draft-release14-routing-control/`.

Original sweep batches 051–053 are closed and checked: 36 complete stream
hashes, every ordered root/drop and selected-rule partition, and 1,109 physical
original hashes. All companion roots are observed and all reported drops return
to baseline. Batch 051's generated model completes every thesis rule with
66,109 warnings and no limitations; all reports 67,571 warnings and 961
limitations. Batch 052 accounts for 60 data files, three complete resolved
models and one input error in `2017/crosswords/grid-05.04_dict-80.dzn`.
That processing error is not a compiler-invalid classification. Thesis reports
3,215 warnings/37 limitations; all reports 7,860 warnings/1,472 limitations.
Batch 053 accounts for 20 data files and two complete resolved models, with
36 warnings/two limitations for thesis and 234 warnings/83 limitations for all.
No process in these captures times out. These remain frozen phase-61 evidence;
final-candidate acceptance is open. Evidence is under
`target/benchmarks/base083/exhaustive-semantic61/shard-{051,052,053}/`.

The public queue comparison stopped twice during setup, before creating its
output directory or launching a compiler/model child. The helper now handles
the two retained compiler-proof stream forms and the older hash/byte-only
records explicitly. Every field in those records remains checked; new maps
capture current modes. The complete child-free setup and all 1,108 physical
pins were checked before authorizing the first actual measurement. These
setup failures are retained separately and are not benchmark cutoffs.

The matched public queue comparison completes both builds and all four runs.
Every child is reaped without timeout, its process group is empty and its
verification error is null. The 1,108 input pins match before/after and current
files; both compiled helpers and four complete ordered facts files were also
checked. Facts are byte-identical per original: Sysadmin 2,913,964 bytes and
Navigation 2,913,992 bytes. Each has 4,658 declaration rows, complete root
state and no usage limitation; every run drops back to its own baseline.
Load, bindings and callable allocation counters are identical across builds.

Observed unused-declaration phase times are 7,225.214→1,028.335 ms for Sysadmin
and 223,491.703→6,019.491 ms for Navigation. Requested allocation bytes fall
59,535,284→55,485,616 and 279,522,605→262,902,569 respectively; phase peak
increments fall 23,321,906→21,341,690 and 111,993,318→103,727,918 bytes.
Allocation calls rise by 422 and 425; retained usage bytes are unchanged.
These are two real source-size controls with counter overhead and explicit
other-work overlap, not an isolated ratio or a universal growth claim. They
establish unchanged public facts and lower observed queue cost, while full
selected native outcomes and final corpus acceptance remain separate.
Evidence is under
`target/benchmarks/base083/semantic62-unused-owner-queue-public-pair/`.

The frozen release-fifteen full all-rule Navigation companion still times out
at the unchanged 300-second cap (actual 300.088 seconds). Its child is reaped,
its process group is empty and its group-verification error is null. Both
complete streams, equal maps and all 1,104 physical runtime paths were checked.
Only manifest/load/analyze begin records are emitted: no root, drop or complete
record. The original remains Unobserved for all 26 rules and the 14 thesis IDs;
semantic acceptance remains null. Public usage-facts parity and the improved
queue cost do not establish full selected-analysis completion. A first native
control with bounded attribution is being prepared to locate the remaining
cost; the companion is not retried. Evidence is under
`target/benchmarks/base083/semantic62-draft-release15-navigation-control/`.

The first frozen-fifteen native all-rule Navigation control also times out at
300 seconds (actual 300.088). Its child is reaped, its process group is empty
and its verification error is null. Both streams are empty; their hashes and
all 1,119 unchanged physical runtime paths were checked. Native root counts,
rule outcomes, drops and parity remain null; no completion is inferred.
The sole stack-sampling opportunity was skipped because current process
identity inspection was denied before utility launch. No request was reserved,
no sample was taken and no identity retry or escalation occurred. Thus no
remaining-cost attribution is available from this control. A single public
shared-analysis pipeline diagnostic is being prepared instead; final corpus
acceptance remains open. Evidence is under
`target/benchmarks/base083/semantic62-draft-release15-navigation-native-control/`.

The next Routing repair is saved separately in
`scripts/semantic-routing-draft.patch`. It applies on top of the current
release-fifteen candidate copy, after `scripts/semantic-candidate.patch`;
it is not integrated into the production sources. It extends inspected
rank-three Boolean reshapes and DecisionInt array-bound reflection while
retaining Unknown values and output coverage. Static review found that formal
array defaults could supply a false nonempty proof; the revised draft restricts
literal-shape evidence to bare top-level Value declarations and includes an
overridden-default negative case. The complete draft was read and its exact
16,616 bytes match SHA-256
`ec0a4bad29e5ac5a40cd2578835553acd241119748a7786b7c97ed3ac8ea09c9`.
No draft changes have been applied to the candidate copy and no draft tests
have run. Focused RED/GREEN validation and final acceptance remain pending.

The frozen release-fifteen public shared-analysis diagnostic completes all
ten ordered stages in 63.066 seconds without timeout. Both compiler and
producer children are reaped, their groups are empty and verification errors
are null. All 1,084 final physical paths and both pairs of complete streams
were checked. Its allocation baseline and after-drop observation both equal
2,301 bytes. Numeric facts account for 34.192 seconds and callable definitions
14.556 seconds in this instrumented run, which overlaps focused test work.
The public chain omits private typed-domain preparation, definition merging,
independent producers and rule checks. It therefore does not explain the
full-rule 300-second cutoff or establish full-rule completion, a matched
performance ratio or semantic acceptance. Evidence is under
`target/benchmarks/base083/semantic62-navigation-release15-guarded-diagnostic/`.

The Routing rank-three Boolean reshape and DecisionInt array-reflection
repair is now included in `scripts/semantic-candidate.patch`; the separate
unapplied draft is superseded and retained in commit `f5bf00e`. The candidate
remains in the disposable workspace, with actual production sources frozen.
The focused search case passes, and all six guarded groups pass, including
nonempty literal reflection with Unknown definedness/no numeric value, empty
top-level reflection Refuted, and no nonempty proof from an overridable
formal default. A first guarded run exposed a missing integer-range declaration
in the miniature standard-library fixture. Adding the concrete instantiated
range signature fixed that fixture without changing production or expectations;
the failed capture is retained. Formatting, clippy and all 193 workspace tests
in 42 suite results pass. Complete streams, receipts and unchanged current
maps were checked; every child is reaped without timeout and has an empty
group/null verification error. The combined 20-file candidate is 228,702 bytes,
SHA-256 `74a9dea0631996260c9be3dd6d585f3e61282958d3d86d7f834f8e4430ada97b`,
and applies cleanly. Original Routing outcomes and final corpus acceptance
remain to be checked with this candidate. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing-family1-green-v2-{guarded,fmt,clippy,workspace}/`.

The frozen-fifteen expanded public diagnostic also completes, in 68.354
seconds. It appends iteration, comprehension structure, global patterns,
generator binding usage and callable input facts in their selected producer
order. All fifteen stage begins/completions match; both children are reaped
without timeout, groups are empty and verification errors are null. Complete
streams and all 1,084 final physical paths were checked. Drop returns from
the retained facts to its own 2,302-byte baseline. The added iteration phase
takes 1.163 seconds, and each later added phase takes less than one second
in this instrumented run with possible copy-check overlap. These public
producers do not explain the full-rule cutoff. Private domain preparation,
coordination and rule checks remain unmeasured; no matched ratio or semantic
acceptance follows. Evidence is under
`target/benchmarks/base083/semantic62-navigation-release15-expanded-diagnostic/`.

The validated family1 candidate builds as optimized release sixteen. The
Cargo capture completes in 14.185 seconds with unchanged source maps;
its selected binaries and 22 libraries match their frozen copies. Complete
streams and artifact records were checked, including thirteen optimized
non-test artifacts and one separate unoptimized custom build. Only the lint
rlib/rmeta differ from frozen fifteen. The new library set totals 28,210,500
bytes. Build success does not establish original-model outcomes or corpus
acceptance. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release16/`
and `semantic62-draft-routing-family1-binaries/`.

The original Routing release-sixteen all-rule control completes in 163.573
seconds at the unchanged 300-second cap. It has one complete resolved root,
26 ordered rule rows and a matching drop (2,394 bytes before/after). Complete
streams and current before/after maps were checked; the child is reaped, its
group is empty and verification error is null. All 180 ordered warning blocks,
source metadata and the 23-file loaded closure match release fourteen.
Limitations fall from 198 to 196: only the two rank-three search limitations
at lines 274 and 291 disappear; all other full blocks retain their order and
bytes, with none added. Rule rows remain twenty Completed/six Limited,
including twelve Completed/two Limited thesis rules. Four search limitations
remain at 373, 182, 235 and 378, plus unbounded-variable at 373. This confirms
the targeted repair on the compiler-accepted original, while remaining valid
processing gaps keep acceptance open. The run overlaps the protected corpus;
it supplies no isolated timing ratio or fresh native outcome. Evidence is under
`target/benchmarks/base083/semantic62-draft-release16-routing-control/`.

A separate frozen-fifteen diagnostic copy adds only 105 outer phase markers
to `analysis.rs`; removing them restores its exact baseline bytes. All 106
source files were checked against the frozen-fifteen map, and the actual
workspace/current sixteen candidate remain unchanged. Its locked/offline
probe-only optimized build completes in 17.440 seconds. Complete streams,
current maps, selected probe origin/frozen copy, 22 libraries and rustc were
checked. Separate build paths change library bytes; no binary parity or stock
performance claim follows. The probe is ready for one selected all-rule
Navigation trace at the existing cap; no model run has occurred yet. Evidence
is under `target/benchmarks/base083/semantic62-navigation-release15-coordination-build/`.

The single frozen-fifteen Navigation marker control reaches its unchanged
300-second deadline (300.130 seconds). The child is reaped with return -9,
its group is empty and verification error is null. Complete streams and
current before/after maps were checked. All 78 observed markers parse in
order; the pending scopes are `analyze_model` and `index_set_mismatch_check`.
Every preceding marked producer/check ends. This locates the unfinished
stage, without attributing CPU cost. The stdout has only its manifest and
load/analyze beginnings: all 26 rules and the fourteen thesis rules remain
Unobserved, with no root/drop/completion or semantic acceptance. Raw stderr
and its separate marker stream match; ordinary diagnostic stderr is empty.
Source inspection confirms repeated full expression/obligation scans per
array access. The retained local-lookup repair needs adaptation and fresh
validation against the current candidate. No retry or sampler was run.
Evidence is under
`target/benchmarks/base083/semantic62-navigation-release15-coordination-control/`.

The retained index-set lookup repair is adapted to the current candidate in
`scripts/semantic-index-lookups-draft.patch` (3,392 bytes, SHA-256
`9232b055f46d212a5530a9675b203a752223d75e3dc556ef415e57ed76e7a462`).
Independent static review confirms first-expression selection, ordered and
duplicate obligations, suppressions, full-axis handling and diagnostic order
are preserved. It applies cleanly to the copy and remains unapplied. Focused
baseline/after checks, workspace gates and current-model measurements remain
required; the draft alone establishes no processing or performance improvement.

The lookup refactor now passes the three existing index-set public tests both
before and after its exact copy application; their complete stdout bytes match.
Formatting, Clippy with warnings denied and all 193 workspace tests in 42 suite
results pass. Complete streams, terminal receipts and current before/after maps
were checked; all children are reaped without timeout, with empty groups and
null verification errors. Only the copy's index-set checker changed. The saved
combined 20-file candidate includes the refactor and applies cleanly; its
superseded standalone draft remains in commit `49655b0`. Production integration
and current-model performance/outcome verification remain open. Evidence is
under `target/benchmarks/base083/semantic62-draft-workspace-index-lookups-{baseline,after}-*/`.

The Routing line-373 Cartesian-sum draft is independently reviewed and saved
unapplied in `scripts/semantic-routing373-draft.patch`. It reuses ordered
collection/reshape inspection, adds exact selected Float comparison/selection
and closed nonnegative integer-power inspection, and grants only Unknown
facts. Its public extension checks the actual Unknown target and retained
Unsupported partial child. The 407-byte reduction compiles with current
MiniZinc/Gecode without solving (0.194 seconds); full streams, receipt, all
1,041 unchanged inputs and generated FlatZinc/output-specification files were
checked. Tests-only failure reproduction and post-repair Rust gates remain
pending. Evidence is under
`target/benchmarks/base083/semantic62-routing373-preparation/compiler-precheck/`.

The validated lookup candidate builds as optimized release seventeen in
12.971 seconds. Complete Cargo streams, current before/after source/helper
maps, thirteen optimized non-test artifacts, both binary origins/frozen copies,
22 library origins/frozen copies and rustc were checked. All children are
reaped without timeout, groups are empty and verification errors are null.
Only the lint rlib/rmeta differ from release sixteen; frozen libraries total
28,210,941 bytes. The original sources remain unchanged. These fixed artifacts
permit current-model checks while the separate Routing regression proceeds;
build success alone establishes no model completion or performance claim.
Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release17/`
and `semantic62-draft-index-lookups-binaries/`.

The Routing Cartesian-sum regression fails before the production repair at
its real Unknown-definition assertion. The first repaired run exposed a
missing `diff` declaration in the miniature standard-library fixture; adding
only its present Int-set specialization fixes that prerequisite without
changing production or expectations. Both failed captures are retained.
The corrected four-group search run passes, including the Unknown target,
absence of Supported WholeArray/coverage proof and partial-child Unsupported
assertions. Formatting, Clippy and all 193 workspace tests in 42 suite results
pass. Complete streams, receipts and current before/after maps were checked;
all children are reaped without timeout, with empty groups and null errors.
The saved 20-file candidate is 240,771 bytes, SHA-256
`3b412bd90b0e23d716fb2474213be8e8dfc4faa08ed032ac3b499f662b0dfe1c`,
and applies cleanly. The superseded draft remains in commit `13044d1`.
Original Routing recapture, production integration and final corpus acceptance
remain open. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-routing373-{red,green,green-v2}-*/`.

The frozen release-seventeen Navigation companion still reaches its unchanged
300-second deadline (300.075 seconds), with zero observed roots. The complete
raw streams contain only the manifest and load/analyze beginnings; all 26
selected rules and fourteen thesis rules remain Unobserved, with no drop or
completion record and null semantic acceptance. The child is reaped, its group
is empty and the verification error is null. Before/after and all 1,149 current
input, frozen-binary and helper pins match. The lookup repair remains validated
for correctness, but this control does not establish Navigation completion or
an isolated speedup. No retry or sampler was run. Evidence and the independent
root audit are under
`target/benchmarks/base083/semantic62-draft-release17-navigation-control/`.

The validated Routing Cartesian-sum candidate builds as optimized release
eighteen in 12.252 seconds. Complete Cargo streams, all current before/after
source and helper pins, thirteen optimized non-test artifacts, both binary
origins and frozen copies, 22 library origins and frozen copies, and rustc
were checked. The child is reaped without timeout, its group is empty and
the verification error is null. Only the lint rlib/rmeta differ from release
seventeen; frozen libraries total 28,229,877 bytes. Original sources remain
unchanged. Original Routing outcomes, production integration and final corpus
acceptance remain open. Evidence and the independent root audit are under
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release18/`,
with artifacts in `semantic62-draft-routing373-binaries/` and
`semantic62-routing373-libraries/`.

The separate release-seventeen Navigation diagnostic build passes in 16.806
seconds. All 106 baseline files match the frozen candidate; its only source
change is the previously reviewed 105 outer phase markers in analysis.rs.
Removing those lines restores every original byte. Complete Cargo streams,
current before/after maps, twelve optimized non-test artifacts, the probe
origin and frozen copy, 22 library origins and frozen copies, and rustc were
checked. The child is reaped without timeout, its group is empty and the
verification error is null. Original and shared candidate sources remain
unchanged. This diagnostic build establishes no runtime attribution or
semantic acceptance; its markers are excluded from the production candidate.
Evidence and the independent root audit are under
`target/benchmarks/base083/semantic62-navigation-release17-coordination-build/`.

The original Routing model completes the frozen release-eighteen all26
companion in 166.842 seconds without timeout. Complete raw streams, all 1,728
current input/binary/helper pins, exact rule rows, unchanged 23-file/383,407-byte
source metadata and restored 2,394-byte drop baseline were checked. All 180
warning blocks remain byte-identical and ordered. Limitations fall from 196 to
194: only the line-373 search and unbounded-variable limitations disappear; no
new limitation is added and all retained blocks preserve their bytes and order.
Unbounded-variable changes from Limited to Completed. All has 21 Completed and
5 Limited rules; the thesis subset has 13 Completed and one Limited rule.
Search coverage still reports lines 182, 235 and 378. The child is reaped,
its group is empty and the verification error is null. This companion supplies
no fresh native performance ratio or whole-corpus acceptance. Evidence and the
independent root audit are under
`target/benchmarks/base083/semantic62-draft-release18-routing-control/`.

Independent closed-shard 054–058 review reconciles 235 observed roots per
selection and one unobserved root. Shard 054 has four nested 1,800-second
cutoffs despite a terminal outer process; no root/drop/completion rows exist.
Shards 055–058 preserve complete records, matching native/companion diagnostic
streams and statuses, and restored drop baselines. Thesis totals are 956
warnings, one error and 592 limitations; all totals are 6,824 warnings, one
error and 1,428 limitations. A data parse error retains NotRun for all 14/26
rules; compiler-invalid exclusion remains unestablished pending its current
MiniZinc model/data precheck. The reviewer checked 100 closed artifacts; the
root separately reconciled 35 receipts, their 70 full streams, rule partitions
and all 1,263 physical originals. Five transitive imports omitted from local
report maps are present in shared before pins and match now. Inner receipts
lack separate group-verification fields; outer reaping/group evidence is
retained. These frozen semantic61 results do not establish candidate acceptance.
Evidence is in `target/benchmarks/base083/exhaustive-semantic61/` under
`shards054-058-readonly-audit.{md,json}` and `shards054-058-root-check.json`.

The fresh frozen-seventeen Navigation marker companion reaches the unchanged
300-second deadline (300.123 seconds). All 84 markers parse in order. The
index-set mismatch check ends, followed by callable-input and precondition
checks; the pending scopes are analyze_model and partial_expression_check.
This locates unfinished work without attributing CPU time to a particular
scan. Complete raw streams, marker separation and all 1,213 current pins were
checked. There is no ordinary diagnostic stderr or root/drop/completion row;
all26 and thesis14 remain Unobserved with null semantic acceptance. The child
is reaped, its group is empty and the verification error is null. No retry or
sampler was run. The next existing partial-expression checker still scans all
guarded expressions and obligations per operation; a local lookup adaptation
needs review and before/after validation. Evidence and the independent root
audit are under
`target/benchmarks/base083/semantic62-navigation-release17-coordination-control/`.

The shard-056 data parse error is now checked against current MiniZinc. One
Gecode compile-only invocation of the exact original model/data pair rejects
`time-instance-20-100.dzn` at 88.5 with a syntax error requiring the array's
closing delimiter. The compiler exits 1 in 0.112 seconds without timeout or
solving, and produces no FlatZinc/output-specification artifacts. Full streams,
the reaped child with an empty group and null verification error, and all 1,042
unchanged original/compiler/standard-library/helper pins were checked. This
establishes compiler-invalid input rather than a missing Zincite requirement;
the observed NotRun rows remain in corpus accounting. Evidence and the root
audit are under
`target/benchmarks/base083/semantic61-shard056-time-data-precheck/`.

The partial-expression lookup draft is saved unapplied in
`scripts/semantic-partial-expression-lookups-draft.patch` (4,878 bytes, SHA-256
`23ac0827e2a579eaf4628e775573a9ca4020f1bf8b9ddba191ee870a03dd6a5e`).
Independent static review found no substantive defect: first-expression lookup,
obligation order and duplicates, default checks and diagnostic behavior are
preserved. Existing focused before/after tests, formatting, Clippy, workspace
tests and finite performance validation remain pending. Actual and shared
Rust sources are unchanged; this checkpoint does not establish task completion.

The Routing lines 182/235 conditional draft and its public regression changes
are saved unapplied in `scripts/semantic-routing-conditionals-draft.patch`
(13,059 bytes, SHA-256
`8ff290eba1d4daa95d7d32368480495b6e9f2502cbc1fe2a28d30a5ba708842c`). The draft inspects complete unproved conditionals before
any branch contributes definitions, and narrowly inspects scalar core lb/ub
array selections without proving their bounds. Root has read the complete
formatted patch; independent static review is ongoing. Current MiniZinc
compile-only precheck, regression RED/GREEN and required Cargo gates remain
pending. Actual and shared Rust sources are unchanged, and the validated
combined candidate is unchanged. This saves work without claiming acceptance.

The conditional draft's current MiniZinc prerequisite now passes. The fixed
534-byte reduction compiles once with Gecode to a 1,229-byte FlatZinc file and
1,200-byte output specification, without solving. Compiler exit is zero in
0.179 seconds, with empty complete stdout/stderr, no timeout, a reaped child,
an empty process group and null verification error. All 1,041 input/compiler/
standard-library/helper pins match before, after and the root audit. Evidence
is under `target/benchmarks/base083/semantic62-routing-user-test-conditionals-preparation/compiler-precheck/`.
This establishes a syntax/type prerequisite only; independent semantic review,
Zincite regression RED/GREEN and Cargo gates remain pending.

The partial-expression lookup baseline passes all seven existing public cases
(four partial-expression and three input-precondition cases) in one bounded
Cargo invocation. Exit is zero in 0.526 seconds without timeout; complete
stdout/stderr, before/after/current state, child reaping and empty process
group were independently checked. No source was changed during the capture.
Evidence and the root audit are under
`target/benchmarks/base083/semantic62-draft-workspace-partial-lookups-baseline-partial_expression/`.
The exact reviewed patch is authorized for the disposable candidate copy;
after tests and required workspace gates remain pending.

Independent review of the conditional draft found one scope regression, which
the root confirmed against the existing strict reflection path: the new array
helper intercepted bare DecisionInt lb calls already admitted under a returning
assert(has_bounds(...)). The scratch correction must decline non-array-selection
operands before the array-specific veto. No other substantive defect was found
in the complete draft review. The saved conditional patch remains an unapplied
checkpoint pending that correction and validation.

The saved conditional draft now includes the narrow scalar-scope correction.
`scripts/semantic-routing-conditionals-draft.patch` is 13,094 bytes, SHA-256
`77b2f80535a97d88137589eb5a06421d039cf573ea81df940fe93663114b1628`. Root checked the complete
1,220-byte source delta against the prior reviewed scratch file, reconstructed
the production patch from its baseline, and verified unchanged public tests.
Non-array-selection operands now return to the existing strict reflection path;
the array-selection checks retain their previous inspection-only behavior.
The draft remains unapplied, with Zincite regression and workspace gates pending.

The independently reviewed partial-expression lookup change is now included
in the saved combined candidate. Focused before/after tests pass the same seven
public cases. After a layout-only rustfmt correction, fmt, Clippy with warnings
denied and all 193 workspace cases across 42 result groups pass. Root checked
complete streams, exact commands, before/after/current state, reaping and empty
process groups for every gate. No source/helper/patch change occurred during
the final gate captures. Actual Rust remains frozen while original89518 runs.

`scripts/semantic-candidate.patch` now reconstructs 21 changed files exactly
from the original workspace and candidate copy: 245,823 bytes, SHA-256
`37dad180498e48f247a87f429071d676b70a69c806a4e8f9cb5eb61aebb5a89d`.
Only partial_expression.rs changes relative to the prior combined candidate;
its formatted source is 16,094 bytes, SHA-256
`75314e42bc0196783bcaede7f55f7f2d1dc9483cd7cf9287b5bddfd9cf215576`.
The superseded standalone draft remains in commit beeda66. Detailed focused
and final gate evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-partial-lookups-after-*`
and `semantic62-draft-workspace-partial-lookups-after-v2-*`.

Independent static review also confirms the exact saved conditional correction
preserves the bare-scalar strict path, with no other substantive finding. That
draft is still unapplied. The existing finite Routing case is suitable for a
matched partial-lookup comparison; retained constraint-true growth cases can
check general traversal overhead but do not exercise growing operation lookups.
Targeted growth and successful CPU/allocation attribution remain missing, as do
fresh complete final-candidate corpus/native evidence and task verification.

The optimized partial-expression successor (release19) builds successfully in
13.150 seconds with no timeout. Root checked all complete Cargo streams,
16 events, 14 artifacts and 13 optimized non-test artifacts, plus unchanged
before/after/current input state. Both native and allocator-probe executables
are frozen independently of mutable Cargo origins. The native is 2,964,464
bytes, SHA-256
`9d2ff843da47b2086374cba213484795d35954e96f7c87c4cac14546f2885996`;
the probe is 2,703,712 bytes, SHA-256
`8ec42c080694cfa07f3c2399db8c85df0fd3fb6d75ba7cc55742451ec6ea4ee5`.
All 22 optimized libraries (28,255,154 bytes), their Cargo-reported origins and
compiler pin were checked. Only the lint rlib/rmeta differ from release18.
The child is reaped, its group empty and the verification error null. Build
evidence and the root audit are under
`target/benchmarks/base083/semantic62-draft-workspace-semantic-release19/`;
frozen artifacts are in `semantic62-draft-partial-lookups-binaries/` and
`semantic62-partial-lookups-libraries/`. This is build evidence; model parity,
performance and final corpus/task acceptance remain pending.

The frozen release18 Navigation native attribution control reached its unchanged
300-second cap (300.074 seconds). Root checked all 1,041 original/compiler/standard
inputs, two frozen binaries and 45 helpers against equal before/after/current
state, plus full streams, reaping and empty groups. The one own-child sampling
attempt at 240.122 seconds returned 255 in 0.038 seconds; its stderr reports
that sample could not examine the process, and no graph was produced. No retry
or privilege escalation was attempted. Native stdout/stderr are empty, and
root counts, per-rule outcomes, allocation attribution and semantic acceptance
remain null. Evidence and terminal root audit are in
`target/benchmarks/base083/semantic62-stock18-navigation-native-attribution/`.
This control provides neither complete Navigation processing nor CPU attribution.
The separate matched Routing18/19 comparison remains in progress.

The independently reviewed Routing378 Float-let draft is saved, unapplied, as
`scripts/semantic-routing-float-let-draft.patch`: 29,265 bytes, SHA-256
`50b4b474a2da0b5ff71a57ce90fd5eaaf3cba0c73af4e2acc7d5a254a9a68b39`.
It inspects the required present parameter Float operations and ordered local
sources while retaining Unknown and discarding temporary outputs/inspected IDs.
Root independently confirmed two review findings, and the revised draft peels
named set domains for known emptiness and inspects unused local initializers'
transitive sources even when strict dependency lookup succeeds. A focused
negative exercises the public direct-RHS path through a named zero-dividing
source. Independent delta review found no further reachable defect; root
reconstructed both complete diffs and checked all current artifact/source pins.
Standalone formatting is the only validation so far. Compiler precheck, public
RED/GREEN, workspace gates and original Routing outcomes remain pending.
The shared conditional preflight overlaps the saved182/235 draft and must be
integrated once. Shared candidate and actual Rust remain unchanged.

The two operation-rich growth models (100/1,000 written declarations) pass
current Gecode compile-only prechecks in 0.194/0.182 seconds, with no solver
run. Both child statuses are zero, full stdout/stderr empty, children reaped
and groups empty with null verification errors. Root checked all 1,044 input
pins (including 1,035 standard files) against equal before/after/current state
and all four emitted FZN/OZN artifacts. Evidence is under
`target/benchmarks/base083/semantic62-partial-operation-growth-preparation/compiler-precheck/`.
The prepared source family grows initialized-array accesses, div/mod operations
and scalar defaults tenfold. This establishes compiler acceptance only; actual
selected semantic facts, full matched outcomes and allocation/native growth
measurements remain pending.

The four-case matched Routing18/19 control completes with exact full source,
all26 analysis rows (including thesis14), native/companion status and raw
diagnostic bytes, and drop-record parity. Root rechecked every original/helper/
frozen-binary pin, all four complete streams, reaping/empty-group/null-error
receipts and equal per-child/global before/after/current state. Both baseline18
source/analysis/diagnostics reproduce the retained complete18 control. Analysis
allocations decrease by 5,900 calls and 28,932 requested bytes in19; retained
and peak allocation deltas remain unchanged. Load/render allocation counters
are identical. These are whole-analysis counters, not checker attribution.
Native18/19 wall times are172.189/163.958 seconds, CPU170.093/163.193 seconds,
peak RSS44.781/44.453 MiB. Companion18/19 wall times are169.215/164.793 seconds.
The protected original corpus overlapped; no isolated timing ratio is claimed.
Evidence and terminal root audit are under
`target/benchmarks/base083/semantic62-partial-routing-pair/`.
This proves finite Routing parity for the lookup change, not complete Navigation
processing, CPU attribution, bounded eligible-operation growth or final task
acceptance. Those requirements remain open.

The integrated Routing Float-let reduction passes current Gecode compile-only
precheck in 0.165 seconds: status0, empty full diagnostic streams, reaped child,
empty group and null verification error. Root checked all1,041 original/compiler/
standard/helper pins against equal before/after/current state and both emitted
artifacts (FZN841 bytes, OZN91 bytes). Evidence and terminal root audit are in
`target/benchmarks/base083/semantic62-routing-conditional-float-integration-preparation/compiler-precheck/`.
This is a complete supplied-value witness for the nested Float/fix/ceil/set
shape, not proof of symbolic bounds or search coverage. The two reviewed drafts
have been mechanically integrated in scratch, with the shared79-line conditional
prepass present once. Tests-only candidate application and meaningful RED are
next; production and actual Rust remain unchanged.

The operation-rich100/1000 growth comparison completes all eight bounded
native/allocator captures. Root checked full raw streams, exact source/all26
analysis/drop/status parity at each size and every current frozen input/helper
pin against equal before/after captures. All fourteen thesis rules complete;
partial-expression and vacuous-constraint remain Limited identically in both
versions, with no warnings/errors. These limitations are retained, not hidden.
At100/1000 declarations, release19 makes2,185/21,982 fewer analysis allocations
but requests165,164/1,180,228 MORE bytes for the lookup tables. Retained/peak
allocation totals do not change. Native CPU18 is0.719/3.386 seconds and19 is
0.709/3.131 seconds; observed whole-analysis allocation growth is6.766x/6.759x
for18/19 under tenfold source-operation growth. These are two-point observations
with corpus overlap, not isolated speedups or checker CPU attribution. Full
evidence and terminal root audit are in
`target/benchmarks/base083/semantic62-partial-operation-growth/`.

The integrated Routing tests-only RED is meaningful: three existing search
cases pass and the extended symbolic case expects float_let_output Uncovered
but receives Unknown at search.rs2007. Cargo exits101 without timeout in20.075
seconds; root checked complete streams, equal/current state, reaping/empty group
and null verification error. Compiler/fixture setup succeeds. Evidence and root
audit are in
`target/benchmarks/base083/semantic62-draft-workspace-conditional-float-red-search/`.
Reviewed production-only candidate application is next; public assertions remain
unchanged, and no authoritative Rust source has changed.

The first integrated Routing GREEN does not pass. With unchanged public
assertions, float_let_output still receives Unknown instead of Uncovered; an
existing total-controls case additionally reports an unsupported arbitrary-value
call. Two search cases pass and two fail. Cargo exits101 in8.616 seconds without
timeout; root checked complete streams and equal/current source/helper state,
reaping, empty group and null verification error. Preserved evidence and root
audit are under
`target/benchmarks/base083/semantic62-draft-workspace-conditional-float-green-search/`.
No broader gates ran. Both failures require diagnosis and narrow rework; the
saved drafts remain unaccepted and authoritative Rust remains unchanged.

Two further drafts are saved before the next diagnostic run.
`scripts/semantic-index-extremum-precedence-draft.patch` preserves the existing
index-set extremum route before the new parameter-set inspection. The failed
total-controls case uses rank-two index extrema; the new inspection had
intercepted that existing route. This narrow correction is applied only in the
candidate copy and still needs the full search-suite diagnostic. Public
assertions remain unchanged; temporary diagnostic prints stay in scratch and
must be removed before final gates. The prepared run has not launched.

`scripts/semantic-partial-attribution-draft.patch` saves the corrected temporary
native-only instrumentation proposal for checker CPU and allocation counts.
It retains the original module documentation first and leaves the checker call
unchanged. It is unapplied, unbuilt and unmeasured; it must use separate
diagnostic copies and must not be built with the allocator companion. CPU
queries target only the instrumented process itself. No successful attribution
or task acceptance is claimed. Preparation evidence for these drafts is under
`target/benchmarks/base083/semantic62-conditional-float-failure-preparation/` and
`target/benchmarks/base083/semantic62-partial-check-attribution-preparation/`.
Authoritative Rust remains unchanged while the original capture is live.

The precedence-corrected full search diagnostic still fails: two cases pass and
two fail, with Cargo status101 in16.904 seconds and no timeout. Root checked
complete streams, equal before/after/current source and helper state, child
reaping and an empty process group. Evidence and terminal root audit are under
`target/benchmarks/base083/semantic62-draft-workspace-conditional-float-diagnostic-v1-search/`.
The earlier unavailable-body assertion now passes, but a later Boolean-array
guard case receives Uncovered where the unchanged test requires Unknown. Its
unproved array index must remain a partiality veto. The symbolic Routing case
also retains a NoMatch for decision-integer division: its test library only
declares parameter-integer division. Correct that fixture before inferring any
remaining Float-array implementation gap. Both follow-ups await reviewed narrow
changes; no public assertion has been weakened and broader gates have not run.
The original capture remains live on its owned session89518, freshly polled
without new output. Final task acceptance remains open.

The next narrow Routing rework is saved as two draft patches.
`scripts/semantic-boolean-formal-membership-draft.patch` extends the existing
uninitialized Boolean selection membership veto to formal parameters after
strict dependency checks fail. It preserves initialized producers, strict
membership success, full child inspection and the separate Boolean-relation
policy. `scripts/semantic-routing-var-int-div-fixture-draft.patch` adds the
missing decision-integer division signature to the symbolic fixture; the
installed MiniZinc stdlib_math.mzn191 confirms that signature. Public assertions
remain unchanged. Candidate-only application and a fresh full search suite
are authorized; no passing result exists yet.

The separate native-only checker instrumentation copies each match all106
frozen18/19 source files before instrumentation, and differ only in the partial
expression lookup repair. Root verified both complete copy maps, all90 actual
originals, eight helpers and two build tools, with absent output directories.
Two offline/locked release builds started in parallel under their own bounded
runners (300 seconds each). These are diagnostic builds, not stock performance
measurements or task acceptance. Their evidence is under
`target/benchmarks/base083/semantic62-partial-check-attribution-preparation/`.

Both native-only attribution builds complete successfully without timeout:
release18/19 take18.723/18.767 seconds. Root parsed all13 Cargo events in each
complete stdout, read full stderr, found no compiler warnings/errors, verified
equal before/after/current copy/original/helper/tool state, reaping and empty
groups, and checked each frozen executable against its emitted artifact.
Terminal root audits are in the build18/build19 subdirectories of the
attribution preparation. No model was processed with these diagnostic binaries
yet; four bounded captures on the already compiler-accepted growth models are
being prepared. These build results do not close CPU attribution, Navigation
processing, Routing search regressions or final task acceptance.

The corrected GREEN-v2 search suite passes three cases and fails one. The
entire existing total-controls case now passes, including Boolean formal guards
and index-set extrema. The symbolic Float-let output still receives Unknown
instead of the unchanged expected Uncovered after adding the installed
decision-integer division signature. Cargo exits101 in20.529 seconds without
timeout. Root checked complete streams, equal before/after/current source and
helper state, reaping and an empty group. Evidence and terminal root audit are
in `target/benchmarks/base083/semantic62-draft-workspace-conditional-float-green-v2-search/`.
No broader gates ran. A second temporary diagnostic capture is authorized to
locate the remaining limitation; no additional Float implementation repair is
yet justified. Original session89518 was freshly polled and remains live.

The second Routing diagnostic confirms that decision-integer division now
resolves against the corrected fixture. Three cases pass; the same Float-let
output remains Unknown, with a located limitation reading “numeric expression
is outside bounded integer arithmetic” on the conditional body. Cargo exits101
in19.645 seconds without timeout. Root checked complete streams, equal
before/after/current source/helper state, reaping and an empty group. Evidence
and terminal root audit are under
`target/benchmarks/base083/semantic62-draft-workspace-conditional-float-diagnostic-v2-search/`.
The division inspection fallback bypasses its separate integer evaluator for
checked parameter-array selections, but still invokes it for decision-array
selections. That is a concrete next trace candidate; it is not yet a confirmed
repair. Keep literal-zero and unsupported-sibling checks and infer no numeric
value from an unproved selection. No production change or broader gate ran.

All four native checker attribution captures complete with successful own-PID
CPU queries and exact full stock semantic stderr/status parity. Root rechecked
all current inputs, actual and isolated sources, binaries and helpers against
equal global/per-child before/after state, complete stream hashes, reaping and
empty groups. Complete begin/end records and terminal root audit are under
`target/benchmarks/base083/semantic62-partial-check-attribution-captures/`.
At100 declarations, checker18/19 request50,261/48,076 allocations and
5,764,974/5,930,138 bytes; measured walls are6.610/4.288ms and quantized CPU
both0.01 seconds. At1000, they request4,102,067/4,080,085 allocations and
504,375,752/505,555,980 bytes; measured walls are649.824/392.587ms and CPU
0.65/0.39 seconds. The earlier fewer-call/more-byte deltas are now attributed
to this checker. Retained output bytes are identical, while peak allocation
increases with lookup tables (100:77,080 to284,904;1000:731,840 to2,438,672).
CPU has10ms resolution and process-query/snapshot overhead; allocator
instrumentation affects measurements. No isolated stock speed ratio is claimed.
Crucially, checker allocations grow about82x/85x under tenfold source-operation
growth, so whole-analysis growth observations do not establish bounded checker
scaling. That cost remains under investigation. A single unchanged300-second
diagnostic19 Navigation capture is being prepared; it has not launched.

`scripts/semantic-decision-selection-division-draft.patch` saves the reviewed
788-byte candidate correction: the existing division/remainder inspection
bypass admits checked present decision-integer array selections alongside
parameter-integer selections. Such operands retain None in the separate
integer evaluator, leaving the operation Unknown. Both operand inspections,
zero-divisor/unsupported-sibling checks and closed-overflow handling remain
unchanged. Root and an independent reviewer traced the observed domains601
limitation to this exact boundary. Candidate-only application, restoration of
the clean public test and one fresh full search-suite check are authorized.
No passing result or final acceptance exists yet.

The reviewed decision-selection correction passes all four full search cases
with unchanged public assertions, Cargo status0 in20.552 seconds, no timeout
and equal before/after/current source/helper state. Root verified complete
streams, reaping and an empty group. Candidate fmt and Clippy with warnings
denied also pass (1.049/3.695 seconds) with complete terminal audits. Workspace
tests are running; these are candidate gates, not authoritative integration.
Evidence is in the conditional-float-green-v3 search/fmt/clippy directories.

`scripts/semantic-partial-abort-filter-draft.patch` saves an unapplied717-byte
filter reorder against frozen19. It checks the abort reason before constructing
a source location, avoiding repeated PathBuf clones for non-abort limitation
rows without changing the predicate, row order or diagnostics. The remaining
scan cost and after-measurements are unverified. No extra index or engine is
introduced. Root independently reviewed the exact filter and location conversion.

A single original Navigation diagnostic19 capture has started with the unchanged
300-second cap and reviewed own-PID CPU permission. It uses a separate frozen
instrumented executable and does not modify shared or authoritative source.
Missing checker-end metrics will remain censored; the historical stock cutoff
cannot support complete diagnostic parity. Evidence is under
`target/benchmarks/base083/semantic62-partial-navigation-attribution/`.

The combined candidate patch now includes the reviewed Routing corrections.
Root applied it in a disposable directory and confirmed that all 21 resulting
files exactly match the current candidate. The patch is 279,855 bytes, SHA256
b4d124254df43ccabc525e25e5b305b26ca2c5d852afe67371ddf8fb0904ddfa.
Authoritative Rust remains unchanged while the original capture is active.

The current candidate workspace suite passes all 193 tests, with zero failures,
in 45.063 seconds. Together with the passing search, fmt and Clippy checks,
this completes the current candidate Rust gates. Root checked full streams,
unchanged input state and terminal receipts. Evidence is under
`target/benchmarks/base083/semantic62-draft-workspace-conditional-float-green-v3-workspace/`.

The instrumented original Navigation run completed in 98.083 seconds under
the unchanged 300-second cap. It exits 1 for 5,643 warnings: 2,810
decision-variable-operator and 2,833 missing-constraint-label diagnostics,
with no errors or analysis limitations. The partial-expression checker uses
922.638 ms wall time and 0.92 seconds quantized CPU, requesting 1,445,406
allocations and 209,004,603 bytes. Root audited all diagnostic lines, source
state and terminal records under
`target/benchmarks/base083/semantic62-partial-navigation-attribution/`.
These instrumented observations do not establish stock companion parity or
final task acceptance. The allocation filter draft remains unapplied to the
shared candidate; its separate diagnostic build is prepared but unlaunched.

Two isolated optimized builds now pass. The allocation-filter diagnostic20
build completes in 14.332 seconds and freezes a native-only instrumented
executable, SHA256
2977bc1af6084450a8858d9d65e960e6aac71f7a111594a93def9ee80babbb2b.
Root checked every Cargo event, full streams, equal current source/helper
state and terminal records under
`target/benchmarks/base083/semantic62-partial-abort-filter-attribution-preparation/build20/`.
The two matched growth measurements remain pending.

The Routing candidate release20 build completes in 11.763 seconds, freezing
normal CLI and companion binaries and 22 reported optimized library artifacts.
Native SHA256 is
50fa9ffa598f4e802b30f84650b7bcca1c1c4f77c349f3d82ca6f8238dc66c6a;
companion SHA256 is
949524ac62137fdefdb5c118a62749a2a27e3e9dcf1970106bdbed01dd69b687.
Root audited complete build streams, reported artifacts and unchanged source
state under `target/benchmarks/base083/semantic62-draft-workspace-semantic-release20/`.
These builds provide executables for further checks; they do not establish
model acceptance. The original capture remains live on its actual owner.
A stock19 original Navigation CLI/companion baseline pair is running, with
the unchanged 300-second cap for each child; its full comparison is pending.

Both allocation-filter diagnostic20 controls complete with exact full stock19
stderr and status parity. Root audited global/per-child before/after/current
state, complete checker records and streams, reaping and empty groups under
`target/benchmarks/base083/semantic62-partial-abort-filter-attribution-captures/`.
At 100/1000 declarations the checker makes 5,358/53,067 allocation calls and
requests 675,824/6,205,748 bytes. Growth is approximately 9.90x/9.18x for
tenfold source growth, rather than the previous approximately 85x calls.
At 1000 the reorder removes 4,027,018 calls and 499,350,232 requested bytes;
retained and peak bytes remain equal. Observed checker walls are 1.938/251.464
ms and quantized CPU 0/0.25 seconds. These overlapping small measurements do
not establish linear CPU growth or an isolated speed ratio. Remaining scans
are being reviewed separately.

Stock19 original Navigation native processing completes in 95.597 seconds
without timeout, status 1 for the same 5,643 warnings and no other diagnostic
lines. Root checked complete streams and unchanged inputs; the companion
child is still running. Full source/outcome/drop parity remains pending.

The stock19 Navigation pair is complete and root-audited under
`target/benchmarks/base083/semantic62-stock19-navigation-pair/`. Both native
and companion finish status 1 without timeout (95.597/96.900 seconds),
with byte-identical diagnostic streams and empty ordinary CLI stdout. The
actual companion reports one complete resolved root, 5,643 warnings, zero
errors/limitations, all 26 rules Completed and all fourteen thesis rules
Completed. All loaded sources join checked compiler originals and the
reported scope returns to its allocation baseline after drop. This supplies
a complete stock19 baseline; final-candidate corpus acceptance remains open.

Independent review and root inspection confirm two remaining limitation
joins in the partial-expression checker: an all-row scan per operation for
exact ranges, and an all-row scan per item for abort containment. They cost
O(operations * limitations) and O(items * limitations), respectively, even
when there are no abort rows. A small existing-module exact-range row lookup
and ordered abort subset can remove these joins while preserving producer
row order and duplicates, suppressions, containment, activation, empty
iteration checks and final sort/dedup. The observed short checker timings
do not independently attribute CPU to these joins. Other linear searches
remain; no overall linearity claim follows from this proposed correction.

The combined candidate patch now includes the measured abort-filter reorder.
Root reconstructed both previous and new patches in disposable directories:
only partial_expression.rs differs, with the exact reviewed two-line reorder.
The updated patch is 280,128 bytes, SHA256
adcdc3d46a201cb9c8be4efcf7b4b5daa21776461e2663f2fc302869b4bdde85.
The further limitation-lookup correction is still being authored; it is not
in this checkpoint. Authoritative Rust and original capture inputs remain
unchanged. A frozen Routing20 normal CLI/companion pair is now running at
the same 300-second caps to check the original Routing model's body coverage.

The reviewed local limitation lookup is now in the candidate. Its exact-range
row vectors retain producer order and duplicates; its ordered abort subset
keeps the existing file/containment, activation, empty-iteration, suppression
and final sort/dedup behavior. The incremental patch is saved as
`scripts/semantic-partial-limitation-index-draft.patch` (2,241 bytes).
The combined candidate is 281,466 bytes, SHA256
538dd47e37337d682a23f422b3f0727aa3c60c792df3e28c88331b54a4bc56e1.
Root reconstructed all 21 candidate files from authoritative originals in a
disposable directory and checked exact equality to the current source copy.

Direct candidate Cargo fmt, Clippy with warnings denied and workspace tests
all pass. Root consumed complete outputs: 193 tests pass, zero fail, including
default/abort guards, suppression, source-preserving fixes, shared includes
and save/batch behavior. The reviewed source remains 16,560 bytes, SHA256
cf1c4cc94e72d1b64609c89ac074d42102e34d60fa44034012191a93228bb9ec.
Actual original source hashes remain unchanged. This establishes candidate
Rust behavior, not final corpus acceptance; matched cost measurements for
the lookup change are still pending.

The current lookup candidate's direct offline/locked optimized build also
passes. Root retained complete tool output and all 16 Cargo events, checked
reported release artifacts and the unchanged 106-file candidate source map,
and froze the ordinary CLI/companion plus 22 library artifacts under
`target/benchmarks/base083/semantic62-limitation-lookup-release/`. Native
SHA256 is 9a26f7d984b5a422625432b68fceb36b963d7b3470c464680fd86c28558eaab0;
companion SHA256 is
7a070b6c86959894788f9150d1561e461e3827b88868dacb3d48a2c6a38dc83d.
The separate native-only diagnostic21 build passes in 13.998 seconds; its
full event/stream/state audit is under
`target/benchmarks/base083/semantic62-partial-limitation-index-attribution-preparation/build21/`.
Its instrumentation is unchanged; only the checker lookup source differs
from diagnostic20. Matched diagnostic21 measurements remain pending.

A root poll of original89518 returned an unknown handle, but this did not
establish termination. The owning worker's subsequent actual poll succeeds
and reports the same live session; a scoped process query confirms driver
41434 is still running. Authoritative Rust integration remains deferred
because that original driver continues checking its semantic61 source pins.
No driver signal, restart, draining or source/protocol change was performed.

Routing20 native/companion processing both complete without timeout and
match each other's full diagnostics/status. Root audited all streams, source
state, actual companion records and drops under
`target/benchmarks/base083/semantic62-routing20-pair/`. Both retain 180
warnings and 194 limitations; 13 of fourteen thesis rules complete and
search-coverage remains Limited. Entire source/analysis/drop records equal19,
but three located search limitations now expose narrower unsupported facts
at lines182/383/378. Their underlying implementation gaps remain under review.
The prepared diagnostic-block parser had treated analysis-limitation headers
as part of the last warning. Root corrected classification in
`diagnostic-block-root-correction.json`: all 180 actual warning blocks are
byte-identical; only the last three limitation blocks change.

Both diagnostic21 lookup controls complete with full stock19 diagnostic/status
parity and successful own-PID CPU queries. Root audited exact before/after/current
state, complete streams and records under
`target/benchmarks/base083/semantic62-partial-limitation-index-attribution-captures/`.
Checker allocation calls are 5,769/57,082 and requested bytes 738,764/7,136,560
at 100/1000 declarations (9.895x/9.660x growth). Retained bytes remain equal
to20. The added lookup costs 411/4,015 calls and 62,940/930,812 requested bytes;
peak increases by 37,992/529,512 bytes. Observed walls are1.681/200.889ms,
quantized CPU0/0.20 seconds. These measurements do not prove linear CPU
growth or an isolated speed ratio. The remaining repeated default-expression
searches are being reviewed without adding instrumentation.

Current21 original Navigation CLI/companion comparison is now running with
the same source, selected IDs and 300-second caps against the complete19
baseline. Its full comparison is pending. The original semantic61 capture
is still live; its latest checked ledger closes105/124 shards and accounts
for5358/6417 planned roots, leaving1059. No final capture report/post-pins or
terminal receipt exists; all90 original Rust hashes still match.

Current21 original Navigation comparison completes and is root-audited under
`target/benchmarks/base083/semantic62-current21-navigation-pair/`. Both
normal CLI and companion finish without timeout, status1 (95.352/97.785
seconds). Full diagnostic/status parity, manifest, source metadata, entire
analysis, all26/thesis14 partitions and drop records exactly match the
complete19 baseline. All26 rules complete, with5643 warnings, zero errors
and zero limitations; loaded sources join checked compiler originals.
Whole-analyze companion counters decrease by11339 calls and941140 requested
bytes, with retained/peak deltas unchanged. These are whole-phase observations
under possible peer overlap, not checker attribution or an isolated speed ratio.
Routing's remaining three search limitations and repeated default-expression
lookup cost are still being diagnosed. Final whole-corpus acceptance is open.

The candidate partial-expression checker now uses its existing exact-range
index for both default-capture expression lookups. The shared private helper
accepts a lookup callback; input-preconditions retains its original lookup.
First-match ordering and the capture guards remain unchanged. The incremental
change is saved in `scripts/semantic-default-expression-lookup-draft.patch`.
Root reviewed the change and checked the assigned candidate workspace:
formatting, Clippy with warnings denied, and workspace tests all pass.
Matched performance measurements for this change remain pending. The earlier
Navigation comparison covers candidate21, before this callback change.
Authoritative Rust integration and final whole-corpus acceptance remain open.
The original capture's owning worker confirms its session is still live:
112 of 124 shards have closed, covering 5,773 of 6,417 planned roots.
All 90 actual source pins match; final report and post-capture records are
absent. The remaining 644 roots are still being processed.
The combined candidate patch also includes this callback change. Root
reconstructed all 21 candidate files in a temporary directory and compared
them byte-for-byte with the tested copy. Authoritative source remains unchanged.

The native-only diagnostic22 build passes. It differs from diagnostic21 only
in the two default-lookup callback files; instrumentation is unchanged. Root
audited complete Cargo events, stream hashes, source state and frozen binary
under `target/benchmarks/base083/semantic62-default-expression-lookup-attribution-preparation/build22/`.
The binary SHA256 is
`9cfa722015c2056db474595005ea8da7703e7f92312710f95dfb11f72881628e`.
Matched captures remain pending. The build receipt records a denied cleanup
signal attempt after completion, with the child reaped and process group
independently confirmed empty.

Both diagnostic22 controls complete with full stock19 and diagnostic21
diagnostic/status parity. Root checked all streams, counter records and source
state under `target/benchmarks/base083/semantic62-default-expression-lookup-attribution-captures/`.
Allocation counts, requested bytes, retained bytes and peak deltas equal21.
Observed checker wall times are 0.967/59.768 ms at 100/1000 declarations;
quantized own-process CPU deltas are 0/0.06 seconds, versus21's 0/0.20.
Other repeated scans remain; these observations do not establish linear CPU
growth or an isolated speed ratio.

The faithful Routing reduction includes the original wrapped modulo shape,
an `array[int]` filtered set collection and the composite coefficient dividend.
The incremental public fixture is saved in
`scripts/semantic-routing-faithful-reduction-draft.patch`.
Gecode compile-only processing succeeds with no diagnostics and produces
FlatZinc and output metadata; no solving occurs. The existing focused search
test then fails: `float_let_output` is Unknown where Uncovered is expected.
All expectations remain unchanged. Narrow repairs are being prepared in an
isolated copy; the tested shared candidate and authoritative source are frozen.

Independent arithmetic review rules out the proposed coefficient bridge as
an established gap. For the written local coefficient, the integer walker
wraps its initializer in `NumericBound::Defined`; `invariant_integer` returns
unknown for that wrapper and for the enclosing arithmetic. A failure in the
initializer's value interpreter alone does not prove this dividend fails.
No additional coefficient handling is authorized without a reproduced failure.

The first Routing repair passes the existing focused search group with all
assertions unchanged; formatting also passes. Its incremental source patch is
`scripts/semantic-routing-wrapped-selection-forall-draft.patch`.
Only a fully inspected selection plus/minus a closed integer can retain an
unknown division operand. The owning unsupported core-forall path can inspect
the whole quantifier as unknown, with strict ambient checks and no outputs
or local certificates. Membership and strict dependency proofs are unchanged.
The coefficient path remains unchanged. Broader Rust gates and original-model
coverage comparisons are pending; focused arithmetic veto checks are being
added before those gates. The original capture remains live at 112 closed
shards and 5,773 planned roots, with all 90 actual source pins matching.

The isolated partial-expression candidate now indexes call facts by their
existing file/head-start identity, retaining the first producer row. It reuses
`operation_head_start` and the unchanged core-outcome classifier in the two
existing private helpers; input-preconditions retains its original lookup.
The incremental patch is `scripts/semantic-partial-call-head-index-draft.patch`.
Root reviewed semantics and checked formatting, Clippy with warnings denied,
and the full workspace: 193 tests pass. The initial import-format and two
needless-borrow failures were corrected before those successful gates.
Matched measurements for this lookup change remain pending; the tested shared
candidate and authoritative Rust source have not changed.

The isolated Routing repair now passes formatting, Clippy with warnings
denied, and all 193 workspace tests. The same public case retains search
limitations for wrapped selections with a literal-zero division sibling or
a closed addition-overflow sibling. This test delta is saved in
`scripts/semantic-routing-wrapped-arithmetic-veto-draft.patch`.
Original Routing native/companion outcomes and final combined-source checks
remain pending; the authoritative source remains pinned to the live capture.

The combined normal candidate passes formatting, Clippy with warnings denied
and all 193 workspace tests. Root reconstructed all 21 patch paths exactly;
only the four approved Routing/lookup sections differ from the prior patch.
The two overlaid source files were touched before rerunning Clippy/tests so
their preserved copy timestamps could not leave an older Cargo artifact fresh.
`scripts/semantic-candidate.patch` now records this combined candidate.

Diagnostic23 native-only build and both growth controls also pass. Root
audited all streams, counter records and source state under
`target/benchmarks/base083/semantic62-call-head-lookup-attribution-captures/`.
Full diagnostic/status parity with stock19 and diagnostic22 is exact.
Checker wall times are 1.116/10.471 ms at 100/1000 declarations; quantized
CPU deltas are 0/0.01 seconds. Calls are 5,770/57,083; requested bytes are
841,172/7,546,168. The index adds one allocation and 102,408/409,608 bytes
to requested/peak memory, with retained memory unchanged. These are bounded
observations, not an overall linearity claim or isolated speed ratio.
Normal combined binaries and original Routing comparisons remain pending.

Normal combined24 binaries are built and frozen under
`target/benchmarks/base083/semantic62-combined24-release/`. Root audited all
16 Cargo events, complete streams, 106 copied source files, 90 actual source
files, both binaries and 22 library artifacts. Sources match before/after.
Native SHA256 is
`7ef9307455454092dc8d4b70cc6dc192b493c548f4aead7f9a9e7f00ad0c10f1`;
companion SHA256 is
`320fc6c3fdc74181091809b69f0c11dd7df43ec19e35967a7dfd95395f201505`.
The initial command omitted the formatter package containing the example and
failed before compilation; its streams are retained separately. The corrected
command selects both packages and succeeds. Original Routing and Navigation
comparisons are being prepared; final whole-corpus acceptance remains open.

Root reviewed the Routing24 driver changes and checked its current physical
prefix: 1,041 compiler/std/model inputs, frozen assets and the four exact
source deltas match. The original model's Gecode compile-only receipt remains
valid. The serial native24/companion24 pair is now running with separate
300-second caps. Current native/companion agreement is required; old/current
semantic differences remain fully reported. The corrected diagnostic-block
parser counts actual analysis-limitation headers instead of folding them into
the preceding warning. The reviewed Navigation24 prefix also passes, including
all 90 actual source pins; that heavy pair remains unlaunched until Routing.

The original89518 owner confirms its capture is live: 119 of 124 shards have
closed, covering 6,158 of 6,417 planned roots. Its final report/post-capture
records remain absent, with all 90 actual source pins matching. Integration
and final whole-corpus acceptance remain deferred.

Routing24 native processing finishes without timeout in 168.914 seconds,
status 1. Root checked complete streams, hashes and before/after state:
all 180 warning blocks equal20, limitations decrease from 194 to 192, and
no errors are emitted. The three prior search limitations are replaced by
one at line182: an output dependency array access lacks an exact traversal
membership proof. This remaining owning path is being diagnosed; no membership
or output facts are guessed. The companion run is live under root session2641,
so per-rule completion and complete pair acceptance remain unverified.

Routing24 companion processing also finishes without timeout, status 1,
in 169.521 seconds. Root independently checked both full streams and hashes,
complete source records, exact all26 order, reparsed coverage and drop records.
Native/companion diagnostics agree. All fourteen thesis rows equal20:
thirteen are Completed and search-coverage remains Limited. Full source and
reported drop records equal20; no processing errors or Unobserved roots occur.
The remaining membership gap keeps semantic acceptance open. The audit is
`target/benchmarks/base083/semantic62-routing24-pair/pair-root-audit.json`.

The Navigation24 child-free preflight passes again: 1,041 compiler/std/model
inputs, 90 actual source files, frozen binaries and 104 control helpers match.
Routing has terminated before this pair starts. Navigation now runs under
root session85127, with serial native/companion children and 300-second caps.
The original corpus capture remains protected; this comparison uses frozen
assets and makes no isolated timing or whole-task completion claim.

Original owner89518 has now returned terminal exit 1 (worker tool observation
8a1f6d, retained in `semantic61-owner89518-terminal-observation.json`). Root
checked the final report and after ledger: all 124 outer shard commands are
reaped with empty groups; both exact 14/26 rule partitions account for 6,417
roots. Each selection retains 21 Unobserved roots, so capture_complete is false
and acceptance remains unproved. The final inventory reports 189,496 originals
rehashed without changes, with source/binary/helper/before-ledger pins unchanged.
This is terminal evidence, not a successful corpus acceptance result. Root's
compact audit is `exhaustive-semantic61/terminal-report-root-audit.json` under
the existing base083 benchmark directory. The Unobserved roots are being
reconciled separately. Actual source integration waits for Navigation's own
protected-source checks to finish.

Navigation24 is terminal: native 94.289 seconds, companion 96.258 seconds,
without timeout. Root checked full streams/hashes, exact source and analysis,
all 26 and thesis 14 Completed outcomes, drop return and reparsed coverage.
All diagnostics/outcomes match21; 5,643 warnings, zero errors or limitations.
Before/after/current input, protected source and helper pins match. The audit
is `semantic62-current24-navigation-pair/pair-root-audit.json`.

With both captures terminal, root integrated the tested combined24 candidate
into the actual workspace. All 21 patch paths exactly match frozen tested
source. Actual fmt, Clippy with warnings denied and workspace tests pass:
42 suites, 193 tests, zero failures. Source-preservation/default/include/fix,
save and batch behavior remain covered by existing public workspace checks.
Evidence is `combined24-actual-integration.json` and the complete actual gate
logs under the existing base083 benchmark directory. Task acceptance stays
open pending Routing and fresh complete corpus reconciliation.

The proposed direct selected-direction Equality case passes unchanged
combined24 (focused test exit 0, non-timeout). It does not yet reproduce the
remaining original Routing limitation; implementation changes are withheld
while its actual traversal path is traced. The full result is retained in
`semantic62-routing-equality-reduction/focused-red/`.

The conditional selected-direction reduction reproduces the remaining
membership failure on unchanged combined24: focused test exit 101, non-timeout,
with the exact output-dependency membership diagnostic at the nested equality.
The direct equality had passed because the root resolver skips flattened root
Equality clauses; the conditional enters branch interpretation. Root checked
this dispatch independently before authorizing the test-only adjustment.
The exact reduction also compiles with current MiniZinc/Gecode without solving,
exit 0. Complete streams/hashes and reaped empty-group receipts are retained in
`semantic62-routing-equality-reduction/focused-conditional-red/` and
`compiler-conditional/`. The faithful test delta is
`scripts/semantic-routing-conditional-equality-draft.patch`; it is not yet
applied to actual tests. A local checked-Unknown-only repair is being prepared;
strict membership/output guarantees and failures remain required.

Root independently reconciled the final 21 Unobserved executions per preset:
five pending BNN analyses, eleven later shard008 roots never reached, and five
observed non-UTF-8 data input errors. Thus there are 16 absent root records,
not 21. Root checked all 40 relevant native/companion receipts, 80 full stream
hashes, exact original path/hash/byte/mode joins, both raw absent-record sets
and observed input-error rows, and all five actual UTF-8 failure offsets.
The compact review and root audit are in `semantic61-final-unobserved-review/`.
No individual performance failure is inferred for the eleven unreached roots.

The remaining pending BNN original, cellda_y_10s, passes current Gecode compile
without solving: exit 0, non-timeout, 4.227 seconds, reaped with an empty group.
Root checked full streams, equal 1,042-entry input/tool/standard maps and actual
FlatZinc/output artifacts. Evidence is `semantic62-cellda-compile/`.
Together with the retained exact positives, all five pending BNN models have
compiler-positive evidence; their Zincite deadlines remain valid-processing
failures to resolve. Historical no-model/undefined-width crossword checks do
not establish invalid contents; the encoding boundary remains separate.

The eight remaining unreached sources without compiler evidence now pass
current Gecode compilation with matching original sibling data. Root reviewed
the exact commands and launched at most two independent checks, each capped
at 60 seconds. All eight exit 0 without timeout, are reaped with empty groups,
and produce FlatZinc/output artifacts. Root checked complete streams/hashes,
equal before/after original/compiler/standard/helper maps and every output.
Evidence is `semantic62-unreached-source-prechecks/`, including root-audit.json.
No solving, synthetic data, conversion or original rewrites occurred. These
are compiler-positive pairs; fresh Zincite outcomes remain to be collected.

The proposed Routing equality repair passes the faithful conditional positive
and the selected arithmetic/literal overflow checks, but a generator-source
countercheck fails: `Rows=1..(9223372036854775807+1)` incorrectly yields search
Completed. Root verified the focused exit 101, full streams and hashes, unchanged
source maps and reaped empty process group. Evidence is
`semantic62-routing-equality-reduction/focused-header-countercheck/`. The tracked
conditional test draft now retains this countercheck and the preceding negative
variants. Production remains in the assigned ignored workspace; actual source
is unchanged combined24. Header safety is being repaired before integration.

Current combined24 processes the previously unreached original cable-tree
model in both native and companion controls without timeout. Root checked
full streams/hashes, all original/asset/source/helper pins, complete resolved
source records, exact 26-rule and 14-thesis partitions and drop return. Both
status 1 and full diagnostics match: 17 warnings, 11 limitations, zero errors
and no Unobserved rows. Thesis unbounded-variable and search-coverage remain
Limited. Located reasons include the Boolean comparison sum defining N, the
pow-containing objective, and redundant_constraint around all_different. These
are current support gaps for investigation, not deadline or compiler-invalid
classifications. The original model/data pair already has positive Gecode
compile-only evidence. The pair ran with shared CPU, so no isolated timing
comparison is claimed. Evidence and independent root audit are retained in
`semantic62-ctw24-pair/`; whole-task acceptance remains open.

The other seven previously unreached compiler-positive original models also
finish current24 native/companion processing without timeout. Root checked
complete streams/hashes and before/after/current pins, seven complete resolved
root/drop records, exact all26/thesis14 partitions and no Unobserved rows. Native
and companion status 1 and full diagnostics match: 323 warnings, 430 limitations
and zero errors. Every model retains at least one thesis limitation; this
resolves missing processing observations, not semantic acceptance. Evidence is
`semantic62-unreached-seven24-pair/`, with root-audit.json and full root-results.json.
Pentominoes retains only array-index-start among thesis rules: the local bounds
q/s are initialized by parameter array selections, which the numeric-bound
walker currently rejects. Other distinct facts are being reviewed. These
controls shared CPU and support no isolated speed comparison; final corpus
evidence must follow final source changes.

The remaining three never-reached originals (unison, gbac and lot-sizing)
pass current MiniZinc model-check-only at their exact 2020 paths, using Gecode
and no solve. This checks model processing, not instance flattening. Native24
and companion24 then finish all three without timeout and match status 1 and
full diagnostics: 452 warnings, 585 limitations and zero errors. Root checked
full streams/hashes, exact26/thesis14 accounting, complete resolved source/drop
records and before/after/current original/asset/source/helper pins. Evidence is
`semantic62-three-tail24-pair/`. All eleven roots never reached by the old shard
now have current processing observations; each retains thesis limitations.
These separate controls preserve the incomplete historical capture and do not
replace the final full corpus requirement.

The faithful Pentominoes local-axis public reduction passes current MiniZinc
model-check-only with Gecode and no solve, but fails unchanged24 ArrayIndexStart
on both q/s axes. Root checked the focused exit 101, full diagnostics and
unchanged105 baseline files. The negative uses literal column6 against written
axis1..5, independent of parameter defaults. Evidence is
`semantic62-local-selection-axis-repair/focused-red/` and `compiler-positive/`;
the test-only delta is retained in `scripts/semantic-local-selection-axis-draft.patch`.
A separate consumer-local repair is being implemented without changing global
minimum, membership or output facts.

The frozen24 largest BNN control, cellda_y_10s, reaches both 300-second
processing deadlines. Root checked actual terminal -9/timeout receipts, reaped
empty groups, full streams and before/after/current original/std/frozen-source/
asset pins. Each child consumed about295 CPU seconds and peaked at about5,049 MiB
RSS. Companion records load followed by analyze begin; no root/drop/completion
record exists, so all26 and thesis14 outcomes remain Unobserved. No completion,
allocation delta, isolated timing ratio or leak is inferred. The current Gecode
compile-only positive remains valid. Evidence is `semantic62-cellda24-pair/`,
including root-audit.json and reparsed coverage.json. Bounded cost attribution
is being prepared; the cutoff remains explicit and acceptance open.

The Routing conditional equality repair is now integrated from its checked
staging workspace. The faithful public case uses bare decision-variable cells
in the coordinate literal, matching the original header shape. The baseline
fails on unproved selected-direction membership; the repair permits inspection
without deriving membership or callable output. Arithmetic selector failures,
literal overflow and generator-header overflow remain explicit limitations.
Only callable_definitions.rs and tests/search.rs change. Root checked all 106
source/fixture pins and the complete validation streams: formatting and Clippy
pass, and 193 workspace tests pass with zero failures across 42 summaries.
The integrated files exactly match those tested bytes. Evidence is retained in
`semantic62-routing-equality-reduction/header-v2-workspace-gates/` and
`integration-current.json`. Original Routing coverage and final corpus checks
remain pending; base-083 remains open.

The original Routing model finishes both normal25 controls within their
unchanged 300-second limits. Root independently checked full streams, before/
after/current pins, terminal status and empty reaped groups, source/drop records,
and exact26/thesis14 accounting. Both status1 and complete diagnostics match24:
180 warnings, 192 limitations and zero errors. All fourteen thesis outcomes are
unchanged: thirteen Completed and search Limited at line182. The focused repair
does not establish a fix for this original gap. Evidence is
`semantic62-routing25-pair/`, including `pair-root-audit.json`; further diagnosis
is being prepared rather than declaring original coverage complete.

Gecode compile-only of the exact two-cell public Routing fixture exposed an
unrelated min(empty) failure. The fixture now retains four bare decision-variable
cells, giving both pipe selections nonempty sets while preserving the relevant
header shape. That source compiles successfully without solving, with identical
original/std pins before and after, in `compiler-header-v3/`. The four-cell test
still fails the baseline at the intended membership limitation. Its interrupted
current focused run printed a passing test but lost its parent terminal receipt;
that observation is not counted as a completed control. Workspace validation of
the corrected current test follows separately. Frozen25 build files remain
unchanged, and the failed two-cell compiler result remains recorded.

The bounded native Cellda attribution control also reaches its 300-second
deadline. Its single owned-child sample at age240 succeeds and captures a raw
graph. Root checked exact PID34361, invocation window, full native/sampler
streams, reaped empty groups and before/after/current input/asset/helper pins.
The graph's main-thread branch has 80 samples, with61 under numeric-fact bounds
analysis and deep Bounds::expression recursion. This identifies a path for
inspection within that one-second window; it proves neither whole-run share nor
allocation/leak attribution. Evidence is `semantic62-cellda24-native-attribution/`,
including `root-audit.json`. Native per-rule/root/drop completion remains
unavailable. Analysis of the full graph and the relevant producer is in progress.

The staged Pentominoes v2 positive and literal-membership/closed-overflow
counterchecks pass in isolation, but integrated workspace validation fails the
positive on both local axes. Formatting and Clippy pass. Root reproduced the
failure in a separate current focused run, with terminal101 and unchanged
production/test hashes; the isolated and integrated array-index/test files are
byte-identical. The actual callable-definition source includes the committed
Routing repair, so the difference is being diagnosed before this local-axis
change is accepted or committed. Evidence is `semantic62-current26-workspace-gates/`
and `semantic62-current26-local-focused/`. The candidate remains uncommitted;
base-083 acceptance stays open.

The apparent integrated Pentominoes failure was a stale main library artifact.
A fresh exact-source diagnostic copy passes, then passes again without logging;
the old main rlib lacks the helper symbol. Root cleared only the lint package's
build artifacts, confirmed all48 frozen24/25 library/binary assets unchanged,
and rebuilt the actual test successfully. No Equality semantic change was needed.
Separately, a new element-domain overflow counterexample genuinely fails fresh
source with no limitation. V3 adds the same discarded-value error veto to the
retained element domain, alongside the existing axes/header checks.

The final local-axis repair and all three closed-overflow counterchecks now pass
actual workspace validation: fmt, Clippy and193 tests across42 summaries, with
zero failures. Root checked complete streams/terminal receipts and all106 source/
fixture pins before, after and currently. Evidence is
`semantic62-current26-v3-actual-workspace-gates/`, including `root-audit.json`.
Only array_indices.rs and the existing domains regression change for this repair;
no domain bounds, membership, parameter defaults or output guarantees are added.
The corrected four-cell Routing fixture also passes these complete checks.
Fresh original Pentominoes outcomes and final full-corpus validation remain pending.

Root independently reparsed all837 raw sample-tree rows and checked every
immediate-child subtotal. The80 disjoint samples partition between two numeric-
fact call sites:61 evaluating bounds and19 locating expression nodes. Source
review confirms repeated evaluation of left-associated numeric prefixes and
repeated root-to-range lookup. This is a concrete repeated-work mechanism, not
a measured whole-run share or allocation volume. A small public32/256-term
fixed-constraint-count growth control is being prepared before a performance
repair; the original Cellda cutoff remains explicit.

Original Pentominoes now completes all14 thesis rules on the committed normal26
release. Both native and companion finish without timeout, report10 warnings,
zero errors and3 remaining non-thesis limitations, and have identical complete
diagnostic bytes/status. Source/dependency metadata matches the retained24 root;
array-index-start changes from Limited to Completed. Root independently checks
full streams, receipts, unchanged inputs/helpers/current90 source pins and reaped
empty groups. Evidence is `semantic62-pentominoes26-direct-control/root-audit.json`.
The abandoned preparer draft is preserved; root's runnable derivative corrects
its hash-name and newline errors before launch. Full-task acceptance stays open.

Both independently generated fixed64-constraint weighted-chain inputs, widths32
and256, compile successfully with current MiniZinc/Gecode without solving. Root
checks exact generated bytes, full streams/receipts, all1035 standard files and
FlatZinc/output artifacts. Evidence is `semantic62-cellda-width-compile/`; frozen24
serial native/companion observations are now running under300-second child caps.
Compiler success alone establishes neither performance nor all-rule completion.

The four frozen24 weighted-chain controls finish and match full native/companion
diagnostics/status at each width. Both roots are complete/resolved, all26 rules
Completed,128 warnings and zero errors/limitations, with exact14 thesis accounting
and drop checks. Root verifies full streams, unchanged input/helper/frozen artifact
pins and every reaped empty group. Evidence is
`semantic62-cellda-width24-controls/root-audit.json`. Increasing width32 to256
at fixed64 constraints changes whole-analyze allocation calls3,087,078 to92,564,954
and requested bytes308,935,118 to6,255,214,532; retained deltas151,440/151,568 bytes
remain close. Native CPU is0.836/8.040 seconds. These are two complete observations,
not per-producer allocation attribution or a candidate speedup. Together with the
raw sample and source review they justify preparing a small repeated-evaluation
repair; no performance production change has been authored yet.

The public Hoist mixed value/callable reduction compiles with Gecode. A tests-only
run on unchanged production genuinely fails f(1): its candidate vector contains
the array Value and two Functions. The narrow Callable-context binding arm now
separates exactly one top-level Value from ordinary callable candidates, leaving
nearest scope lookup, value references, overload ranking and other mixtures intact.
The same regression passes, including crossing-overload ambiguity, unknown-type
veto and local shadowing. Actual workspace fmt/Clippy/tests all pass:193 tests,
42 summaries, independently checked full streams, unchanged106 source/fixture
pins and reaped empty groups. Evidence is
`semantic62-hoist-callable-preparation/actual-workspace-gates/root-audit.json`.
Fresh original Hoist all26 outcomes remain pending; this focused repair does not
establish whole-model or base-083 completion.

The committed Hoist repair is frozen in audited normal27 opt3 binaries/22 libraries
from106 current source/fixture files. Fresh original Hoist native and companion
finish, match complete diagnostics/status and preserve source/dependency metadata.
All14 thesis rules now Completed; warnings remain24 and errors zero. Limitations
fall113 to14; remaining Limited rules are index-set-mismatch, partial-expression,
vacuous-constraint and missing-input-precondition. Root rehashes full streams,
compiler/std/model assets, helpers and current90 sources and checks reaped empty
groups. Evidence is `semantic62-hoist27-direct-control/root-audit.json`. Historical24
comparisons use its exact selected companion root, never invented singleton native
status. This confirms the demonstrated dispatch repair, with whole-corpus and
base-083 acceptance still pending.

SparseMDS's mixed Boolean/enum conditional gap has a current-Gecode-positive
public reduction (compile-only; deprecated-coercion warnings retained). Unchanged
production fails its selected three-argument int_search assertion. The existing
types::join now gives this scalar pair its supported common Int target, preserving
qualification and both written Bool/Enum child identities. Forward/reversed
branches and selected call pass. Actual workspace fmt/Clippy/tests pass:194 tests
across42 summaries; root checks complete streams, source pins and reaped empty
groups. Evidence is `semantic62-sparse-join-repair/actual-workspace-gates/root-audit.json`.
Other SparseMDS membership/definition gaps and fresh original-model outcomes
remain pending; no full-model completion is claimed from this type repair.

Normal28 original SparseMDS native/companion finish and match full diagnostics/
status with preserved source/dependency metadata. Thesis completion rises to13/14: 
only search-coverage remains Limited. Limitations11 to8; warnings36 to39, errors
zero. Type availability now permits additional findings, so warning counts are
not forced to historical equality. Root independently checks full streams, pins
and reaped empty groups. Evidence is `semantic62-sparse28-direct-control/root-audit.json`.
The remaining search definitions/membership facts require further work.

The final numeric-expression pass now indexes first-preorder CST nodes once and
reuses flat outcomes only after declaration interpretation. It preserves supplied
row order/duplicates, file/BOM offsets, NodeKind distinctions and all existing
checked evaluation. Active declaration recursion bypasses memo; a new declaration
cache fill disables it permanently; Symbolic trees are never retained in memo.
Integer-bounds and the uncached evaluator body remain unchanged. Baseline and
candidate numeric groups pass, then actual workspace fmt/Clippy/194 tests pass
with independently checked full streams/source pins and empty reaped groups.
Normal29's complete opt3 build/22 libraries/106 source files are audited.

All four same-input normal29 width controls complete, with exact baseline24 source,
all26 analysis, manifest/drop and full native/companion diagnostics/status equality.
Width32/256 whole-analyze allocations become2,324,655/44,257,189 (baseline
3,087,078/92,564,954); requested bytes267,874,070/3,554,653,516 (baseline
308,935,118/6,255,214,532). Native CPU0.777/4.228 seconds versus0.836/8.040.
Retained and peak analyze deltas are unchanged; native RSS is not reduced in these
two observations. Evidence is `semantic62-cellda-width29-controls/root-audit.json`.
This confirms a material repeated-work reduction without claiming linear growth
or per-producer allocation attribution. Original Cellda's unchanged300-second
controls and full-task acceptance remain pending.

The Routing182 logging-only diagnostic completes with the existing search
limitation. Its64-record cap is reached before the final whole-source trace;
early logs identify reflected 2D selections and ordinary symbolic uncertainty,
not a proven final repair. Source/streams/groups are independently checked in
`semantic62-routing182-diagnostic-preparation/capture/root-audit.json`. No logging
exists in normal29/main. A narrower diagnosis remains necessary.

Original current-Gecode-positive Cellda29 still hits the unchanged300-second cap
for both serial native and companion. Both are reaped with empty groups and full
streams retained; companion has manifest/load/analyze-begin but zero completed
root records. All26 rules and14 thesis outcomes remain Unobserved, with no native
completion count, semantic parity or whole-run allocation result inferred. Root
checks equal before/after/current original/std/helper/frozen29/current90 source
pins and full stream hashes. Evidence is
`semantic62-bnn29-controls/cellda_y_10s/root-audit.json`. Known overlapping tiny
Routing compiler and isolated RED test are recorded separately; no isolated
speedup or memory-leak conclusion follows. The public-width improvement is real
but has not resolved this required original processing failure.

The public Routing membership reduction, with concrete synthetic data, compiles
with Gecode/no-solve and empty diagnostics. Its tests-only current29 run genuinely
fails the new positive inspection assertion with the matching index-proof
limitation; no production change is accepted yet. Exact core-signature/full-child
Unknown-only inspection is being prepared. All main Rust changes are committed;
unrelated newly created logo assets are preserved outside task scope.

The Routing membership repair now passes the retained RED/GREEN public control.
Only the exact selected core `in` signature with present DecisionInt and
ParameterSetInt operands is eligible; every initialized child is inspected.
Unsupported child safety stays Unsupported and successful inspection returns
Unknown, without a membership truth, definition or output-coverage certificate.
The positive result remains Uncovered and closed out-of-range selection remains
Limited. Actual workspace fmt/Clippy/194 tests pass, with full streams/source
pins and reaped empty groups independently checked in the membership preparation
`actual-workspace-gates/root-audit.json`. Normal30 opt3/106 sources/22 libraries
are checked in `semantic62-combined30-release/root-build-audit.json`. The required
original Routing all26 pair and full final corpus acceptance remain pending.
Logo concepts were committed separately at the user’s request.

The full public Sparse search reduction (906 bytes) is current-Gecode
compile-positive without solving. It retains Enum extrema/conversion, both
Boolean sums, guarded zero-based reshape and mixed rank2 selection. The two
implicit Enum-to-Int deprecation warnings are retained. Complete streams,
FlatZinc/ozn and unchanged original/1035 standard/compiler/helper pins are
independently checked in `semantic62-sparse-search-gap-preparation/`
`compiler-positive/root-audit.json`. This establishes the processing prerequisite;
public Zincite RED and conservative repairs remain pending.

The full compiler-positive Sparse public shape is also observed with normal30
all26 companion: 12 warnings, zero errors and seven limitations, including the
five search rows for leaf/unused/used/misclassified and valid[1,i]. This matches
the original unresolved source shapes; no transitive Enum or Bool prerequisite
has been removed to obtain a passing subset. Exact26 manifest/completed-root/drop
accounting, full streams and unchanged source/std/assets are independently
checked in `semantic62-sparse-search-gap-preparation/normal30-public-probe/`
`root-audit.json`. The tiny runtime overlap with Routing30 is retained separately;
no isolated performance conclusion is drawn. Tests-only regression preparation
and the conservative source repairs remain pending.

Normal30 original Routing native and companion now complete with all14 thesis
rules Completed, including search-coverage. Both have status1, matching full
stderr (81,524 bytes),182 warnings and zero errors. Limitations fall192 to191;
four non-thesis rules remain Limited: expensive-comprehension, index-set-mismatch,
partial-expression and vacuous-constraint. Original/source metadata and drop
records equal25; full streams, current pins, exact14/26 partitions and reaped empty
groups are independently checked in `semantic62-routing30-controls/root-audit.json`.
Known tiny Sparse probe overlap is retained separately, with no timing ratio.
This closes the observed Routing thesis gap; remaining originals and final full
corpus evidence still keep base-083 open.

The frozen29 Cellda phase-only diagnostic is terminal at the unchanged300-second
limit. It records102.261575 seconds in resolve_integer_bounds and6.780964 seconds
in resolve_numeric_facts. The first numeric memo invalidation is the final output
Generator (declaration1374), after the weighted source expressions; it is not
established as the remaining cutoff cause. Integer-bounds still reevaluates each
eligible prefix without memo, a concrete local cost candidate. Substantial cost
elsewhere remains unattributed. Full build/native streams, equal1182 physical
original/std/scratch/assets/helper pins and both reaped empty groups are checked
in `semantic62-cellda29-phase-diagnostic/root-audit.json`. Native ends timeout/-9,
with no completed rule/drop record; all26 acceptance remains Unobserved. This is
instrumented phase attribution, with no isolated speedup, allocation or semantic
acceptance claim. A one-line invariant-pass memo candidate is scratch-only and
its unchanged correctness controls are being compared before acceptance.

Exact compiler-positive original CTW is freshly observed with normal30 native
and companion. Both finish with17 warnings, zero errors and11 limitations;
unbounded-variable and search-coverage remain thesis Limited. Complete all26
analysis/source/drop records and full native/companion diagnostics equal the
retained24 control. Unchanged compiler/original/std/current106 source/assets,
full streams and reaped empty groups are checked independently in
`semantic62-ctw30-direct-control/root-audit.json`. Its Bool generator sum,
ParameterInt powers and bodyless forwarding prerequisites remain separate
confirmed work; they are not claimed repaired by Routing membership inspection.

One-line invariant integer-bounds memo reuse is now integrated. This producer
reads immutable domains/call facts and has no declaration/definition cache state;
existing file/range/NodeKind, active-recursion and no-Symbolic guards remain.
Four unchanged numeric groups pass before/after; main fmt/Clippy/194 tests pass
with complete streams/source/group evidence. All106 main sources match the
normal31 release, recorded in its `root-integration-audit.json`.

Eight same-input width30/31 native/companion controls complete with all26 and14
Completed and exact complete source/analysis/drop/status/full diagnostic parity.
At width32/256, whole-analyze allocation calls fall2,324,655/44,257,189 to
1,764,592/8,581,094; requested bytes267,874,070/3,554,653,516 to
232,699,846/1,226,451,516. Retained/peak analyze deltas stay unchanged. Native
CPU is0.754/4.142 seconds before and0.742/1.931 after; native RSS79.44/411.81
MiB before and74.41/391.03 after. Candidate width32 first-use wall1.189 seconds
exceeds baseline0.763 despite similar CPU; that observation is retained rather
than described as a latency improvement. No per-producer allocation or isolated
speed ratio is claimed. Full streams/current pins/empty reaped groups and exact
parity are checked in `semantic62-cellda-width30-31-controls/root-audit.json`.
These controls demonstrate local repeated-work reduction; original Cellda31 and
remaining originals/fresh full6417-root acceptance are still required.

Normal31 original Pentominoes native/companion recheck completes all14 thesis
rules. Complete all26 analysis/source/drop and full diagnostics/status equal26,
with10 warnings, zero errors and three non-thesis limitations. Original+02.dzn
compiler-positive evidence stays unchanged. Current original/std/106 source/assets,
full streams and reaped empty groups are independently checked in
`semantic62-pentominoes31-direct-control/root-audit.json`. This is a focused
original regression check after invariant memo reuse, not final corpus acceptance.

Normal31 original Cellda native finishes within the unchanged300-second cap:
286.917 seconds wall,286.193 seconds child CPU,status1 with no timeout. Complete
native streams and unchanged pins are retained in
`semantic62-cellda31-controls/cellda_y_10s/native31/`; its process is reaped and
the group is empty. The companion is still running. Native termination alone
does not establish rule completion or diagnostic parity; semantic acceptance
remains pending the companion and independent full capture audit. The earlier
normal29 timeout remains a censored baseline, with no speed ratio claimed.

Original Cellda31 companion now also completes within300 seconds (290.579 wall,
289.624 child CPU). Independent full capture audit confirms all26 and all14
Completed,14707 warnings,zero errors/limitations, complete source/root/drop
accounting and exact3,459,979-byte native/companion diagnostic parity. Current
original/std/assets/helpers/main source pins and both reaped empty groups are
checked in `semantic62-cellda31-controls/cellda_y_10s/root-audit.json`.
Whole-analyze still requests67,602,244,693 bytes in909,312,633 allocations with
10,737,755,746-byte peak delta; native/probe RSS is11,497.73/14,271.92 MiB.
These are absolute measurements, not an isolated performance ratio or evidence
of efficient scaling. Original Cellda processing/coverage is now observed; four
other BNN originals and final full-corpus acceptance remain pending.

The first frozen Sparse production candidate fails its focused existing group:
the unchanged rank2 Int matrix sum loses expected Scalar search coverage and
returns Unknown at search.rs1795, before the new906-byte Sparse case executes.
This is a real earlier behavior regression, not a passing GREEN or acceptance
of the candidate. Full streams, unchanged106 candidate sources/helpers and
reaped empty group are checked in
`semantic62-sparse-search-gap-preparation/focused-green/root-audit.json`.
The candidate remains scratch-only; its selected aggregate tuple checks are
being corrected without weakening that existing assertion.

Sparse candidate v2 preserves the existing matrix sum and reaches/passes the
full906-byte positive with its unchanged Unknown/uncovered expectations.
Ordinal-zero and count-overflow counters also pass. The next counter fails:
literal Enum-array selection outside its written1..1 axis completes without a
limitation instead of the expected Limited result (search.rs2536). Candidate v2
remains unintegrated. Full streams/unchanged106 sources/helpers/empty reaped
group are checked in `semantic62-sparse-search-gap-preparation/focused-green2/`
`root-audit.json`; the closed-axis inspection path is being diagnosed.

Independent source review locates that counter's Boolean context: the existing
relational_operand_dependencies intentionally converts undefined numeric/Enum
selection to false and returns only forward dependencies. Requiring Limited
there was an overstrong new test expectation. Preserve that relation behavior;
the targeted counter is being moved to a standalone raw Enum initializer,
where the new closed-axis veto applies. Production v2 remains unchanged pending
the corrected public check; failed v2 evidence remains retained.

Current CTW public reduction retains rank2 input/index_set_1of2, nested selectors,
written Boolean generator sum, symbolic ParameterInt powers and the ignored-capable
redundant wrapper. Its model+data passes current Gecode compile-only with no
warnings/errors; normal31 full all26 probe finishes with1 warning,zero errors and
6 limitations. Thesis unbounded-variable and search-coverage remain Limited at
the penalty/objective/wrapper shapes. Current inputs/std/compiler/assets/helpers,
full streams and empty reaped groups are checked in
`semantic62-ctw-current-gap-preparation/{compiler-positive,normal31-public-probe}/`
`root-audit.json`. Generator Bool-sum inspection already exists; the actual new
source-header gap is rank2 index_set_1of2 inspection, alongside ParameterInt pow
and bodyless redundant_constraint. Repair/public RED preparation is pending.

Original sysadmin4 normal31 native/companion both finish (6.708/6.289 seconds)
with all26 and all14 Completed,1110 warnings,zero errors/limitations. Full
316,077-byte diagnostics and statuses match; source/root/drop records are
complete. Existing current-Gecode compile-only prerequisite, all original/std/
source/assets/helper pins, full streams and reaped empty groups are independently
checked in `semantic62-sysadmin4-31-controls/sysadmin_4_2s/root-audit.json`.
This observes another formerly missing original; inventory4,sysadmin5,navigation
current controls and final full-corpus acceptance remain pending. Sparse v3's
focused regression runs after this pair, with production v2 unchanged.

Original inventory4 normal31 native/companion now finish (28.492/27.278 seconds)
with all26 and all14 Completed,4237 warnings,zero errors/limitations. Complete
source/root/drop/status accounting and full1,061,429-byte diagnostic parity,
current original/std/source/assets/helpers and both empty reaped groups are
checked in `semantic62-inventory4-31-controls/inventory_4_8s/root-audit.json`.
Sysadmin5/navigation current controls and final full-corpus acceptance remain.

Sparse v3 reaches the full positive and all intended negative outcomes; its only
failure is demanding an enclosing-constraint start location when the Boolean
selector limitation precisely covers722..756. Full streams and equal captured
106-source/helper maps are checked in focused-green3/root-audit.json. V4 changes
only that anchor to the exact offending selector, leaving production v2 and all
outcome assertions unchanged; its focused runtime is pending.

Sparse focused v4 now passes the unchanged existing group, exact906-byte public
positive and all four meaningful negative outcomes/precise locations. Full
streams and unchanged106 scratch-source/helper pins are independently checked
in focused-green4/root-audit.json. Exactly callable_definitions.rs and search.rs
are integrated; main31 invariant-bounds memo is preserved. Present Bool sum,
Enum construction/conversion/extremum prerequisites, rank1 reshape/Enum selection
and rank2 nondefining Boolean relation inspection retain Unknown and discard
value/membership/output certificates. Existing core rank2 Int sum flattening and
Boolean relation partiality are preserved. Main formatting/Clippy/workspace
tests pass with full streams/current106 sources/helpers/empty reaped groups
checked in actual-workspace-gates/root-audit.json. Current realstdlib public and
original Sparse native/companion controls, remaining originals and final full
corpus still keep task acceptance open.

Normal32 release is built from fresh106 copies exactly matching committed main;
full Cargo streams, opt3 artifact events, both binaries/22 libraries, current
main/copy pins and empty reaped group are checked in
`semantic62-combined32-release/root-build-audit.json`.
The exact906-byte compiler-positive Sparse reduction now completes all26/all14
with13 warnings,zero errors/limitations using the real standard library; full
streams/current inputs/std/assets/helpers/root/drop accounting are checked in
normal32-public-probe/root-audit.json.

Original Sparse32 native/companion finish with all14 thesis Completed,40 warnings,
zero errors and one non-thesis vacuous-constraint limitation. Full diagnostics/
status parity, original+breast-cancer_train4 compiler-positive prerequisite,
current original/std/main106/assets/helpers and both empty reaped groups are
checked in `semantic62-sparse32-direct-control/root-audit.json`. Compared with28,
five search and two suspicious-domain limitations disappear; one advisory for
the genuinely uncovered valid array is now emitted. No whole-array coverage is
invented by inspecting its nondefining Boolean relation. This closes observed
Sparse thesis coverage, while CTW/other originals/fresh full corpus remain open.

CTW tests-only RED passes selected metadata/sum/power/wrapper tuples then exposes
the known penalty/objective/wrapper gaps. It also reveals an extra miniature
all_different body limitation from a missing standard Boolean disjunction
signature. Full streams/equal captured106 sources/helpers/reaped empty group are
checked in focused-red/root-audit.json. The fixture-only signature correction
is being prepared separately; no production repair is accepted from this RED.

Corrected CTW tests-only RED2 resolves the exact selected standard tuples and
retains the Boolean generator body, then fails only on the same five public
search/unbounded penalty/objective/wrapper limitations. Missing fixture Boolean
disjunction is repaired with the exact standard signature; no fixture body,
public input or production semantic behavior changed. Full streams/current106
scratch/helper pins and empty reaped group are checked in focused-red2/root-audit.json.
Its case is rebased by insertion onto fresh current32 copies, preserving all
Sparse32 code/tests; the reviewed CTW production seams remain scratch work.

Original macc32 native/companion freshly reproduce109 warnings,zero errors and
118 limitations with identical57,160-byte diagnostics. Array-index-start,
constant-variable,unbounded-variable and search-coverage remain thesis Limited.
Current original+37.dzn compiler-positive prerequisite,std/main106/assets/helpers,
full streams/complete all26 accounting and both empty reaped groups are checked
in `semantic62-macc32-direct-control/root-audit.json`. WORLD's ParameterSetInt
union is a concrete shared domain prerequisite being reviewed separately; this
observation does not claim the other macc gaps are repaired.

Original sysadmin5 normal32 native/companion finish (25.185/25.363 seconds) with
all26 and all14 Completed,3251 warnings,zero errors/limitations. Full diagnostic/
status parity, complete source/root/drop accounting, current original/std/compiler/
main106/assets/helpers and both empty reaped groups are checked in
`semantic62-sysadmin5-32-direct-control-v2/root-audit.json`. The first derivative
stopped before any measurement child because its historical compiler ledger
uses hash/bytes without a mode field; every1038 historical input pin field was
independently rechecked unchanged before the schema-corrected control. The
deadline stays300 seconds. Current navigation and full-corpus acceptance remain.

Original navigation normal32 native/companion finish (52.743/53.154 seconds)
with all26 and all14 Completed,5643 warnings,zero errors/limitations. Full
diagnostic/status parity, complete source/root/drop accounting, the current
compiler-positive original,std/compiler/main106/assets/helpers and both empty
reaped groups are checked in
`semantic62-navigation32-direct-control/root-audit.json`. All five original BNN
controls now have observed complete thesis/all-rule processing on normal31 or
normal32. These focused observations do not replace the fresh final full corpus;
CTW and macc repairs and whole-task acceptance remain open.

The public898-byte macc union-axis reduction with34-byte data compiles with
current Gecode without solving; full streams/current1035 standard files,
compiler/model/data/helpers, FlatZinc artifacts and empty reaped group are
checked in `semantic62-macc-current-gap-preparation/compiler-positive/root-audit.json`.
Normal32 real-standard-library processing reports one warning,zero errors and
five limitations across array-index-start,constant-variable and unbounded-variable;
all26 outcomes/source/drop accounting are checked in normal32-public-probe/root-audit.json.
The written union has minimum -2 whenever its operand constructions are defined:
the symbolic Inner range is empty or nonnegative and Dummy is nonempty.
A blanket Unknown decline would lose this checkable fact. Operand arithmetic
errors also need an independent veto; initialized-source dependency inspection
alone does not establish that a closed addition is defined. The shared domain
repair is under review; this observation supplies no accepted production change.

CTW current32 scratch GREEN1 passes the positive source-inspection/Unknown/
coverage assertions and rejects the partial wrapper at its source location,
then fails an over-specific division-by-zero message assertion. The existing
projected-parameter dependency check rejects it earlier for strict partiality;
production is unchanged in the assertion-only v2. GREEN2 passes the complete
public group. Full streams/current106 scratch/helpers and reaped empty groups
are checked in focused-green1/root-audit.json and focused-green2/root-audit.json.

The two-file CTW repair is integrated and formatted: selected rank2 integer
index_set_1of2 sources and ParameterInt pow operands receive inspection with
Unknown results; selected bodyless redundant_constraint inspects its child
through existing clauses/instances and discards outputs/local certificates.
Exact selected signatures, full sources, annotations, defaults, optionality,
cycles and partiality remain guarded. Strict dependencies and Sparse inspection
are preserved. Formatting,Clippy and workspace tests pass (194 tests/42 summaries),
with full streams/current106 main sources/helpers and all three empty reaped
groups checked in actual-workspace-gates/root-audit.json. Current release/public/
original CTW controls and final full-corpus acceptance remain pending.

Normal33 release uses fresh106 copies equal to committed main; full Cargo
streams,opt3 artifacts,two binaries/22 libraries,current main/copy pins and
empty reaped group are checked in semantic62-combined33-release/root-build-audit.json.
The unchanged compiler-positive CTW public reduction completes all14 thesis
analyses with the real standard library: two warnings,zero errors and one
non-thesis vacuous-constraint limitation. Its484 loaded files,full streams,
current compiler/public/std/assets/helpers and complete all26/root/drop
accounting are checked in normal33-public-probe/root-audit.json.

Original CTW normal33 native/companion finish (2.705/2.412 seconds) with all14
thesis Completed,18 warnings,zero errors and six non-thesis limitations. Full
diagnostic/status parity,current original+A031 compiler-positive prerequisite,
std/compiler/main106/assets/helpers,all26/source/drop accounting and both empty
reaped groups are checked in semantic62-ctw33-direct-control/root-audit.json.
Compared with30,the observed search/unbounded source-inspection limitations
are removed and the genuinely uncovered auxiliary array receives search advice.
This closes focused CTW thesis coverage; macc/other originals and the fresh
final complete corpus still keep whole-task acceptance open.

Original macc normal33 native/companion finish (0.623/0.624 seconds) with the
same109 warnings,zero errors and118 limitations as32. Array-index-start,
constant-variable,unbounded-variable and search-coverage remain thesis Limited.
Full diagnostic/status parity,current original+37.dzn compiler-positive
prerequisite,std/compiler/main106/assets/helpers,complete all26/source/drop
accounting and both empty reaped groups are checked in
semantic62-macc33-direct-control/root-audit.json. CTW's inspected wrapper path
does not close these original macc gaps. A tests-only public union-domain RED is
being prepared against fresh106 current33 copies; no producer repair is accepted.

All five previously UTF-8-rejected original crossword data files compile with
their original adjacent crossword_opt.mzn using current Gecode,without solving.
Full five compiler streams/receipts/FlatZinc artifacts,current originals/1035
standard files/compiler/helpers and all five empty reaped groups are checked in
semantic62-crosswords-current-prechecks/root-audit.json. Earlier data-only and
synthetic-context failures did not establish original compiler invalidity.
These five files remain required valid-input coverage; Zincite's UTF-8 input
rejection is a concrete processing gap. A byte-preserving loader/parser boundary
investigation is running separately from the union-domain tests-only preparation.
No original encoding or source bytes are changed and no coverage is excluded.

An independent byte scan confirms all ten invalid single-byte spans in these
five originals lie inside trailing block comments; code/literals contain no
invalid UTF-8. Exact byte/comment ranges are retained in
semantic62-crosswords-current-prechecks/root-encoding-classification.json.
The user's compiler-accepted-input requirement supersedes the older UTF-8-only
brief restriction. The brief now requires exact opaque comment-byte preservation,
original byte coordinates and truthful library source access,while retaining
UTF-8 code/literals and rejection elsewhere. This is a required contract update;
the byte-input adapter is not yet implemented or accepted.

Current33 native/companion independently reproduce all five compiler-positive
crossword input failures: status2,five errors,five input_error roots,zero loaded
files. Full diagnostic/status parity,current originals/std/compiler/main106/
assets/helpers,all26/five-root/drop accounting and both empty reaped groups are
checked in semantic62-crosswords33-input-baseline/root-audit.json. No input is
excluded because its bytes occur in comments. The first syntax-only additive
byte parser is assigned separately; formatter/lint/raw-fix integration remains.

Macc tests-only RED1 passes the selected union tuple but fails a written-operand
fact lookup before reaching the consumers. The assertion used a token range
against expression-node ranges. V2 checks the exact binding token and original
declaration's type instead; no fallback or source/type weakening is added.
RED2 passes the selected tuple,original operand identities/types and real Boolean
generator body,then reproduces exactly the same five public domain limitations.
Full streams/current106 scratch/helpers and empty reaped group are checked in
focused-red2/root-audit.json. The reviewed shared Union producer increment is
assigned in that scratch root; its source/partiality/minimum and coverage gates
remain subject to root review and validation before integration.

Original pillars-and-planks normal33 native/companion finish (1.327/1.434
seconds) with33 warnings,zero errors and22 limitations; search-coverage is
the sole thesis Limited rule. Its three located prerequisites are two selected
body/instance boundaries and one direct-definition safety/enforcement boundary.
Full diagnostic/status parity,current original+p-d2 compiler-positive
prerequisite,std/compiler/main106/assets/helpers,all26/source/drop accounting
and both empty reaped groups are checked in
semantic62-pillars33-direct-control/root-audit.json. A read-only investigation
of these original prerequisites runs alongside the two separate repair drafts;
no supported definition or whole-array coverage is assumed.

The reviewed syntax-only ByteParsedFile increment is integrated. Its additive
byte entry points retain exact original bytes/token slices separately from an
explicit UTF-8 analysis view,accept invalid bytes only within lexer-confirmed
comments,and preserve original coordinates. Existing UTF-8 parse/CST/BOM APIs
are unchanged. The single public regression checks raw token/tree reconstruction,
CRLF/code-after-comment positions,malformed directive payloads and invalid
code/string rejection. All30 syntax tests pass in the frozen107-file scratch
boundary; full streams/pins/group are checked in focused-syntax/root-audit.json.
Main formatting,Clippy and all195 workspace tests/42 summaries pass,with full
streams/current107 sources/helpers and three empty reaped groups checked in
actual-workspace-gates/root-audit.json. Formatter/lint/loader/raw-fix callers,
the five original byte-input controls and final corpus acceptance remain open.

The reviewed typed integer-set union increment is integrated through the existing
domain and array-coverage modules. It checks selected core operations,original
parameter identities/written sources and annotations,visits retained arithmetic
errors,and distinguishes conditional floors from attained minima. Symbolic
0..n-1 union -2..-1 retains the provable -2 minimum without gaining membership,
interval,nonemptiness or output certificates. Named retained source failures stay
Limited rather than disappearing behind a parameter-set Unknown shortcut.
The unchanged RED2 public regression now passes,including two -2 axis findings,
three Completed consumers,no whole-array certificate from the Dummy-only
traversal,and the closed overflow veto. Full106 scratch/helper/stream/group
checks are retained in semantic62-macc-current-gap-preparation/focused-green/
root-audit.json. Main formatting,Clippy and all196 workspace tests/42 summaries
pass; current107 sources/helpers,full streams and three empty reaped groups are
checked in that preparation directory's actual-workspace-gates/root-audit.json.
Original macc outcomes require a fresh release/control; other semantic gaps,
opaque-comment caller integration and final complete corpus acceptance remain
open. This is an incremental commit,not task completion.

Fresh normal34 release includes the committed union-domain and byte-syntax
increments. Current main107/copied107,opt3 Cargo events,two binaries,22
libraries,full streams and the empty reaped build group are checked in
semantic62-combined34-release/root-build-audit.json. The public union model
with the installed standard library now completes all26 rules/all14 thesis
analyses with3 warnings,zero errors and zero limitations; the two Places axes
retain -2 advice. Full compiler-positive original/public/std/main/assets/helper
pins,streams and source/drop accounting are checked in
semantic62-macc-current-gap-preparation/normal34-public-probe/root-audit.json.
Original macc normal34 native/companion agree on115 warnings,zero errors and
106 limitations (1.048/0.984 seconds). Array-index-start now completes;
constant-variable,unbounded-variable and search-coverage remain thesisLimited.
Full current compiler-positive original closure/main107/release/helper pins,
diagnostic/status parity,all26/source/drop accounting and both empty reaped
groups are checked in semantic62-macc34-direct-control/root-audit.json.
No original,data-dependent membership or missing coverage is suppressed.
The remaining original support gaps and final corpus evidence remain open.

The additive formatter byte API is integrated. It reuses ordinary formatting
and restores original comment payloads in the actual sorted-include/protected
emission order,checking output comment kind and analysis spelling before raw
replacement. Equal masked comments with different original bytes stay attached
to their own includes. UTF-8 callers keep the existing API and direct path.
The public check covers reversed include attachments,protected raw comments,
CRLF options,idempotence and malformed opaque directives without output.
All24 formatting tests pass in the frozen108-source scratch boundary; full
streams/pins and its empty reaped group are checked in
semantic62-opaque-comment-formatter-preparation/focused-formatting/root-audit.json.
Main formatting,Clippy and all197 workspace tests/42 summaries pass; current108
sources/helpers,full streams and three empty reaped groups are checked in that
preparation directory's actual-workspace-gates/root-audit.json. CLI byte output,
lint loading,truthful raw snapshots/fixes and original crossword acceptance
remain open. Formatter CLI integration is assigned in a separate current108
scratch; this library increment does not complete task083 or corpus acceptance.

The pillars public reduction compiles with Gecode without solving (.291s);
current public/std/compiler/helpers,full streams,FZN/OZN and the empty reaped
group are checked in semantic62-pillars-current-gap-preparation/
compiler-positive/root-audit.json. Its tests-only current34 scratch fails at
the expected consumer assertion after exact wrapper/max/++ tuple preflights.
Only objective1305..1415 and bare auxiliary wrapper1260..1303 produce Search
limitations. Full107 scratch/helper/stream/group checks are retained in
focused-red/root-audit.json. The installed-standard public normal34 probe
confirms the same two Search limitations,plus an unbounded objective gap;
5 warnings,zero errors,7 limitations with all26/source/drop accounting.
Full historical107 release-copy/current108 main/public/std/assets/helpers
and streams/group are checked in normal34-public-probe/root-audit.json;
lint/syntax sources exactly match that release. This reduction does not yet
reproduce the original's two filtered-wrapper Search limitations. The
original/public structural difference requires reconciliation before claiming
a faithful regression or applying the planned wrapper repair. Frozen public
inputs/tests remain separate from main; no coverage is suppressed.

Formatter CLI byte integration is accepted as an increment. Stdin/stdout/check/
write now retain opaque comment bytes; BOM settings,assignment-only data mode,
protected ranges and atomic write/error contracts stay intact. Invalid code
after an earlier opaque comment and valid Unicode reports exact original byte
ranges/columns; malformed directives retain original CRLF/BOM coordinates.
The focused public CLI case checks byte output,BOM-only change detection and
unchanged failing files. All7 CLI tests pass in the frozen108-source scratch
boundary; full streams/pins/group are checked in
semantic62-opaque-comment-formatter-cli-preparation/focused-cli/root-audit.json.
Main formatting,Clippy and all198 workspace tests/42 summaries pass; current108
sources/helpers,full streams and three empty reaped groups are checked in that
preparation directory's actual-workspace-gates/root-audit.json. No original
crossword acceptance,preservation example,raw lint loading/fixes or final
corpus result is inferred from the synthetic CLI check; those remain open.

Fresh normal35 release includes the formatter byte CLI. Current main108/
copied108,opt3 Cargo events,three binaries,22 libraries,full streams and
the empty reaped build group are checked in
semantic62-combined35-release/root-build-audit.json. All five compiler-positive
original crossword data inputs format successfully,retain each opaque trailing
block exactly once and retain all invalid byte values,then format identically
on a second stdin pass using the original path/mode/settings. Their formatted
.dzn copies compile with Gecode without solving. Every FlatZinc body is byte-
identical to the original compile after excluding exactly one checked command-
invocation comment header; each header matches its actual captured argv. All
five OZN files are byte-identical. Full original/std/compiler/main108/release/
helper and generated-artifact pins,complete streams and15 empty reaped groups
are checked in semantic62-crosswords35-format-control-v3/root-audit.json.
V1 used a .stdout extension rejected by MiniZinc; V2 revealed the expected
command-header difference. Those harness failures exclude no original inputs.
Single formatter observations span .120.. .809 seconds and84..566 MiB RSS
for .91..5.99 MB sources. These are not save p95 samples or an efficiency pass;
the sub-MiB case needs the brief's bounded stdin/save budget check,including
its observed84 MiB peak. Raw token/CST/protected preservation-example checks,
raw lint loading/fixes and final corpus acceptance remain open.

The pillars V1 discrepancy is explained by Search policy: bare searches cover
all four position arrays and suppress unavailable-wrapper rows whose targets
are already covered. Original conditional-body comprehensions supply no
whole-array seed. V2 faithfully retains two alternating-coordinate,two-binder
conditional searches and checks raw callable.unavailable before Search policy.
V2 compiles with Gecode without solving (.263s) and reproduces a raw-producer
RED at line2515 after exact selected wrapper/max/++ preflights: three wrapper
rows at648..840,878..1124,1260..1303 remain unavailable. Full107 scratch/
public/std/compiler/helper pins,streams,FZN/OZN and empty reaped groups are
checked in semantic62-pillars-current-gap-preparation/v2/{compiler-positive,
focused-red}/root-audit.json. Its frozen tests/public sources remain separate
from main; a scoped ignored-wrapper/extrema inspection increment is assigned
against this genuine RED. Neither conditional searches nor ignored constraints
may gain whole-array or output certificates.

The corpus preservation example now parses and formats raw comment bytes.
Token spelling counts, sorted CST events and protected spans compare original
bytes, with UTF-8 diagnostics retaining their earlier debug representation.
Its two inline tests pass, including detection of swapped opaque include
comments and changed protected payloads despite identical analysis text.
Full frozen108 source/helper pins, streams and the empty reaped group are
checked in semantic62-opaque-comment-corpus-example-preparation/
focused-example/root-audit.json. Main fmt, Clippy and all198 workspace tests/
42 summaries pass; current108 sources and full streams/groups are checked
in that preparation directory's actual-workspace-gates/root-audit.json.
Original crossword preservation-example runs still require a fresh release;
model-dependent lint loading and raw snapshots/fixes remain open. This
increment does not complete task083 or substitute for final corpus evidence.

Fresh normal36 release includes the raw corpus checker. Current main108/
copied108, four opt3 binaries, 22 libraries and complete build streams/group
are checked in semantic62-combined36-release/root-build-audit.json. All five
compiler-positive original crossword inputs now pass raw token coverage, CST
coverage, spelling counts, sorted structure, protected bytes and idempotence
with zero parse/reparse errors. Full original/std/compiler/main/assets/helper
pins, complete streams and five empty reaped groups are checked in
semantic62-crosswords36-preservation/root-audit.json. Default source-local
checks succeed; model-loader and raw safe-fix support remain open.

The frozen pillars V2 regression is GREEN with the scoped production draft:
selected bodyless symmetry wrappers reuse ignored-argument inspection and
integer extrema inspect both concatenated source operands while retaining
Unknown extent/value. The genuine failing raw-wrapper assertion, negative
partiality case and no-whole-array/output-certificate checks pass together.
Full historical107 scratch pins, unchanged V2 tests, complete streams and the
empty reaped group are checked in semantic62-pillars-current-gap-preparation/
v2/focused-green/root-audit.json. Main integration, workspace gates and fresh
original/public normal release controls remain pending.

The scoped pillars production draft and faithful public regression are now
integrated. Exact selected bodyless symmetry wrappers are inspected through
the existing ignored-wrapper path; their constraints produce no enforcement
or definition certificates. Selected integer min/max over rank-one integer
concatenations inspect both source operands, retaining Unknown extent/value
and propagating partial-source failures. Strict dependencies and later proof
helpers stay unchanged. Main fmt, Clippy and all198 workspace tests/42
summaries pass; full current108 sources, streams and three empty reaped groups
are checked in semantic62-pillars-current-gap-preparation/v2/
actual-workspace-gates/root-audit.json. Original/public current release
outcomes still need refresh before claiming repaired original thesis coverage.

Fresh normal37 release contains the pillars repair. Current main108/copy108,
four opt3 binaries,22 libraries,complete build streams and the empty reaped
group are checked in semantic62-combined37-release/root-build-audit.json.
Original pillars native/companion agree on39 warnings,zero errors,18 retained
nonthesis limitations (1.655/1.305s); all14 thesis analyses now complete. The
compiler-positive V2 public model likewise completes all14 thesis analyses
with12 warnings,zero errors,4 nonthesis limitations (1.421/1.310s). Both
controls retain every all26 rule partition,exact source/drop accounting and
uncovered-array/auxiliary advice. Full original/public/compiler/std/main/
release/helper pins,streams,parity and two empty reaped groups per control
are checked in semantic62-pillars37-{direct,public}-control/root-audit.json.
No solve ran,coverage certificate was invented or nonthesis gap suppressed.
Remaining original gaps,opaque-byte lint/fixes,save budgets and final complete
corpus evidence keep task083 open.

The normal37 crossword save baseline confirms a bounded performance follow-up.
For original908888B data,50 fresh-process samples per default/nested settings
give p95 127.069/126.840ms,above the100ms budget. The already-formatted1189780B
buffer has p95 141.716/141.293ms; its actual size exceeds1MiB. All200 sampled
outputs match the original preservation control. A separate original-path
stdin first observation reports793.4ms wall/128.8ms CPU and84.547MiB per-child
wait4 RSS,above64MiB. First-use latency is retained,not labelled cold I/O;
its wall/CPU discrepancy remains unattributed. The existing benchmark's
/usr/bin/time -l RSS probes are unavailable because sysctl kern.clockrate
is denied after successful formatter output; no missing RSS is treated as0.
Full current108/original/std/compiler/release/helper pins,streams,50-sample
summary arithmetic and two empty reaped outer groups are checked in
semantic62-crossword37-save-baseline/root-audit.json. CPU/allocation attribution
and a measured bounded repair remain required; no budget is relaxed.

Fresh normal37 original is control confirms the next support boundary. Native/
companion agree on96 warnings,zero errors,131 limitations (2.358/2.377s),
with74 loaded files/413868B. Array-index-start and search-coverage remain
Limited; the other12 thesis analyses complete. Exact all26 partitions,source/
drop accounting,current108/original/compiler/std/release/helper pins,complete
streams,parity and two empty reaped groups are checked in
semantic62-is37-direct-control/root-audit.json. The already compiler-positive
source has a parameter sum/card/SetInt-array selection plus arithmetic feeding
a named1..extent axis; the existing count fallback admits only direct
length/card initializers. A public source-safety/consumer reduction is assigned
without changing production. Search has separate remaining located gaps; no
consumer bridge or task completion is inferred from this diagnosis.

Fresh normal37 original gbac control includes an actual Gecode no-solve
compile with adjacent reduced_UD5-gbac.dzn (.293s),beyond the earlier
model-check-only precheck. Native/companion agree on16 warnings,zero errors,
18 limitations (2.476/2.510s),484 loaded files/772925B. Only search-coverage
remains Limited among the14 thesis analyses. Full current108/original/data/
compiler/std/release/helper pins,FZN/OZN,complete streams,parity,all26/source/
drop accounting and three empty reaped groups are checked in
semantic62-gbac37-direct-control/root-audit.json. The current located Search
gap is assigned for an independent read-only reduction; no acceptance claim
is inferred from the other13 completed thesis analyses.

A frozen diagnostic-only byte-aware counting-allocator probe now attributes
the sub-MiB crossword case without changing production. Raw output exactly
matches the native formatter; all source allocations return to baseline2023B
after drop. Parsing has718690 tokens/358996 nodes,retains50.671MB absolute
live bytes,uses851989 allocation calls and requests132.481MB. Formatting
adds a26.513MB peak delta,with77.184MB absolute live peak and1.347MB retained
delta. Read/parse/format instrumented timings are.200/51.980/52.612ms and are
not save latency or native budgets. Complete main108/scratch108/original/
native/helper pins,opt3 instrumented asset,raw output,full streams and two
empty reaped groups are checked in semantic62-crossword-allocation-preparation/
root-runtime/root-audit.json. Aggregate formatting includes layout and opaque
restoration; no finer measured attribution is claimed. Source review identifies
the full output-token buffer retained solely to restore opaque comments. A
scoped candidate reusing the existing streaming lexical scanner is assigned;
its actual allocation and native save effects still require measurement.

The public is cardinality-sum axis reduction compiles to FZN/OZN with Gecode
without solving (.171s). V1 test compilation failed because two checks called
a private method; its exact frozen test and streams remain visible and are
not a language RED. V2 removes those inaccessible calls while retaining exact
nested type/instantiation/optionality checks. The V2 behavioral run fails at
line442 on the expected 1..extent ArrayIndexStart limitation after selected
core sum/card,axis identities and matching inspected DefinitionFacts pass.
Full main108/scratch108/public/std/compiler/helper pins,FZN/OZN,streams and
empty reaped groups are checked in semantic62-is-current-gap-preparation/
{compiler-positive,focused-red,focused-red-v2}/root-audit.json. Historical
V1 captures check their exact preserved owned-test copy. A scoped count-axis
consumer repair is assigned; raw numeric bounds and closed overflow vetoes
remain required. No production change or completed original analysis is yet
inferred from this faithful consumer RED.

The streaming opaque-comment restoration candidate is integrated in c361670.
It reuses the lexical protected-range scanner and retains comment ranges rather
than a complete second token buffer. The existing byte-formatting check now
also covers comment-looking string chunks and opaque interpolation comments.
All24 focused formatting tests and main fmt/clippy/workspace gates pass
(198 tests,42 summaries). Full108 source pins,complete streams and three empty
reaped groups are checked in semantic62-opaque-comment-streaming-formatter-
preparation/actual-workspace-gates/root-audit.json. Allocation and native save
budget effects remain unmeasured for this integrated revision.

The normal38 release and identical frozen byte-aware allocator probe confirm
the streaming restoration removes the full output-token buffer. Formatting
peak delta falls26,512,728→1,477,924B and requested bytes56,636,290→6,305,186B;
retained bytes,token/node counts,raw output and post-drop baseline are identical.
Instrumented phase timings remain attribution only. Full pins,opt3 asset,
complete streams and two empty reaped groups are checked in semantic62-
crossword38-allocation-streaming/root-runtime/root-audit.json.

Fresh native save samples retain all200 exact output hashes across50 samples
per original/formatted/default/nested variant. Original908888B p95 is187.771/
147.536ms,still above100ms; formatted1189780B p95 is159.005/252.321ms and is
over the1MiB category. No latency improvement is claimed from this variable
run. The separate actual-path first observation has539.545ms wall,120.986ms
CPU and61.875MiB RSS,versus historical84.547MiB RSS. This one memory sample
is below64MiB; it does not establish a repeated peak or cold-I/O result. The
benchmark time utility still cannot query kern.clockrate,so its RSS remains
unavailable. Full1209 source/std/compiler/release/helper pins,complete outer
streams,two empty reaped groups and all50-sample summary arithmetic are checked
in semantic62-crossword38-save-streaming/root-audit.json. Parsing allocation
traffic remains a measured candidate for bounded diagnosis; acceptance stays
open for the latency budget and final corpus.

The raw-byte lint/fix V1 runtime passes the new opaque-comment CLI,copy-edit
and model-snapshot controls,then fails one old zero-width encoding assertion
(8..8 versus the actual offending byte8..9). It is retained in root-focused.
V2 changes only that existing test range and exact byte-parser message; its
production is identical. The full five focused lint targets (54 tests),one
syntax group and two corpus-example tests pass. Full108 source/helper pins,
complete streams and three empty reaped groups are checked in semantic62-
opaque-comment-lint-fixes-preparation/root-focused-v2/root-audit.json. Only
the15 attributable files are integrated; final workspace gates and original
opaque-data/native outcome checks remain pending.

The first is count-axis production candidate compiles but the faithful V2
positive still has the same limitation at211..220. Full source/std/compiler/
helper pins,complete streams and an empty reaped group are checked in
semantic62-is-current-gap-preparation/focused-green/root-audit.json. It stays
in scratch for diagnosis; neither the test nor acceptance is weakened.

All seven public gbac controls ran with installed MiniZinc/Gecode compile-only.
GCC wrapper/direct singleton,BIN singleton/empty and the left empty-weight
guard compile; raw empty lb_array and the reversed disjunction fail with
"lower bound of empty array undefined". This establishes operand-order behavior
for the installed parameter expression,not arbitrary-disjunction safety.
Full source/1035 standard/compiler/helper pins,seven complete streams,emitted
assets and seven empty reaped groups are checked in semantic62-gbac-public-gap-
preparation/compiler-results/root-audit.json. Search reductions are being
prepared; no semantic repair is inferred from compiler acceptance alone.

Integrated raw-byte lint/fix workspace fmt/clippy and all201 tests pass
(42 summaries). Full108 source pins,complete streams and three empty reaped
groups are checked in semantic62-opaque-comment-lint-fixes-preparation/
actual-workspace-gates/root-audit.json. Normal39 opt3 has four binaries and
22 libraries checked against all108 main/copied sources in its root-build-
audit.json. Original data acceptance controls remain pending; these gates
do not establish full corpus coverage or task completion.

Normal39 checks all five compiler-positive opaque crossword originals with
raw token/CST spelling,safe sorted structure,protected comments,zero parse
errors and byte-exact idempotence. Native --rules all also accepts all five
data originals with status0 and empty streams. Full current108/release/
original/std/compiler/helper pins and five complete empty-reaped controls each
are checked in semantic62-crosswords39-{preservation,native-lint}/root-audit.json.
These are data inputs,not complete-model thesis coverage. Source inspection
found the companion nonmodel read branch still used String::from_utf8; its
byte-path alignment is assigned before fresh full native/companion comparison.

The is count-axis production V2 corrects only the parser-required single
parenthesized generator body. The unchanged faithful positive and referenced
MAX+1 negative pass; all202 main tests/fmt/clippy pass. Full pins,complete
streams and empty reaped groups are checked in semantic62-is-current-gap-
preparation/{focused-green-v2-checked,actual-workspace-gates}/root-audit.json.
Normal40 original is native/companion both report96 warnings,0 errors and130
limitations,with exactly the prior ArrayIndexStart line299 bytes9633..9646
removed and no added diagnostic. ArrayIndexStart is Completed; only Search
remains thesis Limited. All26 partitions,source/drop parity,full current108/
original/std/compiler/release/helper pins and two empty reaped groups are
checked in semantic62-is40-direct-control/root-audit.json. Production and the
faithful regression are committed in1063a8f; final corpus remains pending.

The byte-aware companion passes main fmt/clippy/all202 tests and normal41
opt3 build checks. All five compiler-positive opaque data originals plus four
public BOM/CRLF/opaque-comment/code/literal/directive controls have exact native
status and stderr parity,all26 outcome partitions and source-drop baseline.
Full current108/release/original/std/compiler/helper pins,complete streams from18 controls
and18 empty reaped groups are checked in semantic62-crosswords41-native-
companion/root-audit.json. Data classification remains explicit; this does
not claim complete-model thesis analysis.

The separate scratch-only gbac Search observer completes all seven cases.
GCC wrapper/direct singleton reductions retain the targetless false-assertion
boundary atstd4125..4258; BIN singleton/empty retain the nonempty-boundary at
std945..1159 and Unknown bin-load coverage. Public standard template OR/lb
facts remain Unsupported; the exact core ParameterBool OR tuple is observed
only for the isolated left/reversed expressions. Raw lb has an unavailable
boundary; reversed OR lacks it despite compiler rejection. Their Search
Completed result has no decision variables and is not a safety proof. Full
main/scratch108/std/public/helper pins,complete streams and an empty reaped
group are checked in semantic62-gbac-public-gap-preparation/test-preparation/
root-observation/root-audit.json. Concrete instantiated-body evidence and
faithful producer regressions remain prerequisites for a production repair.

The guarded plain-integer matrix row reservation is falsified on the measured
original: all parse/format allocation calls,requested bytes,retained deltas
and peak deltas are unchanged. It passes25 parsing/24 formatting tests and
all five original preservation checks; every200 fresh native output hashes
match,but original p95 remains125.710/126.122ms above100ms. No performance
improvement or acceptance is claimed,and the candidate is not integrated.
Full main/scratch108/original/std/compiler/helper pins,three opt3 assets,
complete streams,eight runtime and three build/test empty reaped groups are
checked in semantic62-plain-matrix-capacity-experiment/{root-measurements,
root-build-validation}/root-audit.json. Probe baseline differs by10B from
argument storage; every source scope returns to its own exact baseline.
Actual row-shape eligibility is being diagnosed before another candidate;
raw pipe counts are not a substitute for CST evidence.

The concrete gbac producer observer completes all12 public controls. The
left-hand empty-weight guard compiles for singleton and empty arrays; raw and
reversed empty bounds fail, and the direct empty-cover GCC fails its assertion.
All four decision-bearing weight producers retain the bound limitation and
Unknown result coverage. The concrete BIN body resolves a present core
ParameterBool OR and a present ParameterInt lb_array result over an actual
parameter integer array. GCC positive and negative controls have identical
concrete body types and the same abort boundary; type facts alone cannot
justify dropping that boundary. Full main/scratch108/std/public/helper pins,
complete streams and six empty reaped groups are checked in semantic62-gbac-
public-gap-preparation/test-preparation/producer-preparation/{compiler-results,
root-observation}/root-audit.json. No producer repair is integrated yet.

The independent is filtered-search and parameter-local public reductions
compile with their actual data; both MAX+1 controls fail at the intended
filter/domain expression. Normal41 native Search-only analysis reproduces
three traversal boundaries and three parameter-Boolean-let boundaries in the
positive reductions. Negative controls remain Limited rather than receiving
coverage. Native status0 does not establish rule completion. Full source/
std/compiler/public/release/helper pins,complete streams and eight empty
reaped groups are checked in semantic62-is-search-gap-preparation/
{root-compiler,root-native}/root-audit.json. Scoped regressions remain pending.

A separate scratch-only matrix-shape probe explains the ineffective guard:
the original has19 matrices,40141 body rows and no column headers; all rows
start with an Expression child. Raw first tokens are40118 identifiers,3 False
and20 True,so the integer-only guard rejects every row. No guard is broadened
by this observation alone. The probe uses8136B stack counters and makes no
counter allocations; all measured allocation counts/deltas remain unchanged,
output is byte-identical and source drop returns to its own2019B baseline.
Full old/new108 source/input/helper pins,one opt3 probe,complete streams and
two empty reaped groups are checked in semantic62-plain-matrix-row-shape-
preparation/root-runtime/root-audit.json. Timings remain diagnostic only;
final corpus acceptance and the original save-budget repair remain open.

The flat-atom matrix row reservation admits only the observed identifier and
Boolean atoms in addition to integers/commas. Complex expressions retain the
ordinary fallback; grammar, source spelling and CST remain unchanged. Parsing
allocation calls fall851989 to718840 (133149 fewer),requested bytes fall
132481080 to107318648 (25162432 fewer); retained49760248 and peak delta49760880
remain unchanged. Formatting allocations remain unchanged and the original
output is byte-identical. All25 parsing/24 formatting tests and five original
preservation checks pass. Each of200 CLI saves matches the exact output hash.
Original p95 is116.479/116.452ms versus the preceding ineffective experiment's
125.710/126.122ms; it still exceeds100ms. Formatted p95 is129.603/129.489ms for
1189780B (>1MiB,no100ms cap). One first invocation is515.590ms,CPU121.043ms,
RSS57.141MiB; filesystem coldness was not controlled. Library timings and that
single RSS observation are not repeated budget acceptance. Full main/scratch
108/input/helper pins,three opt3 assets,complete streams and eleven empty
reaped runtime/build groups are checked in semantic62-flat-atom-matrix-
capacity-preparation/{root-runtime,root-controls}/root-audit.json. Integrated
main fmt/clippy/all202 tests (42 summaries) pass; full108 sources and three
empty reaped gate groups are checked in actual-workspace-gates/root-audit.json.
Final semantic corpus and the original save-budget repair remain open.

The faithful filtered-search V2 regression fails only after its all-call
Resolved preflight,at the Completed assertion with the same three traversal
boundaries as the public native reduction. V1's missing-globals setup was
caught before launching a child; its failure is not language RED. Separate
producer regressions fail at the first valid left-hand guarded bound and the
first enforced Boolean assertion's missing output guarantee. Full pins,
complete streams and three empty reaped groups are checked in semantic62-is-
filtered-search-preparation/v2/root-red/root-audit.json and semantic62-gbac-
public-gap-preparation/test-preparation/repair-preparation/root-red/root-audit.json.
Scoped production candidates are pending; these failures do not close Search.

The ordered parameter-OR reflection proof and direct enforced-Boolean assertion
output contract are integrated. The proof requires the exact present core
parameter Boolean tuple and a same-array zero-length guard to the left of the
inspected bound. Unknown-condition output forwarding requires one checked
core equality occupying the whole third body,with no body unavailable cause;
every third body remains inspected. Nested callable bodies remain withheld.
Both valid guarded weights now derive the result; raw/reversed bounds,
wrong-array/ambiguous-OR guards and partial bodies retain Unknown/Limited.
One-sided outer branches supply no output,and literal false abort remains.
Three focused V2 tests and main fmt/clippy/all204 tests (42 summaries) pass.
Full scratch/main108 pins,complete streams and six empty reaped groups are
checked in semantic62-gbac-public-gap-preparation/test-preparation/
repair-preparation/{focused-green-v2,actual-workspace-gates-v2}/root-audit.json.
V1 incorrectly expected a user-resolved OR where actual selection was Ambiguous;
its failing fixture is preserved,and V2 checks the user candidate and ambiguity
explicitly. The workspace runner's first wrapper-path preflight failed before
any child; only the corrected V2 gates establish validation. A fresh normal42
release is building for actual standard/native controls. No original BIN load
or GCC completion,full corpus acceptance or task completion is claimed.

The CST diagnostic counts358996 nodes,318546 with one child and40450 with
multiple children. Retained1077685 child slots occupy17242960B. Shape-derived
owning boxes are717992 versus718840 measured parse allocation calls;637088
boxes belong to318544 plain atoms. These are storage-derived lower bounds,
not per-site allocation measurements. Scalar atoms already reserve one slot,
so there is no leaf-vector growth/shrink waste. Parsed-source teardown in this
instrumented,warmed traversal is13.976ms,releasing50669136B to its own2024B
baseline with no new allocations. Output/parse/format counts/deltas remain
unchanged. Full main/scratch108/input/helper pins,one opt3 probe,complete streams
and two empty reaped groups are checked in semantic62-matrix-cst-drop-
attribution-preparation/root-runtime/root-audit.json. A representation candidate
requires separate measurements; these timings are not CLI budget acceptance.

The guarded filtered-search falsifier compiles with actual data and Gecode;
both a direct conditional filter and a named conditional integer alias retain
the two baseline traversal limitations. The unused MAX+1 branches are valid.
An unconditional transitive closed-arithmetic scan would therefore be wrong;
the prepared traversal repair must preserve lazy contexts and keep real eager
overflow vetoes. Full source/std/compiler/model/data/native/helper pins,emitted
assets,complete streams and two empty reaped groups are checked in semantic62-is-
filtered-search-production-preparation/guarded-root-control/root-audit.json.
This remains an explicit support gap,not a new closed-overflow error claim.

Normal42 fresh opt3 tools retain all108 main/copied sources and22 libraries;
four assets,complete build streams and an empty reaped build group are checked
in semantic62-combined42-release/root-build-audit.json. Original gbac with its
actual adjacent data compiles to Gecode FlatZinc without solving. Native and
companion both report16 warnings,0 errors and18 limitations,all26 partitions
and source-drop parity. Exactly the BIN945..1159 nonempty-boundary disappears;
it is replaced by the deeper fzn_bin_packing_load10:36 bytes308..370 exact
traversal-membership limitation. All other diagnostic lines are unchanged,
including GCC's false-assertion boundary. Thus the guard repair advances the
original but Search remains Limited. Full source/original/data/std/compiler/
release/helper pins,emitted compiler assets,complete streams and three empty
reaped groups are checked in semantic62-gbac42-direct-control/root-audit.json.
The deeper membership and guarded nested-output contract remain open.

Filtered search traversals now inspect initialized parameter expressions with
their actual lexical generator headers. The faithful Eligible/Removed and
cardinality filters pass without claiming whole-array coverage; eager MAX+1
still produces a limitation. Conditional and short-circuit arithmetic remain
outside this new admission path, preserving the guarded compiler-positive
examples as explicit support gaps.

The combined focused search check passes. Formatting, clippy with warnings
denied, and all 204 workspace tests pass against the current 108 source files.
Evidence is retained in semantic62-is-filtered-search-production-preparation:
focused-green-combined and actual-workspace-gates/root-audit.json. This is an
implementation checkpoint; original-corpus revalidation and final task
verification remain open.

Plain identifier, integer and Boolean atoms now retain their single token inline
in the private CST child representation. Calls, inverse heads and trivia retain
the ordinary path; both paths use the same access-suffix parser. Public children,
debug output, source spelling and ranges keep their existing shape.

On the 908,888-byte crossword input, parsing allocation calls fall from 718,840
to 400,296 and retained storage falls by 2,224,736 bytes. Five original inputs
pass token/tree coverage, protected-byte, spelling and idempotence checks. All
200 fresh CLI save outputs match the previous formatted bytes. Original-input
p95 is 103.000/103.733 ms with default/nested EditorConfig, versus the preceding
116.479/116.452 ms. The 100 ms target remains open. The 1,189,780-byte formatted
input takes 117.113/117.432 ms p95; it is above the size band for that target.
First invocation takes 403.181 ms and peaks at 52.078 MiB; filesystem coldness
was not controlled. Evidence is in semantic62-single-child-cst-prototype-
preparation/root-runtime and root-controls/root-audit.json. The old inferred
owning-box count does not apply to the changed representation.

Formatting, clippy with warnings denied and all 204 workspace tests pass on
the integrated syntax change; actual-workspace-gates/root-audit.json retains
the checked source snapshot and complete streams. Final corpus acceptance and
the remaining save-budget follow-up remain open.

Normal43 opt3 tools include both the filtered-search repair and inline atoms.
Fresh original `is` native and companion runs agree on all 26 rule partitions:
96 warnings, zero errors and 127 limitations. Exactly the three traversal
limitations at original bytes 15821..15850, 16171..16200 and 16509..16538
disappear relative to normal40; no diagnostic line is added. Search remains
the only Limited thesis rule. Complete sources, streams, release assets and
native/companion parity are checked in semantic62-is43-direct-control/
root-audit.json; the existing original Gecode compile-only precheck is retained.

Both guarded unused-MAX+1 examples still compile with Gecode and retain traversal
support limitations on normal43. Their selected ranges now cover the initializer
expressions; no arithmetic-invalid claim is introduced. Full input/compiler/std/
source/native pins, streams and generated assets are checked in semantic62-is-
filtered-search-production-preparation/guarded43-root-control/root-audit.json.

The existing ordinary changed/formatted, invalid edited, nested and grown-nested
save cases retain exact output/status parity across 1,000 baseline/candidate
samples (50 per case and settings). Candidate p95 spans 5.09..27.88 ms, below
the 50 ms small-input limit. Nested growth keeps the existing output bytes;
the grown case's separate native wait4 RSS rises from 3.641 to 3.797 MiB.
The private node header grows by eight bytes, so the allocation reduction is
not universal. Ordinary/invalid/nested RSS remains below 4 MiB in these controls.
The driver's `/usr/bin/time` RSS is unavailable because its clockrate sysctl is
denied; this is not a formatter error. Separate bounded per-child wait4 captures
retain the usable memory measurement. Full streams, source pins, sample arithmetic
and output parity are checked in semantic62-inline-cst-small-save-control/runtime/
root-audit.json and rss-controls/root-audit.json. The slow crossword still needs
the separate 100 ms budget follow-up.

The compiler-positive boundary-set union now has a focused public regression
that fails on the actual unsupported union-source limitation at bytes 824..840,
while retaining a same-axis whole-array control. This is a private RED candidate,
not an accepted repair. Its current108 snapshot, full streams and expected failure
are checked in semantic62-macc34-union-comprehension-test-preparation/
root-red-runtime/root-audit.json. The local-initializer reduction still has no
reproduced current UnboundedVariable failure and does not claim an original fix.

Fresh normal43 original `macc` native and companion runs retain 115 warnings,
zero errors and 106 limitations with all 26 partitions and exact diagnostic
parity. ConstantVariable, UnboundedVariable and Search remain Limited among
the thesis rules. The original six next_pos/block_pos membership limitations
still exist, even though the simpler public local-initializer reduction does
not reproduce them. Complete current source/release/original/compiler pins and
streams are checked in semantic62-macc43-direct-control/root-audit.json.

Nine public weighted-load controls have actual Gecode compile-only outcomes in
semantic62-gbac-fzn-membership-preparation/compiler-results/root-audit.json.
Seven compile successfully; unaligned empty-weight and wrong-array assertion
controls warn that an undefined access becomes false in Boolean context and
detect inconsistency. They remain partiality negatives, not membership proofs.
The explicit alignment-abort control fails its assertion. The shifted-index
control fails its array declaration before reaching the call, so it does not
yet establish a shifted-selector expectation. All nine captures, complete
streams and fourteen generated assets remain accounted for; no solve ran.

Normal43 formats the complete crossword_opt model and all five compiler-positive
raw-comment data instances into a temporary tree. Each formatted pair compiles
with the same Gecode backend without solving. Raw FlatZinc bytes differ only
in physical line 3, the compiler's `% Command line invocation:` comment containing
the staged input/output paths; every other byte is equal. The six formatted
sources, ten compiler assets, complete original/current streams and eleven
empty reaped groups are checked in semantic62-crossword43-formatted-compiler-
control/runtime/root-audit.json. Installed standard includes remain the same.

The three row-local Boolean relation reduction compiles cleanly with its explicit
data. Its separately retained MAX+1 table-domain variation fails with integer
overflow in the triples declaration, before any relation-body guess. Complete
source/compiler/std/helper pins and both outcomes are checked in semantic62-is-
search-gap-preparation/normal43-next/root-compiler/root-audit.json. These establish
compiler expectations for the next focused regression; no Search repair is
claimed yet.

The weighted-load observation reaches the corrected shifted initializer using
array1d(2..2, [1]); that model compiles successfully. Ten public/concrete producer
captures and matching native/companion Search runs all retain one membership
limitation. Aligned direct and nested positives, actual BIN positives, partiality
negatives and compiler-invalid rows remain distinguished; no output guarantee
is inferred from matching array element/index types. Two existing producer-guard
tests pass unchanged. All 24 bounded commands, source/input/release/compiler pins,
full streams, exact selected one-rule partitions and source-drop checks are
retained in semantic62-gbac-fzn-membership-preparation/observation-preparation/
root-observation/root-audit.json and coverage-summary.json. A scoped successful
assertion relation and nested-output propagation remain separate repair concerns.

Diagnostic attribution on normal43 identifies a second lexical scan in raw-byte
comment restoration: 11.949 ms scanning the 1,189,780-byte formatted output,
within a measured 51.983 ms formatting phase. Exact output, parse/format allocation
counts and source-drop baseline remain unchanged. Full source/helper/build/probe
streams and the opt3 asset are checked in semantic62-crossword43-second-scan-
attribution-preparation/root-runtime/root-audit.json. This is a local diagnostic
timer, not a new native p95 result. Reusing ranges when layout leaves their
coordinates unchanged is the next candidate; transformed-layout fallback and
the 100 ms budget still require verification.


The formatter now reuses its independently scanned comment ranges for raw-byte
restoration only when layout returns the unchanged LF buffer. Layout rewriting
and other line endings retain final-output scanning, and existing comment
kind/text/order checks remain. The private named result passed focused clippy
and all 24 formatting tests after an earlier tuple-result clippy failure.

The candidate preserves all five original crossword data files through parsing,
formatting, reparsing and idempotence. Across 200 fresh CLI saves, every output
matches the retained formatted bytes. The 908,888-byte original now has p95
91.863 ms/default and 92.005 ms/nested EditorConfig, below the 100 ms budget;
normal43 previously measured 103.000/103.733 ms. The 1,189,780-byte formatted
buffer measures 105.765/106.516 ms and remains outside the up-to-1-MiB budget.
The separate first invocation costs 480.587 ms wall, 88.071 ms CPU and 52.281 MiB
RSS; filesystem coldness was not controlled. Wall is recorded before process
cleanup, so its larger delay is not attributed to cleanup sleep. It remains a
separate first-use observation, not the repeated-save percentile.

The candidate also retains output/status behavior on 1,000 old43/candidate small
control samples. Candidate p95 ranges from 4.889 to 27.319 ms, including invalid
edited buffers; diagnostic messages match after substituting only each recorded
temporary input path. All five formatted model/data pairs compile with Gecode
without solving. Each FlatZinc difference is confined to physical line 3, the
command-invocation path comment; all other bytes are equal. Full pinned streams,
allocation/drop checks, release assets and root audits live under semantic62-
opaque-comment-range-reuse-preparation/v2/{root-runtime-v2,root-controls,
small-save-runtime,formatted-compiler-runtime}. Whole-task acceptance remains open.

The row-local Search test V1 stopped in its range preflight and is not behavioral
RED evidence. V2 uses retained CST ranges and passes all type/core tuple checks;
it then fails the three intended value-let limitations at 426..655,783..1044 and
1143..1325. Its following negative checks are not reached in RED. Complete streams
and source pins are audited in semantic62-is-search-gap-preparation/normal43-next/
tests-only-v2/root-red/root-audit.json. A narrow production repair remains pending.

The integrated three-file formatter change passes main workspace formatting,
clippy with warnings denied and all 204 workspace tests. Full receipts are
audited in the same V2 actual-workspace-gates/root-audit.json.


The boundary union-comprehension repair now passes its exact previously failing
public regression, all six consumer suites and all 205 combined workspace tests.
It defers only the exact present typed parameter-integer comprehension marker
until complete callable, instantiation and domain facts exist. The coverage
consumer checks every expected axis and actual initialized source before
comparison; it retains named identities and converts inspected membership to
Unknown, without fabricating members or cardinality. The separate closed-error
check follows collection aliases only on this new path. Existing non-bare domain
handling and legacy scalar/Boolean/lazy inspection remain unchanged. Independent
static review found no substantive correctness issue within the bounded patch.

A fresh original macc native/companion all26 comparison removes exactly one
constant-variable unsupported union limitation at bytes3054..3074 and adds no
diagnostics. Warnings remain115, errors0, limitations106->105. ConstantVariable,
UnboundedVariable and Search remain Limited; separate extrema and six original
array-membership gaps are still required work. The fresh release has four opt3
assets,22 libraries and108 copied sources. Full frozen tests, combined gates,
release/source audits and original diagnostic/parity/drop checks are retained
under semantic62-macc34-union-comprehension-repair-preparation and semantic62-
macc-union-candidate-direct-control/root-audit.json. This increment does not
complete task base-083 or substitute for the final fresh expanded corpus.

Weighted-load tests-only V1 fails compilation because five checks call private
TypeInst::known from an integration test. All three stored commands exit101
before test semantics, including both existing guard tests. The input/fullstream
and empty-group audit is retained in semantic62-gbac-fzn-membership-preparation/
contract-red-preparation/root-red/root-audit.json. This is not behavioral RED;
production is unchanged and a public-type-check correction is pending.

The integrated union repair also passes actual main workspace fmt, clippy with
warnings denied and all205 tests; source sets exactly match the audited opt3
combined release. Receipts are in the repair preparation actual-workspace-gates.


Fresh current public all26 native/companion controls distinguish three MACC
reductions: boundary-union has7 warnings/0errors/1IndexSetMismatch limitation;
extremum-headers has1warning/0errors/8limitations across ConstantVariable,
ExpensiveComprehension, IndexSetMismatch and VacuousConstraint; local-subset-
initializer has4warnings/0errors/0limitations with every selected rule Completed.
All native/companion diagnostics agree and scopes return to baseline. Exact
compiler-positive inputs remain unchanged. Reports are summarized in semantic62-
macc34-constant-domain-proposal/current-all26-public-summary.json. An initial
runner's nonexistent local-subset.mzn path error is retained; only the correctly
named local-subset-initializer case was subsequently run, without repeating the
first two successful cases.

Weighted-load tests-only V2 now compiles and passes the exact core/present tuple
preflight, then fails the valid direct root WholeArray assertion with the actual
membership boundary415..475. Both unchanged producer guard tests pass. Later
negative rows have not run in RED. Full1585 input pins, streams and three empty
reaped groups are audited in semantic62-gbac-fzn-membership-preparation/
contract-red-v2-preparation/root-red/root-audit.json. Production remains private.

The row-local Search production candidate passes the full focused test including
its overflow negative, but full workspace validation catches a real regression:
nested_unknown=1*(not symbolic[1]) changes from Unsupported to Unknown in the
numeric-facts test. The global Boolean-operation admission is too broad. It is
not integrated; the next candidate must restrict this inspection to the checked
non-defining row relation and restore generic numeric/raw-partiality behavior.
The focused success and failed workspace streams/source pins are separately
retained under semantic62-is-search-gap-preparation/normal43-next/production-
preparation/{root-focused-green,root-full-gates}/root-audit.json.

The IndexSetMismatch-specific boundary-union test V1 fails compilation at two
private known() calls, before semantic preflight or behavior. This is not RED
proof; public exact Int/SetInt type checks are being corrected in a new snapshot.
The failure is preserved in semantic62-boundary-union-index-set-preparation/
root-red/root-audit.json. No public API or main source change is justified by it.


Row-local Search V2 restores the generic Boolean direct-safety branch exactly and
confines relation selection inspection to the fully checked row-let body. The
old numeric raw-partiality regression and the full row test (including its closed
error negative) both pass. All205 private combined workspace tests also pass.
The owning inspection returns Unknown without exported outputs, local IDs,
whole-array facts or search seeds; strict dependencies and iteration remain.

Its fresh original is native/companion all26 runs retain96warnings/0errors and
reduce limitations127->124. Exactly three value-let limitations disappear at
10928..11095,11111..11330 and11505..11671; no diagnostic lines are added. Search
remains Limited for other original gaps. Release4opt3 assets/22libraries/108source
pins, focused/full tests and complete diagnostic/scope-drop audits are retained
under semantic62-is-search-gap-preparation/normal43-next/production-preparation-v2,
semantic62-row-search-v2-release and semantic62-is-row-search-v2-direct-control.
The formatter release bytes are identical to the accepted92ms implementation.

Boundary-union IndexSetMismatch V2 passes compilation and core/declaration
preflight but fails its expression-range lookup, so it is not behavioral RED.
V3 uses actual owned CST ranges and passes every preflight, then fails the exact
selected-rule completion assertion with its located union-source limitation at
824..836. The subsequent overflow negative has not run in RED. Both failure
classifications, full streams and input pins remain in the respective v2/v3
root captures. A bounded Guarded domain-source repair is now private work.

The integrated row Search V2 passes actual main fmt, clippy with warnings denied
and all205 workspace tests; source pins exactly match the audited opt3 release.
The actual-workspace-gates/root-audit.json preserves the required command evidence.

The direct asserted-array contract passes its new public regression and both
existing assertion guards in the frozen standalone candidate. Composing its
owned source and test hunks with current row Search V2 also passes all four
focused checks, workspace fmt, clippy with warnings denied and all206 tests.
The caller must prove aligned written axes before forwarding the direct output;
aborting, shifted, computed and nested cases keep their boundaries. These are
private candidate checks, not original GBAC coverage or task completion. Full
streams, unchanged inputs and terminal groups are audited under semantic62-gbac-
fzn-membership-preparation/{contract-production-preparation/focused-green,
contract-combined-preparation/combined-gates}/root-audit.json.

The symbolic set extrema test passes its actual unary core/type/declaration
preflights, then reproduces eight limitations across four selected analyses.
The completion assertion fails before later numeric/iteration and empty-set or
overflow checks. This genuine behavioral RED is retained under semantic62-macc34-
extrema-test-preparation/root-red/runtime/root-audit.json; a bounded repair is
private work, with no main implementation claim yet.

Named initialized union generators now reuse their fully inspected call-aware
domain row and retain named Unknown membership. The exact deferred marker,
present parameter integer-set type and top-level declaration identity gate this
route; other sources keep the previous interpretation. The focused boundary
test and overflow negative, existing guarded/index/iteration consumers, combined
workspace gates and actual main gates pass (207 tests in the combined source).

Fresh Gecode compile-only and native/companion all26 controls make boundary-union
fully Completed with its same seven warnings. Original MACC retains115warnings
and no errors, with limitations105->101: exactly four IndexSetMismatch union-source
limitations disappear at3054..3069,6993..7012,7023..7050 and9022..9047, with no new
diagnostic lines. Other original gaps remain. The current combined opt3 release,
four assets/22libraries/108sources,42 controls, complete per-rule partitions,
scope drops and unchanged original/std inputs are independently audited under
semantic62-weighted-boundary-release-control-preparation. Actual main source and
gates are retained under semantic62-weighted-boundary-main-integration.

The direct asserted-array contract is now integrated after the same audited
207-test main gates and fresh controls. Only the callable's entire direct
assert/forall body can use the selected core index-set equality for dependency
selectors; whole-array traversal keeps its existing proof. Caller output transfer
requires checked bare actual/default identities, compatible literal initializer
axes and same_members==Some(true) for the two written axes. Unproved, unequal,
computed or converted actuals withhold outputs. All original guards remain.

The direct singleton control now completes Search and all14 thesis analyses;
VacuousConstraint still has two separate limitations. Invalid assertion/shifted
and wrong-array partiality controls and valid nested/BIN controls retain explicit
Search limits. Original GBAC's16warnings/0errors/18limitations and complete
diagnostic stream are unchanged. Compiler rejection is kept distinct from
successful capture, and no solver was run. The formatter asset is byte-identical
to the accepted92ms release. Wider nested-conversion/GCC repairs, symbolic extrema
and other corpus gaps still prevent base-083 completion.

Symbolic parameter integer-set extrema now use full initialized source inspection
in late range queries. Only selected core min/max calls with present parameter
integer-set arguments enter this route. Endpoints, arithmetic, aliases and
independent expected axes retain closed-error checks before unknown membership
can end comparison. The result supplies no extremum value, cardinality, membership
or whole-array coverage. Raw domain queries keep their existing interpretation.

The public boundary-header regression reproduces the old eight limitations and
passes after the repair, including empty-set, transitive overflow and independent
axis overflow guards. Actual main fmt, clippy with warnings denied and all208
workspace tests pass; all108 source paths match the frozen candidate. Evidence is
under semantic62-extrema-main-integration/actual-workspace-gates/root-audit.json.

Fresh opt3 Gecode compile-only and native/companion all26 controls make the public
extremum model fully Completed with its one warning unchanged. Original MACC has
115warnings/0errors/77limitations, down from101:30 old limitation lines disappear
and six remaining-reason lines appear, for a net24 reduction. Four opaque index
offsets and two bounded-integer source gaps remain explicit. ConstantVariable
completes; UnboundedVariable and SearchCoverage remain Limited among thesis rules.
Full streams, assets, input pins and per-rule partitions are checked under
semantic62-extrema-release-control-preparation/{release-results/root-build-audit.json,
control-results/root-audit.json}. No solver was run. This is a useful source
checkpoint; remaining original-model gaps and final corpus acceptance still
prevent base-083 completion.

Two remaining support gaps now have faithful public compiler controls. The IS
subset reduction retains the scalar sum/card/bool2int relation over a named
axis minus a parameter set. Gecode accepts it. After checking the actual mixed
decision/parameter equality tuple and binder/axis identities, the focused test
reaches the missing traversal-membership limitation. The numeric guard passes.
Evidence is under semantic62-is-diff-subset-selection-preparation/v2/root-runner/output.

The four real-library GCC controls use the installed four-argument deprecated
wrapper and complete standard bodies. Gecode accepts symbolic and singleton
models; both deliberately invalid empty-cover models abort at the real standard
assertion. All four observation preflights preserve the unrelated unsearched
auxiliary. The symbolic positive reaches the false-assert source limitation;
the prior scratch build error is retained separately. Historical a2b78 native
and companion all26/thesis14 records agree for every control, with checked drop
snapshots. Evidence is under semantic62-gcc-real-body-controls-preparation-v2/root-capture.
These are diagnosed gaps, not repairs or current-main acceptance. No solver was
run. The next source changes and final corpus checks remain required.


A whole owning assert can now inspect one selected nested weighted-load predicate
without changing that child's unconditional summary. The bounded path checks the
complete one-binder forall equality and weighted sum, maps only distinct owning
formals, and uses the checked index-set equality for that invocation's weight
selector. Caller output transfer still requires equal checked actual axes;
unknown, mismatched and computed actuals retain their limitations.

The independent same-callee mismatch counter and prior assertion/BIN guards pass.
Actual main formatting, Clippy with warnings denied and all208 workspace tests
pass; all108 source paths match the gated opt3 candidate. Fresh Gecode compile-only
and native/companion all26 controls complete Search and all14 thesis analyses in
the bare nested singleton, with one warning and two separate VacuousConstraint
limits. Conversion and BIN controls remain Limited. Original GBAC remains exactly
16warnings/0errors/18limitations, with byte-identical diagnostics and partitions.
Evidence is under semantic62-nested-weighted-main-integration/actual-workspace-gates
and semantic62-nested-weighted-release-control-preparation/{release-results,
control-results}/root-*.json. No solver was run. Conversion, computed-row and GCC
support and the final corpus checks remain required for base-083.

Core forall relations can now inspect complete Boolean conditionals with
parameter guards and set-valued headers. All branches retain source inspection;
the traversal grants neither a value nor an output guarantee. Closed integer
errors in headers and filters remain explicit, including the two overflow
counters in the focused public test.

Actual main formatting, Clippy with warnings denied and all209 workspace tests
pass. The public control completes all14 thesis analyses. Original IS retains
96warnings/0errors and now has123limitations, down from124: only the verified
Boolean selection index-membership limitation disappears. Warning lines are
byte-identical; Search remains Limited. Fresh opt3 native/companion diagnostics
and all26 partitions agree, using the retained Gecode compile-only proof.
Evidence is under semantic62-is-conditional-main-integration/actual-workspace-gates
and semantic62-is-conditional-release-control-preparation/{release-results,
control-results}/root-*.json. Remaining original-model gaps and the final corpus
checks still prevent base-083 completion.

Model-local scalar selections can now inspect a two-axis integer decision array
with a parameter first selector and decision second selector, or an enum-valued
array with parameter selectors. The result remains unknown. Source domains,
both selectors and annotations are checked before accepting that uncertainty;
no membership, definition or whole-array output is granted.

The focused original-shaped model and closed-selector overflow counter pass.
Actual main formatting, Clippy with warnings denied and all210 workspace tests
pass, retaining the IS and nested-call repairs. Fresh opt3 native/companion all26
controls match the separately verified Macc repair exactly: the public model has
12warnings/0errors/0limitations and all26 analyses complete; original Macc has
115warnings/0errors/53limitations, down from77 before the repair. The24 removed
limitation lines have no replacements or added warnings. UnboundedVariable now
completes; SearchCoverage retains the remaining reshape limitations.
Evidence is under semantic62-macc-local-selection-main-integration/actual-workspace-gates
and semantic62-macc-local-selection-release-control-preparation-v4/{release-results,
control-results}/root-*.json. The unchanged Gecode compile-only proofs were reused;
no solver was run. Remaining support gaps and final corpus acceptance still
prevent base-083 completion.

Scalar selections over a checked parameter-set difference now retain membership
in its exact left axis. Both written set sources, selected core signature,
annotations and closed errors are checked. This proves one selector's membership;
it supplies no array definition or whole-array coverage.

Actual main formatting, Clippy with warnings denied and all211 workspace tests
pass. Matched current-build native/companion controls report all26 outcomes and
all14 thesis partitions. The focused model completes every thesis rule: its
limitations fall from5 to4, and a newly visible warning correctly identifies the
unsearched decision array. Original IS retains the same96 warning lines and
0errors; limitations fall from123 to121. Boolean selections and the objective's
bounded integer interpretation still require support. Existing Gecode compile-only
proofs were reused; no solver ran.
Evidence is under semantic62-is-diff-subset-main-integration/actual-workspace-gates,
semantic62-is-diff-subset-release-control-preparation/control-results and
semantic62-is-diff-subset-current-original-before-preparation/output.
Remaining original-model gaps and final corpus checks keep base-083 open.

Nested asserted weighted equalities can retain their owning array identities
through exact type-preserving core index2int calls, including enum2int on the
already-Int bin argument. Every written conversion and source is inspected;
the existing formal ownership, complete body and actual-axis checks still apply.

The candidate passes all210 existing tests and focused mismatch, unknown-axis,
captured-source and abort guards. Its eight matched current-build controls retain
all26 partitions, fourteen thesis outcomes and native/companion agreement.
The converted aligned singleton loses its membership limitation and completes
every thesis rule. False and unproved actual axes remain Limited with explicit
axis causes; the independent unguarded mismatch remains Limited. Original GBAC
is unchanged at16warnings/0errors/18limitations. Compiler-invalid and partial
controls retain their classifications; the compiler proofs were reused without
solving. These candidate captures precede composition with the IS subset repair.
Evidence is under semantic62-already-int-conversion-production-preparation/
{root-results,release-results,control-results}. Full BIN/GCC and computed-row
support, remaining original-model gaps and final corpus checks remain required.
After composition with IS, actual main formatting, Clippy with warnings denied
and all211 workspace tests pass. The combined source and checks are recorded
under semantic62-already-int-conversion-main-integration.

Parameter integer-set arrays now pass the checked array1d reshape and conditional
source inspection. A present core exists inspects its Boolean relation body using
the existing relation checker. These results remain unknown: they grant neither
set membership nor array definitions. Selected types, source annotations,
initializers and closed errors remain checked, including an eager axis overflow.

Fresh matched opt3 native/companion controls complete all fourteen thesis rules
on the public neighbour model and original Macc. Original limitations fall from53
to51: the two reshape limitations disappear, and pickup/delivery correctly receive
uncovered-search warnings, taking warnings from115 to117. All26 outcomes, loaded
sources and drop records agree between native and companion runs. Existing Gecode
compile-only proofs were reused; no solver ran. Four non-thesis analyses remain
Limited. These controls use the private candidate before IS/conversion composition.
Evidence is under semantic62-macc-neighbour-set-reshape-preparation/
release-controls-v2/{release-results,control-results}. After composition, actual
main formatting, Clippy with warnings denied and all212 workspace tests pass;
checks are under semantic62-macc-neighbour-set-main-integration.
Remaining IS, BIN/GCC support and final full-corpus checks keep base-083 open.

Owning parameter-set locals now inspect the complete cardinality/min/max/xor/sum
Boolean conditional without defining an array. Every initializer, enclosing
generator/filter, written type and annotation is checked. An unchanged local
comprehension binder may retain membership in its exact inspected difference's
left axis; cardinality, extrema values and whole-array coverage stay unproved.
The Boolean relation consumer reuses the complete conditional inspection and
checks the selected core xor. Generic numeric safety and member_index are unchanged.

Fresh matched opt3 all26 native/companion controls complete every thesis rule on
the focused model. Original IS retains all96 warning lines and0errors;
limitations fall from121 to117, with exactly four Set-local causes removed and
no added diagnostics. Its two redundant-local boundaries and circuit private
integer-array boundary still keep Search Limited. Existing Gecode compile-only
proofs were reused; no solver ran. These captures precede composition with the
conversion and Macc repairs, whose existential inspection branch is retained.
Evidence is under semantic62-is-set-local-let-preparation/production-v1/
{root-gates/runtime,release-controls}. Composed main formatting, Clippy with
warnings denied and all213 workspace tests pass; checks are under
semantic62-is-set-local-main-integration. Base-083 remains open pending the
remaining support gaps and final complete corpus evidence.

GCC invocation inspection now keeps selected actuals while inspecting standard
bodies and their branches. Reachable false assertions remain mandatory causes;
unreachable branches still receive source checks. The existing strict scalar
output route is retained, and targeted errors also retain a targetless boundary
so already searched results cannot erase an evaluation failure.

Fresh matched normal opt3 controls remove the false assertion limitation from
the compiler-accepted singleton GCC case (three limitations to two). Symbolic
GCC and original GBAC replace that false assertion with a conditional-domain
limitation; their limitation totals and warning lines remain unchanged. Both
compiler-rejected empty controls remain byte-identical. All twenty native and
companion captures agree, with complete all26/thesis14 partitions and matching
drop records. Existing Gecode compile-only proofs were reused; no solver ran.
Evidence is under semantic62-gcc-invocation-release-controls-preparation-v4.
After preserving the accepted IS, conversion and Macc changes, actual main
formatting, Clippy with warnings denied and all214 workspace tests pass under
semantic62-gcc-main-integration. Conditional-domain support, remaining IS and
BIN/Macc gaps and final corpus evidence keep base-083 open.

Ignored Boolean wrappers now inspect the owning two-row forall and complete
initialized row-local scope. Both headers use the same checked table identity;
each source/filter sees its original preceding scope, and all initializers,
written axes, annotations and Boolean relation sources remain checked. This
inspection grants no array outputs or inferred selector membership.

Matched normal opt3 all26 controls complete all fourteen thesis rules on the
public model. Original IS retains its96 warning lines and0errors; limitations
fall from117 to116, removing only wrapper388 at12039..12423 with no added
diagnostics. Native and companion streams, partitions, dependencies and drop
records agree. Existing Gecode compile-only proofs were reused; no solver ran.
Evidence is under semantic62-is-dual-row-wrapper-preparation/production-v1/
release-controls. After preserving GCC and earlier accepted repairs, actual
main formatting, Clippy with warnings denied and all215 workspace tests pass
under semantic62-is-dual-row-main-integration. The nested wrapper403 and circuit
boundaries, remaining BIN/GCC/Macc gaps and final corpus evidence keep base-083 open.

The installed BIN body now retains its ordered weight guard and checks all four
FZN clauses before forwarding the weighted load definition. Targetless sums and
both bound relations receive source and arithmetic checks; the nested contract
keeps the existing invocation boundary and exact caller-axis requirements.

Matched normal opt3 controls remove the full-body search limitation from both
public singleton models, completing all fourteen thesis rules. The partial
negative retains its diagnostics exactly. Original GBAC removes the FZN selector
limitation but reaches the caller's computed-array identity boundary at88;
warnings and limitation totals remain16/18. All sixteen native/companion captures
agree with complete all26 partitions and matching drop records. Existing Gecode
compile-only proofs were reused; no solver ran. Evidence is under
semantic62-full-bin-v2-release-control-preparation. Actual combined main
formatting, Clippy with warnings denied and all216 workspace tests pass under
semantic62-full-bin-main-integration. Computed row support, remaining IS/GCC/Macc
gaps and final complete corpus evidence keep base-083 open.

Offset selectors now retain the original generator source. For present parameter
integer arithmetic over a checked extremum range, an inspected symbolic range
stays Unknown instead of becoming an unsupported opaque offset. This establishes
neither shifted membership nor exact array coverage. Closed arithmetic errors
remain explicit; selectors with unavailable prerequisites keep the existing path.

Matched normal opt3 public and original Macc controls remove exactly four offset
limitations each, with no added diagnostics or changed warnings. The public
reduction retains12 warnings and5 limitations; original Macc retains117 warnings
and47 limitations. All fourteen thesis rules complete before and after, and all
eight native/companion captures agree with complete all26 partitions, unchanged
sources and matching drop records. Existing Gecode compile-only proofs were
reused; no solver ran. Evidence is under
semantic62-macc-offset-index-release-control-preparation. After retaining the
accepted BIN and other repairs, actual main formatting, Clippy with warnings
denied and all217 workspace tests pass under semantic62-macc-offset-main-integration.
Neighbour-source inspection, remaining IS/GCC/BIN gaps and final complete corpus
evidence keep base-083 open.

GCC invocation extent inspection now checks selected parameter-set sources using
the mapped actual's lexical generators. A successfully inspected selection has
unknown extent; it proves no emptiness, cardinality or array membership. An
unsupported or erroneous selected source remains an explicit cause. Existing
literal and written-range extent paths retain their behavior.

Matched normal opt3 controls remove only the standard GCC conditional-domain
limitation from symbolic GCC and original GBAC. Symbolic GCC keeps5 warnings and
4 limitations, with all fourteen thesis rules complete. Original GBAC keeps16
warnings and17 limitations; its separate BIN search gap remains. The singleton
and both compiler-rejected empty controls retain diagnostics byte-for-byte. All
twenty native/companion captures agree with complete all26/thesis14 partitions,
unchanged sources and matching drop records. Existing Gecode compile-only proofs
were reused; no solver ran. Evidence is under
semantic62-gcc-selected-set-extent-release-controls-preparation-v2. Actual main
formatting, Clippy with warnings denied and all217 workspace tests pass under
semantic62-gcc-selected-set-extent-main-integration. Remaining IS/BIN/Macc gaps
and final complete corpus evidence keep base-083 open.

Nested row constraints now inspect the outer table selections and initialized
integer locals before checking both inner row filters and their local body. The
prior local identities remain inspection state; they confer no array outputs or
cell membership. Closed source errors remain explicit.

Matched normal opt3 controls remove the original IS limitation at line403,
bytes12601..13186, and reveal the separate search warning for sel at line125.
Original IS changes from96 warnings/117 limitations to97/116; its circuit search
limitation remains. The public reduction changes from4 warnings/5 limitations to
5/4, with all fourteen thesis rules complete. All eight native/companion captures
agree on full26 outcomes, dependencies and drop records. Existing Gecode
compile-only proofs were reused; no solver ran. Evidence is under
semantic62-is-nested-row-wrapper-preparation/production-v1/release-controls.
Actual combined main formatting, Clippy with warnings denied and all218 workspace
tests pass under semantic62-is-nested-row-main-integration. Remaining circuit,
BIN and Macc support gaps and final complete corpus evidence keep base-083 open.

Selected parameter-set generators now inspect the actual preceding headers,
array source, selectors and initialized collection sources. Inspected selections
retain unknown membership and extent. Only value arithmetic inside a checked
literal-inactive branch can be skipped; source, type, annotation, alias, cycle,
domain and selector errors remain vetoes. Existing strict entry points retain
their behavior.

Matched normal opt3 controls remove 25 limitations from original Macc, changing
47 limitations to 22 while preserving all 117 warnings and completion of all
fourteen thesis rules. The selected and inactive public reductions each lose
three limitations without new diagnostics. The inactive reduction retains a
separate Search division-by-zero limitation, which still requires repair. All
twelve native/companion captures agree on full 26-rule outcomes, dependencies,
source bytes and drop records. Three Gecode compile-only proofs were reused; no
solver ran. Evidence is under
semantic62-macc-neighbours-release-controls-preparation-v1. Actual main
formatting, Clippy with warnings denied and all 219 workspace tests pass under
semantic62-macc-neighbours-main-integration. Remaining support gaps and final
complete corpus evidence keep base-083 open.

Computed weighted BIN rows now inspect their selected item sources and require
exact shared source identity, checked implicit axes and complete output traversal.
Unknown membership, filtered traversal, partial initialization and dependency
cycles retain their existing refusals.

Matched normal opt3 controls remove the bare-array Search limitation from both
the public aligned-row reduction and original GBAC. The public model changes
from eight limitations to seven with no warnings; GBAC changes from seventeen
to sixteen while retaining all sixteen warnings. All fourteen thesis rules
complete in both after captures. No diagnostics are added. Native and companion
captures agree on all 26 rules, dependencies and drop records. Existing Gecode
compile-only proofs were reused; no solver ran. Evidence is under
semantic62-computed-bin-row-release-controls-preparation-v1. The exact owned
hunks preserve the selected Macc repair on main. Actual main formatting, Clippy
with warnings denied and all 220 workspace tests pass under
semantic62-computed-bin-row-main-integration. Remaining circuit and inactive
Macc Search work and final complete corpus evidence keep base-083 open.

The installed MiniZinc source/type precheck now accounts for all 6,417 retained
roots in their original interleaved order: 2,541 models and 3,876 standalone data
files. Model-only checks accept 2,241 sources. The remaining 300 results comprise
270 type rejections, twelve syntax rejections, seventeen missing includes and
one include cycle. Missing includes remain unavailable prerequisites; standalone
data has no invented model pairing. All 2,234 new checks completed without
timeouts, alongside 307 reused checked receipts.

ROOT independently verified all 5,082 complete compiler streams, every new
child's reaped empty process group and the complete root order. All 189,496
original files were rehashed unchanged; the existing dangling-file discovery
error remains explicit. The 276 new nonzero captures have independently checked
raw error headlines and categories. Evidence is under
semantic62-complete-compiler-source-check-preparation/source-types-v1 and
semantic62-compiler-nonzero-review-v1. These model-check-only results establish
source/type processing, not complete-instance validity or FlatZinc compilation.
Paired Gecode compile-only controls, final semantic corpus reconciliation and
the remaining support repairs are still required for base-083 acceptance.

Inactive selected-set branches now use the checked generator source when Search
inspects initialized dependencies. Unknown membership remains unknown, and an
active division by zero still refuses inspection.

Matched normal opt3 controls remove the inactive public reduction's false Search
division-by-zero limitation. Its warnings change from four to five because Search
can now report the uncovered `load` variable; limitations change from two to one.
The selected reduction and original Macc and GBAC retain byte-identical diagnostics
between the fresh before/after captures. All fourteen thesis rules complete in
every after capture. Native and companion agree on full 26-rule outcomes,
dependencies and drop records. Four retained Gecode compile-only proofs were
reused; no solver ran. Evidence is under
semantic62-macc-inactive-search-release-controls-preparation-v1. Final corpus
reconciliation and circuit support remain required for base-083 acceptance.
Actual main formatting, Clippy with warnings denied and all 220 workspace tests
pass under semantic62-macc-inactive-search-main-integration.

Guarded circuit bodies now inspect their private index set, extrema, cardinality,
order array and selectors in the owning scope. This inspection preserves unknown
membership and produces no private output certificates. Selected callable views
also retain default-expression types. Present actuals can reach optional formals
when the existing coercion check allows them; optional actuals remain unsupported.
The focused public circuit regression passes its positive, partial-initializer
and optional-actual cases.

Matched normal opt3 controls remove the installed `all_different` optionality
refusal. Limitations change from thirteen to twelve in the written circuit,
four to three in the installed circuit and 112 to 111 in original IS. Warnings
remain seven, one and 97 respectively. All three Search outcomes remain Limited
at the remaining value-call dependency gap. Native and companion agree on full
26-rule outcomes, dependencies and drop records. Five retained Gecode compile-only
proofs were reused; no solver ran. Evidence is under
semantic62-circuit-optional-formal-release-controls-preparation-v1. Final corpus
reconciliation and the remaining conversion support keep base-083 open.
Actual main formatting, Clippy with warnings denied and all 221 workspace tests
pass under semantic62-circuit-optional-main-integration.

Explicit present parameter integer-set conversions now inspect their original
argument dependencies through the existing traversal. The checked core tuple
provides no converted values, cardinality, extent or output guarantee. The public
regression keeps symbolic sets and a guarded local set inspectable while retaining
the explicit division-by-zero refusal and existing negative guard cases. Transient
type/CST preflight checks remain in private evidence rather than permanent tests.

Matched normal opt3 controls complete Search in the written circuit, installed
circuit and guarded conversion reductions. The installed circuit and guarded
conversion complete all fourteen thesis rules. Original IS removes both circuit
value-call limitations but retains one separate Search membership limitation at
its table constraint. Warnings remain seven, one and 97 in written, installed and
IS controls; limitations change from twelve to ten, three to one and 111 to 109.
The guarded conversion changes from two warnings/three limitations to three/two,
adding the legitimate uncovered `xs` warning. Native and companion agree on full
26-rule outcomes, dependencies and drop records. Six retained Gecode compile-only
proofs were reused; no solver ran. Evidence is under
semantic62-set2array-release-controls-preparation-v1. Its native V2 source differs
from final main only in the simplified permanent test; production bytes match.
Actual main formatting, Clippy with warnings denied and all 221 workspace tests
pass under semantic62-set2array-main-integration. The written circuit's symbolic
index-domain limitation, IS membership and final corpus checks keep base-083 open.

Checked core `index_set` calls on a bare rank-one integer array now recover its
declared axis in the shared domain producer. Named axes and arithmetic errors
remain intact; an unconstrained formal axis remains Unknown. This query does not
inspect array value initializers or establish membership, totality or outputs.
The public regression covers the written circuit's symbolic axis, closed start
advice and retained overflow refusal. The API without callable facts remains
conservative.

Matched normal opt3 controls remove the written circuit's sole thesis limitation:
all fourteen thesis rules now complete there. Warnings remain seven and all-rule
limitations decrease from ten to nine. Installed circuit, original IS and guarded
conversion diagnostics remain unchanged; IS still has its separate Search table
limitation. Native and companion agree on full 26-rule outcomes, dependencies and
drop records across all sixteen runs. Six retained Gecode compile-only proofs were
reused. Evidence is under semantic62-circuit-domain-release-controls-preparation-v1;
the audited release's 108 source files match current main. Actual main formatting,
Clippy with warnings denied and all 222 workspace tests pass under
semantic62-circuit-index-domain-main-integration. IS table inspection and final
corpus, functional and performance checks keep base-083 open.

Plain rank-one integer array literals can now inspect their original cells when
strict membership remains unproved. Inspection returns Unknown rather than a
membership or output guarantee. Closed invalid selectors still refuse inspection.
The existing public control group covers both paths without adding a new test.
Actual main formatting, Clippy with warnings denied and all 222 workspace tests
pass under semantic62-is-table-literal-main-integration.

Matched normal opt3 controls remove the table-literal membership refusal in
original IS and its public reduction. IS retains 97 warnings and 109 limitations:
Search remains Limited at a downstream concatenation path. The public reduction
has no warnings; limitations increase from eight to nine as inspection reaches two
unsupported calls in the installed table wrapper. Written and installed circuit
results remain unchanged, with all fourteen thesis rules complete. Native and
companion agree on full 26-rule outcomes, dependencies and drop records across
sixteen runs. Six retained Gecode compile-only proofs were reused; no solver ran.
Evidence is under semantic62-is-table-release-controls-preparation-v1. The next
checks distinguish IS's parameter array-of-set concatenation from table wrapper
index and conversion calls. These gaps and final corpus, functional and
performance checks keep base-083 open.

Selected core concatenation of parameter integer-set arrays now permits source
inspection. The shared shape check retains core identity, rank, type and coercion
guards. Inspection preserves annotation and closed-error refusals and returns
Unknown for extent and values. Strict dependency production remains limited to
its previous integer-array shapes; no new output guarantees are introduced.
Guarded evaluation also retains Unknown for the new shape.

The existing public control group now checks successful symbolic inspection,
retained division-by-zero refusal and absence of output definitions. Private
candidate formatting, Clippy and all 222 workspace tests pass; existing output
and reshape guards also pass. Evidence is under
semantic62-is-set-concat-production-preparation-v1. Actual main formatting,
Clippy with warnings denied and all 222 workspace tests also pass under
semantic62-is-set-concat-main-integration.

Matched normal native/companion controls retain byte-exact diagnostics and full
26-rule results on all six models. Written and installed circuit still complete
all fourteen thesis rules; original IS, the table reduction and the set-array
reduction with an array1d receiver remain Search Limited. The partial reduction
retains its division-by-zero warning and limitations. Six before pairs and their
compiler proofs were reused; twelve after captures agree on outcomes, dependencies
and drop records. No compiler or solver ran during these captures. Evidence is
under semantic62-is-set-concat-release-controls-preparation-v1. These observations
require further receiver inspection and do not establish final corpus completion.

Checked core rank-two parameter integer index views now inspect a selected owning
formal or initialized top-level source. The already-integer index2int conversion
returns Unknown after source checks. Defaults, optional elements, opaque
initializers and closed errors remain refused; these views establish no membership,
extent or output guarantee. A core two-argument Boolean assertion can inspect
both initialized arguments when strict dependency collection cannot prove them.
Existing mapped and literal-false abort checks continue, and three-argument
assertions retain their strict dependency and output contracts.

One public regression checks symbolic inspection, partial, optional and opaque
refusals, and absence of output definitions. The composed private candidate and
actual main pass formatting, Clippy with warnings denied and all 223 workspace
tests. Evidence is under semantic62-table-index-wrapper-composed-preparation-v1
and semantic62-table-index-wrapper-main-integration. The accepted Gecode
compile-only proof was reused.

Matched normal optimized wrapper controls complete all fourteen thesis rules on the
table reduction: its two standard table Search limitations disappear, and two
public uncovered-value warnings appear (warnings 0 to 2; all-rule limitations 9
to 7). The other five controls retain their entire analyses. Original IS remains
at 97 warnings, 109 limitations and 13 of 14 thesis rules completed. Both set-array
reductions remain at 12 of 14; the partial case retains its divisor warning.

The coordinator checked 1752 input pins, 48 complete streams and all 24 retained control
children, with native/probe parity, all26/thesis14 partitions, source dependencies
and drop records. Evidence is under
semantic62-table-index-wrapper-release-controls-preparation-v1/
{release-results/root-build-audit.json,control-results/root-audit.json}.
The normal release uses the composed source; a final release built from main remains
pending. The receiver diagnosis, base-083 and final corpus acceptance remain open.

The one-axis core array1d receiver now inspects a checked parameter integer-set
array concatenation through the existing inline reshape path. Exact selected
types, annotations, initialized sources and closed-error refusals remain checked;
cardinality stays Unknown. Strict dependency and output handling are unchanged.
The existing public group adds symbolic and division-by-zero receiver cases with
no output guarantees. The positive regression failed on the prior source after
its selected receiver tuple was checked, then passed with the repair; the negative
retains its refusal. Private typed preflight checks were removed from the patch.

The focused private check and actual-main formatting, Clippy with warnings denied
and all 223 workspace tests pass; existing test names are unchanged. Evidence is
under semantic62-is-set-concat-reshape-{red,production}-preparation-v1 and
semantic62-is-set-concat-reshape-main-integration. Independent review found no
local defect in the guard. Matched native receiver controls, final corpus and
functional/performance checks remain pending; base-083 remains open.

Matched normal controls after the array1d receiver repair complete all fourteen
thesis rules on original IS: warnings stay at 97 and limitations fall from 109
to 107. The positive set-array reduction also reaches fourteen, with limitations
falling from four to one. Circuit and table controls are unchanged. The partial
set-array control retains its divisor warning and four limitations; two causes
now identify the closed division error instead of unsupported concatenation.

The normal release was built from actual main and audited across 108 source
files, four executables, 22 libraries and 18 Cargo events. All 2049 matched-control
input pins and 48 complete streams were checked, with exact native/companion
outcomes, dependencies and drop records. Evidence is under
semantic62-final-actual-main-release-preparation-v2/release-results and
semantic62-final-six-controls-b691/control-results.

The final public controls pass all 30 captures: exact default/thesis/all selections,
shared-include native/library parity, multi-root diagnostic order, omitted-default
equivalence, and source preservation plus two-pass stdin saves for BOM, CRLF,
opaque comment bytes and protected spans. All 165 inputs and 60 complete streams
were checked. Evidence is under
semantic62-final-public-control-preparation/b691-public-controls. Fresh full-corpus
and isolated performance checks remain required; base-083 stays open.

The fresh parallel corpus capture has eight completed batches. Independent
capture review of batches 4–8 checks 320 ordered roots per selection, matching
drop and completion records, exact fourteen/thesis and twenty-six/all rule
partitions, and native/companion diagnostic parity. This proves capture integrity,
not compiler validity or semantic acceptance. The remaining sweep is running.

The exact local ZipQueens model with its existing small data file now compiles
with MiniZinc 2.10.1 and Gecode without solving. It returns zero with empty
diagnostic streams and produces 3049 bytes of FlatZinc and 853 bytes of OZN;
all 1055 runtime input pins remain unchanged. A separate private fact check
confirms matching named-axis identities for a rank-two enum-valued array and its
nested decision selector. The selector's dependencies are supported, while the
outer access lacks an implemented membership proof. This is a confirmed support
gap, not an invalid-instance exclusion; no production repair is claimed yet.
Evidence is under semantic62-zip-queens-{instance-compiler,membership-preflight}-
preparation-v1. Both checks overlap the parallel corpus and make no isolated
timing claim. Private sources and diagnostic tests remain outside the repository.

One standalone assignment data file has a captured parse error and NotRun rule
outcomes. A fresh Gecode compile-only check with the existing project model
rejects that exact data for mixing indexed and non-indexed array entries, matching
Zincite's refusal. This checks the original file without repairing or inventing
data; all 1054 input pins and both complete streams were verified, with the child
reaped and its group empty. Evidence is under
semantic62-assignment-error-data-compiler-check-v1. The rejection excludes only
that exact checked input; other unpaired data remains unclassified by this check.

The normal formatter's exact ZipQueens output also compiles with the same small
data file and Gecode without solving. The OZN output is byte-identical. Full
FlatZinc differs in one command-line comment recording the changed file paths;
every other byte agrees. The library preservation checker passes spelling,
structure, protected-byte, reparse and idempotence checks on the original.
ROOT verified all three owned children, six complete streams and 1205 unchanged
runtime pins. Evidence is under semantic62-zip-queens-format-compile-preparation-v1.
This validates the checked pair, not all corpus inputs or semantic rule coverage.

The private social-golfers cardinality repair now has a genuine behavioral RED
on unchanged production code and a passing focused GREEN. The public reduction
completes search analysis while retaining Unknown values and no output or
whole-array definition certificate; closed selector and member-domain overflow
remain Limited. The existing numeric partiality guard also passes. ROOT checked
1188 unchanged inputs, all four complete streams and both reaped empty groups
for each stage. Evidence is under semantic62-social-golfers-card-repair-
preparation-v1/v2. This repair is not integrated; actual-main gates and original
model outcomes remain pending. The parallel corpus continues on unchanged main.

A fixed completed prefix covers 935 roots per selection. ROOT independently
recomputed the compiler-source matching and rule counts from thirty complete
companion streams: 562 roots are source-check accepted, complete and dependency
resolved; 177 complete all fourteen thesis analyses and 385 retain at least one
Limited thesis outcome. The thesis outcomes agree between thesis and all captures.
These are source-check results, not supplied-instance compile proofs or whole
corpus acceptance. Evidence is under semantic62-b691-prefix-compiler-reconciliation-v1.

Review of the private named-selector repair exposed a closed-domain error bypass.
A focused reproduction retains the Unsupported initializer for a shared index
set containing division by zero, but still emits a Supported whole-array output
definition. ROOT verified the failing child, both full streams and 229 unchanged
inputs. The repair is not accepted or integrated; a correction is in preparation.
Reproduction evidence is under semantic62-zip-queens-axis-error-reproduction-preparation-v1.

The private named-selector domain veto passes the five-case group and existing
cycle/alias guard. A second focused reproduction nevertheless confirms that an
annotated lookup-source declaration can still produce a Supported output. The
third private candidate restores the initialized-source veto and passes all six
cases plus the cycle/alias guard. ROOT verified 240 unchanged input pins, four
complete streams and two reaped empty groups; independent local source review
has no unresolved finding. Both failing reproductions remain preserved. Evidence
is under semantic62-zip-queens-named-selector-repair-preparation-v3; original-model
controls, actual-main gates and integration remain pending.

The Challenge nonogram model with its existing non_fast_4 data also compiles
with MiniZinc 2.10.1 and Gecode without solving. ROOT verified empty diagnostics,
231100 bytes of FlatZinc, 320 bytes of OZN and 1039 unchanged input pins, with the
child reaped and its group empty. Evidence is under
semantic62-nonogram-instance-compiler-check-v1. Its located conditional-bound
analysis limitation remains a support gap to investigate, not an invalid-input exclusion.

The private composition of the named-selector and social-cardinality repairs
passes both focused public test groups. ROOT independently reconstructed the
original patch hunks and checked the two changed paths against the unchanged
main baseline, then verified both complete test streams and reaped empty groups.
Evidence is under semantic62-named-selector-social-card-owned-composition-preparation-v1.
Original-model controls, actual-main gates and integration remain pending.

An actual-fact check confirms that knapsack selects the standard sum overload
with optional decision integer elements; its decision-set enum generator and
parameter array body retain symbolic Unknown values. A tests-only reduction
reaches the expected completion assertion but remains Limited because the
collection signature is unsupported. The existing numeric partiality guard
passes. ROOT verified all four complete streams, both reaped empty groups and
1165 unchanged inputs. Evidence is under semantic62-knapsack-optional-sum-repair-preparation-v1/red.
Only source/type acceptance is retained for the original knapsack model; no
matching instance data or instance compile acceptance is claimed.

The same actual-fact check locates the nonogram gap in the local conditional
bound: initialized inspection retains an Unknown conditional value, while the
raw numeric-bound producer records unsupported syntax. Its existing supplied
instance compile proof remains separate. Evidence is under
semantic62-knapsack-nonogram-fact-preflight-preparation-v1. No conditional value,
array membership or whole-task completion is inferred from this diagnostic.

A fixed audit of terminal corpus shards 019–025 covers 350 ordered roots per
selection. ROOT independently recomputed all fourteen complete companion streams,
rule partitions and source-state counts and rechecked 148 capture pins: native
and companion diagnostics agree, with no Unobserved rows. Zincite classifies
44 roots as complete/resolved, 285 as data and 21 as fragment or rejected source;
these classifications do not establish compiler validity. Evidence is under
semantic62-b691-shards019-025-snapshot-audit.json and root-shards019-025-count-audit.json.
The remaining corpus capture is still running; no whole-corpus acceptance is claimed.

The tests-only nonogram conditional-axis reduction reaches the expected positive
completion failure. Closed branch and selector overflow counterguards retain
Limited outcomes, and the existing domains guard passes. ROOT checked all four
complete streams, both reaped empty groups and 1287 unchanged runtime pins.
Evidence is under semantic62-nonogram-conditional-axis-test-preparation-v1/root-red.
The private consumer repair is in preparation; no raw domain fact is relaxed.

The second private knapsack candidate completes its positive optional-sum case.
Its negative test placed division by zero only in the solve objective, outside
this search-coverage consumer's constraint-body inspection. Independent source
review confirms that fixture placement error; it does not establish an inspector
bypass. A corrected capacity-constraint negative is pending on unchanged candidate
production. The existing numeric guard passes. Evidence is under
semantic62-knapsack-optional-sum-repair-preparation-v1/green-v2/root-results.

The corrected knapsack capacity-constraint negative passes with division by zero
retained as a located limitation; the positive symbolic optional sum completes.
The existing numeric guard also passes. ROOT verified all four complete streams,
1201 unchanged inputs and both reaped empty groups. Independent local production
review has no unresolved finding. Evidence is under semantic62-knapsack-optional-sum-
repair-preparation-v1/green-v3. Actual-main gates, original controls and integration
remain pending; original supplied-instance compiler acceptance remains unobserved.

The private nonogram consumer repair passes its symbolic conditional-axis case,
closed branch and selector overflow counterguards and the existing domains guard.
Raw domain facts remain unsupported; checked source inspection supplies no value
or membership proof. ROOT verified four complete streams, 1303 unchanged runtime
pins and both reaped empty groups. Independent local review has no unresolved
finding. Evidence is under semantic62-nonogram-conditional-axis-production-preparation-v1/root-green.
Original-model controls, actual-main gates and integration remain pending.

The unchanged closed lex_lesseq unit model also compiles with MiniZinc 2.10.1 and
Gecode without solving, producing 1117 bytes of FlatZinc and 294 bytes of OZN.
Actual facts resolve both add_to_output references to the implicit standard
Annotation declaration with present parameter Ann type, zero parameters and no
body. Initialized-source inspection rejects both annotated arrays. ROOT checked
four complete streams, 1159 unchanged inputs and both reaped empty groups under
semantic62-atomic-standard-annotation-preflight-preparation-v1. The predicate-body
inspection limitation remains a separate observation; no whole-model completion
or annotation-policy repair is claimed from this preflight.

The four-repair private composition builds with normal release settings. ROOT
checked all sixteen Cargo events, thirteen optimized artifact events, two frozen
executables, twenty-two library files and 1291 unchanged inputs. Sixteen original
model controls then complete with exact native/companion diagnostics and status
parity: ROOT checked thirty-two full streams, eight root records, 1325 unchanged
inputs and all reaped empty groups. Knapsack search coverage and nonogram array
index analysis now complete under both presets; both models complete all fourteen
thesis analyses. Knapsack retains an uncovered-search warning, not a fabricated
coverage certificate. ZipQueens remains Limited at its invoked actual type or
optionality check. Social Golfers loses the cardinality limitation but retains
separate search and unused-declaration limitations. Evidence is under
semantic62-named-selector-social-card-knapsack-nonogram-functional-controls-preparation-v1.
This is private functional evidence; actual-main gates, integration and final
corpus validation remain pending, with no isolated performance claim.

The tests-only atomic standard annotation reduction passes its exact binding,
origin, type and zero-parameter preflight, then fails its positive coverage
assertion with Unknown instead of WholeArray. Existing user annotation and
partial-call refusals pass before that assertion. ROOT checked four full streams,
1284 unchanged inputs and both reaped empty groups under
semantic62-atomic-standard-annotation-red-preparation-v1/root-results. Production
is unchanged; the separate original lex_lesseq body limitation remains open.

The private atomic-annotation repair passes that positive case, the existing
user/partial-call refusals and the numeric safety guard. ROOT checked four full
streams, 1417 unchanged inputs and both reaped empty groups under
semantic62-atomic-standard-annotation-production-preparation-v1/root-results.
Removing the new helper and restoring its four inspection calls reconstructs
the baseline exactly; the global annotation API is unchanged. Original-model
controls, actual-main gates and independent task verification remain pending.

The unchanged Jobshop documentation model compiles with its supplied jdata.dzn
through MiniZinc/Gecode without solving, producing 27734 bytes of FlatZinc and
395 bytes of OZN. Its actual enum_next tuple preserves the same TASK enum
identity through the source, generator binder and result. Source inspection
rejects the successor as an arbitrary value call; its index membership remains
Unknown. The owning filter is j < last, with last = max(TASK), whose nonemptiness
and value remain unproved. ROOT checked four full streams, 1302 unchanged inputs
and both reaped empty groups under semantic62-jobshop-enum-next-preflight-preparation-v1.
No successor membership, totality, output certificate or repair is inferred.

The five-repair private composition adds the bounded atomic-annotation repair to
the four-repair build. Its normal release build passes, with 1533 unchanged
inputs. Original Lex controls agree between native and companion captures under
both presets: all fourteen thesis analyses complete, with four warnings and no
limitations. The all-rules preset retains one separate vacuous-constraint
limitation. ROOT checked eight full streams, 1567 unchanged inputs and all four
reaped empty groups. This supersedes the earlier inference that the isolated
predicate-body refusal established an original Lex search limitation. Independent
local annotation review reports no actionable finding. Evidence is under
semantic62-four-gap-atomic-annotation-functional-controls-preparation-v1.
Actual-main integration and full-task verification remain pending.

The tests-only Jobshop reduction reaches its positive completion assertion after
the identity and safety preflight and negative cases pass. The existing array
cycle guard passes. ROOT checked four full streams, 1300 unchanged inputs and
both reaped empty groups under semantic62-jobshop-enum-next-test-preparation-v1/root-red.
A bounded private successor inspection repair is being prepared; no successor
value, membership or output guarantee is established by the failing test.

The first ZipQueens generator diagnostic fails before its body-feasibility and
behavior assertions: it incorrectly requires the selected standard declaration
to belong to an implicit include. This is a diagnostic preflight failure, not a
confirmed generator behavior failure. The numeric safety guard passes. ROOT
checked four full streams, 1173 unchanged inputs and both reaped empty groups
under semantic62-zip-queens-generator-actual-red-preparation-v1/root-results.
The diagnostic is being corrected before any production repair is authorized.

The private Jobshop successor repair passes its unchanged positive and negative
regression and the existing whole-array/alias-cycle guard. ROOT checked four
full streams, 1321 unchanged inputs and both reaped empty groups under
semantic62-jobshop-enum-next-production-preparation-v1/root-green/runtime.
The repair returns Unknown successor value and index membership. Independent
local review and original-model controls remain pending; actual-main integration
and task acceptance are not established by these focused checks.

The corrected ZipQueens diagnostic reaches a genuine generator behavior failure.
The bare-array control completes through the same installed standard predicate
and retained default; the generator positive then fails at invoked actual typing.
The trace maps the generator header and scalar body as two arguments, whereas
overload selection correctly sees one comprehension array and the second formal
has a retained default. ROOT checked four full streams, 1201 unchanged inputs
and both reaped empty groups; the numeric safety guard passes. Later filter and
closed-error cases were not reached. Evidence is under
semantic62-zip-queens-generator-actual-red-preparation-v3/root-results.
A private argument-mapping repair is now authorized. Earlier diagnostic failures
remain retained separately; no production behavior or whole-task pass is claimed.

## User-requested pause, 2026-10-09

Base-083 remains open. Main Rust sources retain the b691 source map; reviewed
repairs are still private. The user requested a pause to conserve quota. All
four retained agents have been stopped; interrupted Zip implementation and
Social Golfers usage diagnosis require a fresh readiness check before reuse.

The six-repair normal release build passed and ROOT audited its sixteen Cargo
events, two binaries, twenty-two libraries and 1957 unchanged inputs. Original
Jobshop controls (handle 43377) and the Social Golfers membership observer
(handle 55632) subsequently returned terminal zero from their wrappers. Their
full captures still require ROOT audit; no semantic acceptance is inferred.
Evidence is under semantic62-six-gap-jobshop-functional-controls-preparation-v1
and semantic62-social-golfers-membership-preflight-preparation-v1.

Corpus handle 84137 was confirmed live at the pause, with 123/124 terminal
shards. Leave that existing capture intact. On resume, poll the same handle,
then use the prepared b691 final reconciliation only after confirmed terminal
state. Preserve the main Rust freeze until that audit completes. Next steps are
capture audits, original Jobshop outcome reconciliation, the Zip mapping repair
and remaining Social Golfers gaps; full workspace and task verification follow.

## Resume and complete baseline capture, 2026-10-09

Base-083 resumed with four delegated lanes: ZipQueens generator arguments,
Social Golfers membership, Social Golfers usage diagnosis, and original Jobshop
capture review. Integration, runtime checks, commits and task acceptance remain
ROOT-owned and sequential.

Corpus handle 84137 completed without restarting. The saved-capture reconciliation
passed: all 124 shards, 496 native/companion child receipts and 6417 original
roots are accounted for, with no missing or Unobserved rule rows. Both original
inventory checks cover 189496 files; tool/source/library checks cover 148 files.
All 5082 compiler streams were checked. Source/type classifications are 2241
Accepted, 3876 data-only, 276 compiler-nonzero and 24 retained Rejected; Accepted
still does not establish supplied-instance validity. The stale editor symlink
reported during discovery remains explicitly recorded.

The b691 thesis capture has 1173 complete resolved roots, of which 437 complete
all selected analyses; the all-rules capture has 190 such roots. These are
baseline observations, not acceptance of the pending repairs. The overlapping
three-process capture took 13321.94 seconds; it is not isolated performance
evidence. The audit is retained under
semantic62-b691-final-reconciliation-preparation-v1/root-audit.json.
The baseline capture is now settled, permitting reviewed repairs to enter main.
Base-083 and its final corpus, public behavior and performance verification
remain open.

The six prepared repairs are integrated into the main working tree: matching
rank-two named selectors, decision-set cardinality inspection, optional integer
sums, symbolic local conditional axes, atomic standard output annotations, and
parameter enum-successor inspection. All 108 candidate source pins were checked
against the settled baseline before copying the four changed paths. Actual-main
formatting, Clippy with warnings denied and all 227 workspace tests pass; sources
remain unchanged through the three checks. Gate captures are under
main-six-repair-gates-v1. Final independent task acceptance remains pending.

Original Jobshop captures were independently reconciled against the actual
b691 root records and diagnostics. All fourteen thesis rules complete, with
three warnings and no limitations. Search changes from Limited to Completed,
removes its arbitrary-value-call limitation and adds uncovered `s` advice;
uncovered `end` and the decision disjunction warning remain. The all-rules
preset retains three Limited rules and six limitations. All eight streams,
four receipts, 1991 control pins and 108 candidate files were checked. The
retained original/data Gecode compilation succeeds without solving. Successor
values and index membership remain unproved; this establishes no output
guarantee. Review is under
semantic62-six-gap-jobshop-functional-controls-preparation-v1/bounded-jobshop-review.json.

The Social Golfers membership reduction reaches a genuine behavior failure at
the Completed assertion after parsing and selected-call prechecks pass. It
reports unproved traversal membership for `schedule[w,1]`. The retained decision
set cardinality and numeric safety groups pass. ROOT checked six full streams,
three reaped empty groups and 1278 unchanged inputs under
semantic62-social-golfers-membership-red-preparation-v1/root-results.
A minimal private integer-membership source-inspection repair is authorized;
its later output/coverage assertions remain unobserved until the positive passes.

Independent local integration review reports no concrete soundness finding
across the four changed paths and their strict consumers. The reviewed main
files match the six-repair candidate. This review does not replace final
base-083 verification. The report is retained alongside the Jobshop review as
bounded-six-composition-review.json.

The ZipQueens generator repair is integrated. Generator calls retain one
independently inferred collection argument and their real later defaults;
forwarded source inspection checks headers, filters and body while extent stays
Unknown. Ordinary scalar generator results remain scalar actuals. Virtual
collections supply no output or search certificate.

The installed standard-body control passes all six cases: bare/generated and
parameter-filter positives complete, while optional and closed-error cases keep
limitations. The portable public group and numeric guard pass after correcting
fixture setup order. Independent local review checked all twelve argument
consumers and the formatting-only production revision, with no concrete finding.
ROOT checked 1436 unchanged v1 inputs and six streams, then 1567 unchanged v2
inputs and four streams. Earlier fixture and formatting failures remain retained.
Actual-main fmt, Clippy with warnings denied and all 228 workspace tests pass;
the three checks leave sources unchanged. Evidence is under
semantic62-zip-queens-generator-actual-repair-preparation-v2/root-results and
main-zip-generator-gates-v2. Original-model and final task verification remain
pending alongside the remaining Social Golfers repairs.

The Social Golfers integer-membership repair is integrated. Its exact selected
core tuple permits parameter-to-decision integer coercion and checked decision
set source inspection, with membership still Unknown. Both operands and lexical
header sources are checked. A closed selector below an invariant written range
lower bound is rejected; symbolic lower bounds supply no exclusion or membership
proof. Strict dependencies, raw domains and output machinery remain unchanged.

The five-case public group, retained cardinality guards and numeric guard pass.
ROOT checked six streams, three reaped empty groups and 1558 unchanged inputs;
independent local review reports no concrete finding. Actual-main fmt, Clippy
with warnings denied and all 229 workspace tests pass with unchanged sources.
Evidence is under semantic62-social-golfers-membership-green-preparation-v2 and
main-social-membership-gates-v1. The earlier selector-bound failure is retained.
Remaining Social Golfers work includes ordinary decision-set array construction,
infinity atom typing and generic standard-prelude availability; original replay
and final task acceptance remain pending.

Infinity atom typing is integrated: the source-preserving literal now has present
parameter-integer type, matching MiniZinc's parser and AST. Numeric values and
finite bounds remain unproved. The existing public callable group checks type,
selected identity and absence of fabricated finite bounds; the numeric partiality
guard remains unchanged. Both focused checks pass, with 166 unchanged inputs,
four checked streams and two reaped empty groups. Independent local review found
no concrete issue. Actual-main fmt, Clippy and all 229 workspace tests pass with
unchanged sources. Evidence is under semantic63-infinity-atom-green-preparation-v1
and main-infinity-atom-gates-v1. Original Social Golfers usage replay and final
base-083 acceptance remain pending.

Configured generic standard-prelude loading is integrated after core propagation.
The loader skips only an absent std/solver_redefinitions.mzn; existing paths and
other access failures retain ordinary loader errors. Newly loaded implementations
remain noncore and their real bodies are analyzed. The focused public group checks
minimal-library compatibility, core flags, prototype/body selection, supported
and unresolved helper bodies, and unreadable-prelude reporting. It passes with
1156 unchanged inputs, two checked streams and one reaped empty group. Independent
local review found no concrete issue. Actual-main fmt, Clippy and all 230 workspace
tests pass with unchanged sources. Evidence is under social_prelude-green-v1 and
main-generic-prelude-gates-v1. Original-body outcome and cost comparisons remain
pending; this is no claim of Social Golfers or whole-task completion.

The isolated original Social Golfers comparison confirms Infinity typing removes
eight unused-declaration limitations, adding no diagnostics. Thesis outcomes stay
12 Completed / 2 Limited: the unresolved fzn_array_set_union helper and ordinary
array construction remain. Both opt3 native/companion pairs agree on full output,
selected IDs and returned allocation scopes; ROOT checked 1222 unchanged inputs
and four streams per arm. The source-only and model/data no-solve compiler proofs
remain distinct and reusable. Evidence is under
semantic63-infinity-only-social-usage-replay-preparation-v1/root-comparison.json.

Decision-set array construction is integrated through the existing constructor
inspector and direct dispatch. It admits only present rank-one integer-axis arrays
of exact decision integer-set cells selected directly from a checked source.
Lexical headers, selectors, written domains and closed errors remain inspected;
success remains Unknown, with no output or search certificate. All three unchanged
public cases pass, including selector and member-domain overflow negatives.
ROOT checked 1423 unchanged inputs, two full streams and one reaped empty group;
independent local review found no concrete issue. Owned formatting then actual-main
fmt, Clippy and all 231 workspace tests pass with unchanged sources. Evidence is
under semantic62-social-golfers-constructor-green-preparation-v1 and
main-decision-set-constructor-gates-v1. Original helper-body and final corpus/task
acceptance remain pending.

The isolated generic-prelude original replay completes unused-declaration on Social
Golfers: thesis changes from 12 Completed / 2 Limited to 13 Completed / 1 Limited.
It removes the unresolved fzn_array_set_union diagnostic and adds none; ordinary
array construction remains the one limitation on this pre-constructor snapshot.
The actual loaded closure grows from 30 to 186 files, retaining real-body analysis.
ROOT checked 1497 unchanged inputs, four full streams, native/companion parity and
returned allocation scopes. Build streams and both opt3 binaries/22 libraries are
audited. Evidence is under social_prelude-original-replay-v1/control-results.
This functional comparison makes no isolated performance or whole-task claim;
current-main constructor/body replay and final expanded corpus remain pending.

Current1431 original-model controls are complete: 13 native/companion pairs agree
on complete resolved roots, selected IDs, full diagnostics and returned allocation
scopes. ROOT checked 52 streams, 26 reaped empty groups and 1285 unchanged inputs.
ZipQueens, Knapsack, Nonogram, Lex and Jobshop complete all fourteen thesis rules.
Social Golfers completes thirteen; its former constructor limitation moves into
the real fzn_partition_set body (decision-set source form unsupported). TeamMeetings
reproduces two thesis limitations, including ordinary sum over enum binders.
These are concrete remaining facts to repair, not missing symbolic input values.
Extra all-preset limitations remain explicit. TeamMeetings and Knapsack retain
source/type acceptance with instance validity unobserved; five other originals
retain their distinct original compile/no-solve proofs. Evidence is under
semantic63-current1431-original-model-controls-preparation-v1/control-results.
The four-asset current1431 opt3 build,22 libraries and both public/cost child-free
validators pass. Final public/cost/corpus execution remains pending after repairs.

Ordinary core integer sums now inspect present parameter enum headers with exact
source/binder identity. Existing lexical, initializer, filter, body and closed-error
checks remain; construction stays Unknown and supplies no output certificate.
The focused public group passes: the symbolic sum completes with meetings Uncovered
and no outputs; body and filter division by zero retain Unsupported safety facts
and Limited search results. ROOT checked 136 unchanged inputs, two full streams
and one reaped empty group. Independent local review found no issue. Actual-main
fmt, Clippy and all 232 workspace tests pass with 108 unchanged source files.
Evidence is under team_enum_sum-green-v2 and main-ordinary-enum-sum-gates-v1.
Original-model replay and final task acceptance remain pending.

TeamMeetings also has an accepted original plus existing yampc.dzn Gecode compile
proof, with no solve. ROOT checked 1175 unchanged inputs and both streams; retained
FZN/OZN outputs remain private under semantic63-team-existing-data-compiler-v1.
This supplies the instance check missing from the earlier original-model record.

Currentde6 diagnostic controls replay four exact originals with the audited opt3
build. All eight thesis/all native-companion pairs have complete resolved records,
matching statuses and full diagnostics, and returned allocation scopes. ROOT checked
1265 unchanged inputs,32 full streams and16 reaped empty groups; no roots are
unobserved. Parade and ATSP each retain one thesis rule Limited; Grouping retains
three and TeamMeetings two. The ordinary enum sum repair passes its public check
but does not complete the original TeamMeetings body. Remaining current refusals
include indexed search annotations, selected-set sum inspection, computed union
bounds and enum-set value inspection. Existing source/type-only proofs remain
distinct from TeamMeetings' accepted existing-data Gecode compile/no-solve proof.
Evidence is under semantic64-currentde6-four-original-controls-preparation-v2/
control-results/root-audit.json. This is diagnostic evidence; final public,
performance, whole-corpus and independent task verification remain pending.

Indexed annotation literals now inspect every alternative when a present symbolic
parameter selector retains a closed enum with exactly the literal's length.
Candidate search values and heuristics retain source, type, alias-cycle and
closed-error checks. Unchosen alternatives supply no search coverage; earlier
explicit coverage is preserved. The three focused public cases pass: symbolic
choices complete inspection, while division by zero and an opaque second
alternative retain located limitations. ROOT checked 144 unchanged inputs,
both full streams and one reaped empty group; independent static review found
no substantive issue. Evidence is under parade_annotation_selection-green-v1.
The permanent regression drops detailed fixture preflights retained in that
private evidence. Actual-main fmt, Clippy and all 234 workspace tests pass with
108 unchanged source files, under main-parade-selection-gates-v1. Original
Parade direct-definition limitations and final
public, performance, corpus and whole-task acceptance remain pending.

Strict parameter-integer sum dependencies now inspect a selected set source's
initializer before accepting its dependency identity. The selected-source closed
error scan also traverses exact core parameter-Boolean conjunction operands
without deriving their truth values. This repairs a public counterexample where
a selected initializer containing division by zero incorrectly completed search
inspection. Both unchanged private cases pass: the symbolic sum remains Completed
with unknown data and no output or scalar-search certificate, and the closed-error
variant remains Limited. ROOT checked 150 unchanged inputs, both full streams and
one reaped empty group, under atsp_selected_set_sum-green-v1. Actual-main fmt,
Clippy and all 235 workspace tests pass with 108 unchanged source files, under
main-selected-set-sum-gates-v1. Fresh original ATSP outcomes and final task
acceptance remain unobserved.

The weighted version of the same public fixture now completes inspection without
scalar-search or output coverage. Its closed-error variant remains Limited, but
the let diagnostic forwards the earlier strict membership refusal instead of the
division-by-zero reason. That final diagnostic assertion fails; no unsound
completion is observed. ROOT checked 157 unchanged inputs, both full streams and
one reaped empty group, under atsp_selected_set_sum-weighted-diagnostic-v1.
Fresh original ATSP outcomes remain unobserved after this repair.

Parameter enum-set intersections and selections now have a direct source-inspection
path with exact enum identities and a full-enum array axis. It retains initialized
operand, selector, annotation, cycle and closed-error checks; success is Unknown
and creates no membership or output certificate. Both public Team cases pass:
the symbolic intersection/cardinality sum completes search inspection with
meetings Uncovered and no callable outputs, while to_enum(Team,0) retains its
Unsupported ordinal-outside fact and Limited result. ROOT checked 173 unchanged
inputs, both full streams and one reaped empty group, under team_enum_set_value-
green-v1. Actual-main fmt, Clippy and all 236 workspace tests pass with 108
unchanged source files, under main-enum-set-value-gates-v1. Fresh original
TeamMeetings and final task acceptance remain pending.

Decision-set union generators now require inspection of their selected value
body as part of source admission. The supported shape retains one exact owning
formal array, its own index_set header and binder, declared domains and closed
errors. A local body view supplies typed checks without changing Boolean callable
selection or supplying output facts. A written recurrence guard precedes helpers
that restart source traversal. All eight focused public cases pass, including
selector/member overflow, aborting or opaque selected bodies, intersect/card
wrappers and the length/card recurrence. ROOT checked 1181 unchanged inputs,
both full streams and one reaped empty group; independent review of the revised
patch found no substantive issue. Evidence is under semantic63-social-array-union-
green-preparation-v6. Actual-main fmt, Clippy and all 237 workspace tests pass
with 108 unchanged source files, under main-decision-set-union-value-gates-v1.
The installed Social body still has unsupported annotation
and conditional/let prerequisites; original completion and final acceptance are
unclaimed.

Let-source fallback failures now retain their actual Unsupported reason instead
of forwarding the earlier strict-dependency refusal. The eligibility/type guards
and Unknown admission are unchanged. Both exact weighted public cases pass: the
symbolic sum completes without scalar-search/output coverage, and the closed
error remains Limited with its division-by-zero reason. ROOT checked 163 unchanged
inputs, both full streams and one reaped empty group, under atsp_selected_set_sum-
diagnostic-green-v1. The existing permanent selected-set fixture now uses weights[j]
instead of a literal body; no extra regression group is added. Fresh original
outcomes and final task acceptance remain pending.

Actual-main formatting, clippy and workspace tests pass: 237 tests and 108
unchanged source files, recorded under main-let-source-diagnostic-gates-v1.

Fresh original-model checks after 2b58974 use the normal release build under
semantic64-post-repair-release-preparation-v1. All ten thesis/all native-companion
pairs finish with complete resolved records, matching diagnostics/status, returned
allocation scopes and no timeout or leftover. ROOT checked 1,335 unchanged inputs,
40 full child streams and 20 reaped empty groups under
semantic64-post-repair-five-original-controls-preparation-v1.

Team Meetings now completes all 14 thesis and all 26 expanded analyses. Parade
and ATSP remain 13/14 and 21/26; Grouping remains 11/14 and 22/26; Social Golfers
remains 13/14 and 24/26. Parade's symbolic annotation limitation is removed;
ATSP's expanded partial-expression analysis now completes. Remaining thesis
limitations are direct definitions in Parade, nested indexing and optional sums
in ATSP, computed bounds/definitions and an optional local in Grouping, and the
selected installed union body in Social Golfers. These are functional observations;
final public, performance, full-corpus and independent task acceptance remain open.

Focused remaining-path checks now establish behavioral RED on current source:
ATSP's nested selector refuses its direct definition, and a separate prefix-first
capture reaches the present integer binder refusal for both optional sums. The
same four fixtures and preflights were retained; only final assertion order changed.
Parade's corrected test reaches the rank-one decision selector refusal after its
complete/resolved/type/no-output checks. Its first capture failed to compile due
to a private test API call and proves no behavior. Social's atomic commutative-hint
case reaches the selected-body annotation refusal. Captures are retained under
semantic65-atsp-remaining-preparation-v1, semantic65-atsp-prefix-first-preparation-v1,
semantic65-parade-direct-definition-preparation-v2 and
semantic65-social-installed-body-preparation-v1. ROOT checked complete streams and
unchanged inputs; later final assertions or cases remain unobserved where the first
positive assertion aborts. Repairs and fresh original completion remain pending.

Rank-one decision integer selections now inspect the written source and selector
while retaining Unknown membership and withholding search/output certificates.
The focused Parade positive and both closed-error cases pass under
semantic65-parade-direct-definition-green-v1. Atomic implicit-standard
promise_commutative annotations now permit the same body inspection as the
existing totality hint; the selected body still has to pass its prerequisites.
Social's atomic-hint case passes, while its actual conditional body remains
Limited under semantic65-social-installed-body-preparation-v2.

The existing distinct-symbolic-domain selector case now expects Completed source
inspection with Unknown coverage. Declaration identities remain distinct and no
membership or output proof is supplied; its exact model passes MiniZinc's
source/type check. The optional-selector refusal remains. Actual-main formatting,
clippy and all 238 workspace tests pass under
main-rank-one-selection-and-annotation-gates-v2; ROOT checked 108 unchanged source
files, six complete streams and three reaped empty groups. Fresh original-model
outcomes and final task acceptance remain open.

Optional integer sums now inspect a selected decision integer range's endpoints
and present parameter integer binder without computing its extent or membership.
The existing enum path remains separate. The focused prefix positive and closed
zero cases pass before the independent nested-selector refusal under
semantic65-atsp-optional-prefix-green-preparation-v1; that four-case suite is
not wholly green. A two-case permanent public projection retains completed
inspection, absent output/search certificates and the located zero refusal.
Actual-main formatting, clippy and all 239 tests pass under
main-atsp-optional-prefix-gates-v1. ROOT checked 108 unchanged source files, six
complete streams and three reaped empty groups. Original ATSP completion and
final task acceptance remain pending.

Selected decision-set union bodies now inspect complete typed conditional branches
through the existing source checks. Whole-body recurrence and annotation guards
remain first; the inspection provides no branch value or output certificate.
The symbolic empty/literal-set positive and possible branch-overflow negative
pass under semantic65-social-installed-body-preparation-v3. The actual installed
body remains Limited at its singleton array selection, with later original
negative assertions unobserved; the focused six-case suite is not wholly green.
The permanent pair extends the existing union regression group. Actual-main
formatting, clippy and all 239 tests pass under
main-social-conditional-sources-gates-v1, with 108 unchanged source files, six
complete streams and three reaped empty groups. Original Social completion and
final task acceptance remain open.

The selected union helper's singleton branch now inspects only its owning formal's
bodyless core array1d conversion, exact tuple, source and selector, under the
same-array nonempty branch guard and selector 1. It returns Unknown and supplies
no membership or output certificate. The singleton positive and selector-overflow
negative pass under semantic65-social-installed-body-preparation-v4; the actual
local body remains Limited at its initialized-source annotation, with later
original negatives unobserved. The permanent pair extends the existing group.
Actual-main formatting, clippy and all 239 tests pass under
main-social-singleton-sources-gates-v1, with 108 unchanged source files, six full
streams and three reaped empty groups. Original Social and task acceptance
remain pending.

Decision integer selections now locally interpret only checked parameter-array
min/max bounds as Unknown, retaining their declaration IDs, raw Unsupported facts,
source axes and closed arithmetic/empty-source vetoes. Selected primitive written
bodies, defaults, domains and annotations are checked before that interpretation.
Search now includes its existing safety/enforcement refusal reason under the same
prefix and location. The first private minmax candidate completed the symbolic
case and retained closed-zero Limited, then failed its generic-message assertion;
it was not a wholly green suite. The deep-guard candidate plus the retained reason
passes all three cases, including zero and known-empty errors, under
semantic65-atsp-minmax-green-preparation-v2. ROOT checked 119 unchanged inputs,
two full streams and one reaped empty group. A 98-line permanent projection
retains public outcomes, withheld certificates and the located errors, and uses
standard commutative metadata on its min/max stubs. Actual-main formatting,
clippy and all 240 tests pass under main-atsp-extremum-bounds-gates-v1, with 108
unchanged source files, six full streams and three reaped empty groups. Fresh
original ATSP outcomes and final task acceptance remain pending.

Fresh original ATSP controls now use committed dfe629a and an audited normal
release with four executables and 22 libraries. The build retains 1,175 unchanged
inputs and complete streams. The thesis/all native-companion pairs under
semantic65-atsp-original-replay-preparation-v1 retain 1,214 unchanged inputs,
eight full streams, four reaped empty groups and matching diagnostics/statuses.
Thesis remains 13/14 Completed with two limitations; all remains 21/26 Completed
with 17 limitations, down from four and 19 respectively. Rule outcome partitions
are unchanged. Search still refuses the original line-174 output dependency
selection and line-169 local constraints/result; the focused source-safety repairs
do not establish strict output-dependency membership. The outer wrapper failed
to record its receipt after its child completed because its imports omitted
platform; the inner captures are intact and independently audited, and the outer
group is absent. No original control was rerun to conceal that recording failure.

The private Grouping direct floor adapter consumes the independently tested domain
helper. Under semantic65-grouping-direct-floor-green-preparation-v2, both floor
cases pass their frozen public source-safety, Search and withheld-certificate
assertions. The separate reverse positive still fails its source-call assertion;
its following negative is unobserved. ROOT audited 132 unchanged inputs, both
full streams and the reaped empty group. The domain and direct floor increments
remain private pending their small permanent projections and main validation.

The Grouping domain producer and direct floor consumer are now integrated
together. They inspect the exact bodyless floor/division/sum selections, retained
array source and closed divisor, returning Unknown without a value, bound,
membership or output proof. Changed written primitive bodies/defaults/domains
and unsupported metadata remain refused. The direct caller reuses the domain
guard and preserves initialized-source failures. Permanent coverage retains
three domain cases and the two floor-chain cases without the private observation
matrix. Actual-main formatting, Clippy and all 242 tests pass under
main-grouping-floor-sources-gates-v1; ROOT checked 108 unchanged source files,
six complete streams and three reaped empty groups. The reverse source and fresh
original Grouping outcomes remain pending; base-083 is still open.

The fresh Social union v7 check reaches the actual conditional/local standard
body but remains Limited: initialized arithmetic inspection is unavailable.
The first five cases pass; the failing sixth leaves four later cases unobserved.
ROOT checked all 1,237 unchanged inputs, both full streams and the reaped empty
group. No Social candidate has been integrated or accepted from this capture.

The private ATSP let-equality diagnostic retains the failed candidate's behavior.
All three array element guards return Ok and the preceding demands source is
Supported; the next veto is uncertain_equality_header on that source. Its local
count-only admission does not recognize the ordinary symbolic num_demands bound.
The operand inspection and later negative assertions remain unobserved. ROOT
checked 125 unchanged inputs, both full streams and the reaped empty group.
The temporary trace must not integrate; both repairs and whole-task acceptance
remain pending.

The selected union/FZN conditional, local and generator body checks now pass
the ten focused v8 cases under semantic65-social-installed-body-preparation-v8.
ROOT checked 1,247 unchanged inputs, both full streams and the reaped empty
group. Main retains three new cases: the real local-body shape, a changed
selected operator body and a known-empty local array. The body walk produces
neither definitions nor output guarantees. Ordinary union calls and guarded
partition-set closure remain separate boundaries pending fresh original controls.

The reverse source reader inspects the selected standard body, initialized
sources and exact formal/local/binder identities. An installed-library control
first exposed unsupported primitive metadata; the local guard now reuses the
union reader's atomic standard annotation check. Under
semantic65-grouping-reverse-source-green-preparation-v3, the installed-library
positive and the three focused cases pass. ROOT checked 1,163 unchanged inputs,
four full streams and two reaped empty groups. Main's three cases retain the
actual arithmetic metadata and refuse initialized-source and selected-body zero
errors. Values, extents, membership and output proofs remain unproved.

The ATSP let-equality source guard and bare symbolic count header now preserve
actual element/header/operand errors instead of replacing them with the earlier
strict membership limitation. Its private fallback returns Result<bool,String>;
both successful uncertainty and actual source errors withhold both output
directions. The positive, zero and empty cases pass under
semantic65-atsp-remaining-preparation-v1/let-minmax-green-v3, with 131 unchanged
inputs, both full streams and a reaped empty group. Main retains those three cases.

The three increments are integrated together. The first main Clippy run found
four redundant closures; those were removed. Main validation under
main-union-reverse-let-sources-gates-v2 passes formatting, Clippy and all 245
tests. ROOT checked 108 unchanged source files, six full streams and three
reaped empty groups. Fresh original and complete corpus controls, measurements
and whole-task independent verification remain pending; base-083 stays open.

Fresh original controls now use committed 6af4a55 and its normal optimized
release build. The build completed successfully; ROOT checked its unchanged
inputs, full streams, four executables and 22 libraries. The twelve serial
controls under semantic65-three-original-post-integration-replay-preparation-v2
completed without timeouts or remaining processes. ROOT checked all 1,272
unchanged inputs, 26 full streams and 13 empty process groups; native and
companion statuses and ordered diagnostics agree for all six selections.

ATSP completes 14/14 thesis rules and 22/26 all rules, with 20/51 warnings and
0/15 limitation diagnostics respectively. Compared with the preceding ATSP
capture, both selections remove two limitation diagnostics; search coverage
now completes. Grouping completes 13/14 thesis rules and 24/26 all rules,
with 3/6 warnings and 1/2 limitation diagnostics; each selection removes six
limitation diagnostics. Its remaining paths are guarded callable interpretation
and the optional local declaration in fzn_increasing_int_opt. Social completes
13/14 thesis rules and 24/26 all rules, with 0/12 warnings and 2/8 limitation
diagnostics, unchanged from its preceding capture. All six selections report
zero errors. These are source-only lint controls, not solver or final corpus
acceptance. Decision-prefix work, remaining support gaps and whole-task
verification remain pending.

The private decision-prefix v2 fixes complete-body token inspection and preceding
header scope, but its focused positive still fails. A tests-only diagnostic
preserving all production and fixture bytes narrows the remaining limitation to
the predecessor prefix's numeric array-membership refusal. The plain prefix has
no reported limitation in that capture. ROOT checked all 1,181 unchanged inputs,
both full streams and the empty reaped group under
semantic65-atsp-decision-prefix-source-preparation-v1/diagnostic-v1. The positive's
later Unknown assertions and both negative cases remain unobserved; this candidate
has not been integrated. The next repair is scoped endpoint source inspection,
without a membership or value proof and without changing raw numeric domains.

The fresh Social dependency graph exposed a separate identity gate: the actual
fzn_array_set_union standard file is included rather than implicit. Its selected
predicate now reaches the existing mandatory full-body inspection in either
case. The production repair removes one local implicit-file requirement; the
existing three-case regression now loads that predicate from a separate standard
file. Its positive and both source-error cases pass, and the private original
Social Search check completes while leaving Sched Uncovered. ROOT checked 1,186
unchanged inputs, both full streams and an empty reaped group under
semantic65-social-included-fzn-identity-preparation-v1. Only the production repair
and strengthened existing test are integrated. Main validation under
main-social-included-fzn-gates-v1 passes formatting, Clippy and all 245 tests;
ROOT checked 108 unchanged sources, six full streams and three empty reaped groups.
Fresh complete corpus evidence and whole-task verification remain pending.

The prefix v3 positive now passes its completion, Unknown count/coverage and
retained membership assertions; the earlier-header zero case also passes. Its
closed body-type overflow still completes both selected rules. Reusing the
existing closed-integer source checker on written types in v4 makes
index-set-mismatch Limited, while expensive-comprehension still completes because
it does not consume the referenced array's unavailable dimension facts. ROOT
checked all 1,204 unchanged inputs, both full streams and an empty reaped group
under semantic65-atsp-decision-prefix-source-preparation-v1/green-v4. The overflow
reason assertions remain unobserved. Neither candidate is integrated; the next
bridge must retain actual expression dependency identities and ignore unrelated
declaration limitations.

The private prefix v5 passes all three actual-standard-library controls: symbolic
completion retains Unknown count, coverage and membership, while the earlier
header zero and closed body-type overflow produce located Limited outcomes with
no findings. ROOT checked 1,220 unchanged inputs, both full streams and an empty
reaped group. Independent review then found that the reused closed-type checker
also evaluates literal fragments in inactive conditional branches. A fresh
MiniZinc source-only check accepts the positive with the element bound
`0..(if false then 1 div 0 else 1 endif)`; ROOT checked its 1,041 unchanged inputs,
both full streams and empty reaped group. The public inactive-branch control and
repair remain pending, so v5 is not integrated.

The Grouping v2 harness compiles and reaches all three complete-root and actual
optional-overload preflights. Each case then fails an overly broad empty-output
assertion: the installed symmetry wrapper legitimately exposes its scalar Bool
formal. ROOT checked 2,321 unchanged inputs, both full streams and an empty reaped
group. The later Search assertions remain unobserved. The next tests-only
correction checks the selected predicate and root array rather than unrelated
callable outputs; the three accepted source-only compiler receipts are retained.

The current-main prefix repair now passes all four actual-standard-library cases,
including the inactive bound, and the portable four-case regression passes in one
test. ROOT checked 1,276 and 1,299 unchanged inputs respectively, both complete
streams and an empty reaped group for each run. Both closed-fragment and referenced
source scans now omit the same proven inactive literal branch only during written
type inspection; existing callers retain their traversal. The primitive metadata
visitor reuses the existing standard annotation checker. Main formatting, Clippy
and all 246 tests pass under main-decision-prefix-gates-v2, with 108 unchanged
sources, six complete streams and three empty reaped groups. The first gate run
is retained with its needless-reference Clippy failure and unobserved tests.

Grouping v3 reaches the intended behavior checks after narrowing its output
assertion. The selected-body zero case passes all assertions. The positive still
rejects the optional local declaration; the source-zero case is Limited but that
same refusal hides its actual initialized-source error. ROOT checked 2,326
unchanged inputs, both full streams and an empty reaped group. Its source reader,
fresh original-model replays, final corpus/cost evidence and whole-task
verification remain pending; base-083 stays open.

Fresh normal-release replay at 2757cc9 completes all 14 thesis rules on ATSP and
Social; Grouping remains at 13/14. The six native/companion pairs agree. ROOT
checked 1,385 unchanged inputs, 26 complete streams and 13 empty reaped groups.
That replay also exposed ordinary parameter ranges reaching the decision-prefix
predecessor check before source eligibility. The dispatch now requires an actual
decision integer array cell before checking that predecessor; ordinary parameter
ranges retain their existing interpreter. The portable regression adds this
positive case while retaining the four existing error and inactive-branch cases.
ROOT checked its five-case pass against 1,169 unchanged inputs and both full
streams. Main formatting, Clippy and all 246 tests pass under
main-parameter-range-dispatch-gates-v1, with 108 unchanged sources, six complete
streams and three empty reaped groups. Fresh corrected-source model replay remains
pending. Grouping's candidate passed independent static review; its focused
runtime acceptance remains pending. Whole-task verification and final corpus/cost
evidence remain outstanding; base-083 stays open.

The corrected ee97571 normal release and original-model replay are now checked.
The build retains 1,202 unchanged inputs, two full streams, an empty reaped group,
18 normal opt3 Cargo events, four executables and 22 libraries. The six replay
pairs retain 1,498 unchanged inputs, 26 full streams, 13 empty reaped groups, exact
14/26 rule selections and native/companion status and diagnostic parity. ATSP
completes 14/14 thesis and 23/26 all analyses; Social completes 14/14 and 25/26;
Grouping remains 13/14 and 24/26. The ordinary-range false limits are gone;
remaining concrete limitations still require investigation.

Grouping's first private optional-body candidate compiled but failed two of three
cases: source-zero passed; positive used an immediate-token helper for a complete
body comparison; body-zero retained its failure but lost the precise reason.
ROOT checked 2,330 unchanged inputs and both streams. The next candidate fixes
complete token selection and the precise closed zero-divisor reason. Its
body-zero passes, but positive reaches the unchanged 240-second cap and source-
zero is unobserved. ROOT checked 2,456 unchanged physical inputs after timeout,
both streams and an empty reaped group. The runner stops before writing after.json;
this absence and the timeout remain explicit. No candidate is integrated and no
deadline is raised. A bounded private positive-path investigation follows.

The cost review found the historical shared common.mzn hash in semantic56's
before/after maps; the earlier private no-historical-hash claim was too broad.
Seven retained cost cases have matching source/dependency/configuration and
protocol evidence. Only two missing operator cases are being prepared against
those exact historical assets. This does not create a nine-case comparison or
attribute those assets to 6af. Final candidate costs, corpus reconciliation and
whole-task verification remain pending; base-083 stays open.

The unchanged selected-set helper now has direct original-ATSP evidence: all
three selected cells, with exact resolved owning arrays and preceding generator
headers, return inspected symbolic Unknown. ROOT checked 1,163 unchanged inputs,
both full streams and an empty reaped group. This is helper evidence only; the
iteration consumer repair and its public behavior checks remain pending.

The two missing semantic56 operator baseline cases completed under the existing
all/repeat2 protocol. ROOT checked 1,235 unchanged inputs, 12 distinct receipts,
24 full streams and two empty reaped outer groups. Both complete resolved roots
retain exact 26-ID partitions, native/companion parity and returned allocation
scopes. Each completes 24 rules and limits two; the 100/1,000-operation inputs
retain 201/2,001 limitation diagnostics. These complement the seven historical
cases against the exact retained assets and 90-source map; no final candidate
comparison or ratio is admitted.

The private Grouping positive-only diagnostic passes all public assertions in
200.77 seconds including compilation. ROOT checked 2,574 unchanged inputs, both
full streams and an empty reaped group. Its 120 wrapper reconstructions total
139,734 ms; analyze_model takes 64,259 ms. This instrumented attribution is not
a normal-release cost claim. Independent static review found exact immutable
resident views for most wrapper tuples: their constructor inputs match, and
all written-source/default/metadata/type checks surround the proposed reuse.
A local reuse candidate retains the existing constructor when no exact
nonrecursive resident exists. Full three-case acceptance, final native costs,
corpus reconciliation and whole-task verification remain pending.

The Grouping resident-view candidate now passes all three real-standard-library
controls within the unchanged 240-second cap (225.82 seconds including
compilation). ROOT checked 2,583 unchanged inputs, both streams and an empty
reaped group. The current-main portable projection compiles, but its positive
case expects Uncovered where the controlled library returns Unknown. ROOT
checked 2,716 unchanged inputs and both streams; later assertions and error
cases remain unobserved. This projection is being checked before integration.

The selected-set iteration candidate passes its positive and initialized-set
filter-zero assertions. Its header-zero case fails an overly broad expectation
that both rules must be Limited: expensive-comprehension is Limited, while
index-set-mismatch is Completed. Independent review confirms that an index
obligation proven from matching declaration identities does not certify header
evaluation. The revised test will retain the expensive-comprehension zero error
and check the index rule's separate outcome. ROOT checked 1,184 unchanged inputs,
both streams and an empty reaped group. Production changes remain private;
base-083 and final acceptance remain open.

The selected-set v2 public regression passes all three cases. ROOT checked
1,197 unchanged inputs, both full streams and an empty reaped group. Positive
selected sources remain inspected symbolic Unknown without invented members,
counts or coverage; both error controls retain the required located failures.
The production delta dispatches actual parameter-set array selections to the
existing source reader and preserves the guarded and declaration-source checks.
It changes no raw domain interpretation or index rule. ROOT integrated the exact
production and portable-test bytes; final release, original-model replay, cost
comparison, corpus reconciliation and whole-task verification remain pending.
Required integrated-main formatting, Clippy and workspace tests pass (247 tests);
ROOT checked 108 unchanged sources, six streams and three empty reaped groups.


The optional-increasing repair is integrated into main. It inspects the exact
selected standard recurrence and optional wrappers while retaining symbolic
Unknown, closed source/body errors, and no invented output or coverage facts.
The portable written-body regression passes its positive and two zero-error
cases. A fresh-target counter-test compiles both source versions independently:
the repaired version passes and the original version fails on unsupported local
optionality. Earlier shared-target comparisons were inconclusive because Cargo
could reuse a library from another copied workspace with preserved timestamps.

The actual-standard-library three-case control also passes within the existing
240-second limit. Required integrated-main formatting, Clippy and workspace tests
pass; the retained v4 capture records the final test count and ROOT audit.
One existing set-array error assertion now expects the observed precise division
by zero reason; the separate rank-two interpreter keeps its combined arithmetic
reason. Rule limitation and no-output checks remain intact. Final release,
original-model replay, matched costs, corpus reconciliation and independent
whole-task verification remain pending; base-083 remains open.


Final-source validation at 4fea5c2 now has a fresh normal opt3/no-debug release:
four executables, 22 libraries, 18 Cargo events and 108 unchanged sources. ROOT
verified both streams, 1,204 input pins and the empty reaped build group.
All six original native/companion pairs match full diagnostics and status.
ATSP, Grouping and Social Golfers each complete all fourteen thesis analyses;
the all preset retains respectively two, one and one Limited rules. Their
located limitations remain acceptance work, not invented completion.

The 30 public controls pass, including default omission, shared-root ordering,
source/token preservation and two-pass stdin formatting. All 700 retained save
samples pass expected statuses. The missing temporary crossword input was
restored from the pinned public archive with its exact retained bytes/hash.
The 908,888-byte original measures about 88 ms p95 and 52 MiB exact-child RSS,
within its applicable budget; the 1,189,780-byte formatted input measures about
102 ms p95 and 63.5 MiB RSS, with the over-1-MiB qualification retained.

The instrumented Grouping positive passes with matching 465 operation and 120
wrapper tuple sequences. Fresh body constructions fall from 120 to 30; summed
wrapper timers are 139,775 ms before and 36,989 ms after. Other recorded source
differences prevent an isolated attribution or normal-release ratio claim.
The final nine-case native/companion cost capture is complete. Its saved-record
comparison retains null ratios where dependency evidence differs; that evidence
is being reconciled. Full final corpus capture and whole-task verification are
still pending. Base-083 remains open.


The first final corpus launch stopped before creating a capture or starting any
shard: 2,326 temporary archive inputs had disappeared. ROOT restored only the
missing files from the pinned Challenge-a844 ZIP (2,205 files) and documented
2026 tar archive (121 files), checking retained bytes, hashes and modes. Separate
workers confirmed both restored subsets. All 189,496 prior inventory paths are
present again. The fresh final-corpus-4fea5c2-v2 preflight passed and all three
shard lanes started; terminal corpus outcomes and acceptance remain pending.

All final cost observations complete: 30 native and 15 companion, with no
cutoffs; maximum native wall time is 11.11 seconds against the 180-second cap.
Shared-root CPU is roughly 0.55 seconds per root in the 1/3/10/20 controls, and
cumulative requested allocation scales with that count. RSS grows to 92.6 MiB;
its allocator-level cause is not established, despite measured live-allocation
scopes returning to baseline. Twelve semantic selection pairs load 156 extra
standard files compared with the retained baseline, so full-work speed ratios
remain null. Current scaling evidence is separate from historical speed claims.

The original all-preset limitations are reconciled to actual fact-reader gaps:
ATSP numeric generator/array-access interpretation, Grouping guarded callable
interpretation, and Social decision-set/intersection/partition interpretation.
They are not missing dependencies or proof of invalid models. These remaining
vacuous-constraint and suspicious-domain gaps prevent an all-26 completion
claim; the observed fourteen-rule thesis controls remain complete.

While the final corpus capture runs, five private candidates have passed ROOT
static review: bare full-axis slice binding, ordinary bodyless exists source
inspection, decision enum-set cardinality body inspection, enum collection
binders, and uninitialized nominal enum-subset domains. Their preparations are
under `target/benchmarks/base083/semantic66-full-axis-unused-preparation-v1`,
`semantic65-latinbool-ordinary-exists-preparation-v1`,
`semantic65-teams-decision-enum-card-preparation-v1`,
`semantic66-teams-enum-collection-preparation-v1`, and
`semantic65-teams-symbolic-enum-domain-preparation-v1`. Each retains its patch,
public regression and outstanding validation. No candidate is integrated or
runtime-verified; the capture's source files and release assets stay unchanged.

The optional-Float weak-equality candidate has passed static review. Its entry
guard preserves the existing fallback for non-Float selections. ROOT must
observe required original selected tuples, run
focused behavioral RED/GREEN checks with separate fresh Cargo targets, then
integrate passing repairs and run workspace gates. Any Rust repair requires
new final release and corpus evidence before base-083 can pass verification.

The final-corpus-4fea5c2-v2 capture is terminal with all 124 shard receipts and
6,417 original inputs. Its outer process exited 1. Shard 54 reached the
1,800-second child limit for both native and companion thesis/all selections
on a 9.3 MB generated BNN planner model. Shard 59's native selections finished
in about 1,548 and 1,744 seconds; its thesis companion matched, but its all
companion reached the same limit during analysis. No deadline was extended or
root omitted. These cutoffs remain performance and coverage work.

ROOT ran the prepared saved-capture reconciliation after terminal admission.
The audit independently rehashed all 189,496 original inventory files before
and after, and all 150 source/tool/binary/library pins; none changed. It retains
three Unobserved selection/root results from shards 54 and 59. All other shards
pass its capture checks. Capture integrity therefore remains false, and
semantic acceptance remains undecided. Compiler source/type classifications
account for 2,241 Accepted, 3,876 data-only inputs, 276 CompilerNonzero and 24
retained Rejected labels; these do not establish complete instance validity.
The saved result is under
`target/benchmarks/base083/semantic65-final-corpus-reconciliation-preparation-v2/root-audit.json`.
Private repair validation and a separate timeout investigation are now active;
base-083 remains open.
