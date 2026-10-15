# Query acceptance, 2026-10-11

This report retains evidence from successive implementation stages. The original
acceptance and base-100 resource failures below are historical captured outcomes.
The adopted contract accepts the named private reductions as incomplete previews;
complete structured control and write-refusal evidence is included below.
Base-101 repaired the remaining normal-library use-consumer resource failure;
final navigation and validation evidence is recorded in the last section.

Release build at `e7b8f012952f78b84eef18528a831ec77360008a`; Rust 1.98.1;
Apple M1 Max, arm64, macOS 26.6.2. Production code was unchanged. Both
`zincite-query` and `zincite query` used identical arguments and produced equal
exit status, complete stdout bytes and complete stderr bytes for each comparison.
The README contains paired recipes for item filtering, nested inspection,
assignment edits, structured filtering, enum reduction and semantic navigation.

## Private instance reductions

The read-only model was `~/MiniZinc/models/misc/seating/seating.mzn`. Inputs were
its `test_data/small.dzn` and
`instances/mixed/instance-guests-150-topics-10-tables-mixed.dzn`. No guest names,
records or private source are included here. Outputs and detailed evidence stayed
in `/tmp/zincite092/`; the model and both inputs retained their SHA-256 hashes.

Build and run the same settings through each command form:

```sh
cargo build --release -p zincite -p zincite-query
# Use mznStdlibDir from minizinc --config-dirs.
stdlib=/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc
model="$HOME/MiniZinc/models/misc/seating/seating.mzn"
data="$HOME/MiniZinc/models/misc/seating/test_data/small.dzn"
zincite-query --model "$model" --stdlib-dir "$stdlib" -I "$stdlib/gecode" \
    'reduce_enum("Guests", keep_first(8))' "$data" > /tmp/small-query.dzn
zincite query --model "$model" --stdlib-dir "$stdlib" -I "$stdlib/gecode" \
    'reduce_enum("Guests", keep_first(8))' "$data" > /tmp/small-root.dzn
```

Use the mixed input and `keep_first(110)` for the second pair. Capture stderr and
status as well as the candidate: all four runs exited **1**, reporting four
computed enum-indexed accesses in the read-only model as unresolved dependencies.
They returned inspectable, reparsed candidates, but did not establish complete
reductions. In-place writing remains unavailable for these incomplete results.
Under the subsequently adopted contract, these two named reductions are accepted
as incomplete best-effort previews. Their computed dependencies remain explicit;
general computed enum-index expression evaluation is excluded. Compiler acceptance
does not upgrade either candidate to a complete reduction.

| Check | Small | Mixed |
| --- | ---: | ---: |
| Input / candidate bytes | 1,370 / 1,001 | 27,035 / 21,405 |
| Enum members / keyed records | 12 → 8 | 150 → 110 |
| Retained nested interest-array width | 3 | 10 |
| Same-table groups | 2 → 2 | 23 → 23 |
| Different-table groups | 1 → 0 | 14 → 14 |
| Surviving singleton same-table groups | 1 | 1 |

Direct JSON inspection checked exact retained enum prefixes and spelling, exact
record-key coverage and order, and byte-identical retained record values,
including nested interest arrays. Each supported group set matched its original
ordered members after removing discarded guests. Newly empty groups disappeared;
singletons stayed. The earlier manual reductions are feasibility controls,
so their group counts are not expected command results.

Unrelated `Topics`, `Tables`, `table_capacities` and
`max_gender_imbalance_per_table` assignment fragments remained byte-identical.
The small input had no comments. The mixed candidate retained 173 comments in
their original order and spelling; 44 removed comments belonged to discarded
enum or record entries. Removed guest members were absent from retained keys
and supported sets; strings remained part of unchanged retained records.

MiniZinc 2.10.1 build 33348285743 accepted both originals and both candidates
with exit 0 and empty stdout/stderr using:

```sh
minizinc --solver gecode --instance-check-only "$model" "$data"
minizinc --solver gecode --instance-check-only "$model" /tmp/small-query.dzn
```

The mixed pair used the same options. These checks performed no solving and did
not upgrade Zincite's incomplete status or establish satisfiability.

## Recipes and resource limits

Temporary synthetic inputs exercised all six paired recipes. Item counting,
nested record-field inspection, assignment replacement, positive-element
filtering and explicitly keyed enum reduction exited 0 with equal output and
empty stderr. The enum example used only the enum and keyed records; positional
arrays require model facts even when they are unrelated to the target enum.

On a small model with a declared `capacity` and a constraint using it,
`filter(name("capacity")) | declarations | uses | json` returned the same
declaration-use fact through both forms. Without a configured standard library,
it exited 1 and reported unavailable builtin declarations and an unresolved
operator. Supplying the complete installed 2.10.1 standard library instead hit
the model-fact collection limit and exited 2 with empty stdout. This historical resource
gap prevented claiming complete semantic-query acceptance with that library at
the original captured build. The later admission and use-consumer stages below
account for it separately.
All runs used default query limits: nesting 64, work 1,000,000 and collection
100,000. The CLI provides no limit override. Collection accounting includes
visited slots and expanded values, so 100,000 written array elements can exhaust
the limit before an edit finishes.

## Fresh command measurements

The temporary measurement script reused `invoke` and `peak_rss` from
`scripts/bench-save.py`. Each case/form had ten sequential fresh-process samples,
with complete stream capture and ordinary warm filesystem caches. The timer
included process creation and completion. First invocation was retained
separately; it was not controlled cold-cache evidence. Peak child RSS came from
a separate `/usr/bin/time -l` probe outside the sandbox, which otherwise denied
`kern.clockrate`. All probes returned the expected status.

Synthetic inputs were `weights=[...]` with `n` elements cycling through
`-1,0,1`, selected by `filter(name("weights")) | filter_elements(gt(0))` through
stdin with `--stdin-filepath data.dzn`. Real reductions used the settings above.
Measurements and native probe records are `/tmp/zincite092/measurements.json`.

| Case / bytes | Standalone median (min–max) ms | Root median (min–max) ms | Standalone / root RSS MiB | Status |
| --- | ---: | ---: | ---: | ---: |
| Small reduction / 1,370 | 161.69 (159.79–164.39) | 162.04 (160.26–166.42) | 52.70 / 54.17 | 1 |
| Mixed reduction / 27,035 | 163.47 (162.21–166.81) | 163.42 (161.24–165.56) | 53.14 / 54.58 | 1 |
| 1,000 elements / 2,345 | 4.64 (4.31–4.85) | 4.60 (4.34–5.05) | 2.89 / 3.00 | 0 |
| 10,000 elements / 23,345 | 8.96 (8.74–9.21) | 9.04 (8.75–9.51) | 7.91 / 8.03 | 0 |
| 100,000 elements / 233,345 | 26.50 (26.21–26.89) | 26.45 (26.06–27.02) | 36.95 / 37.05 | 2 |

The largest filter hit the explicit collection limit and returned no candidate;
its timing measures rejection, not successful transformation throughput. The
successful filter points show no unexplained superlinear growth. Real reduction
cost changed little between the two inputs with the same model/includes. First
invocations were 160.75/241.66 ms for small reduction, 164.73/166.26 ms for mixed,
5.39/5.08 ms for 1,000 elements, 8.95/9.50 ms for 10,000 and 26.71/27.45 ms for
the rejected 100,000-element case (standalone/root).

Ten samples support a bounded comparison, not a save-path p95 budget claim.
Query operations load model facts when required; they have no formatter-save
budget. Shared syntax/layout was unchanged, so save behavior was not remeasured.
The corpus/thesis gate remains separate from these instance checks.

Required validation passed: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. No new test was added because these checks demonstrated
supported behavior and explicit limitations rather than an integration defect.

## Finite semantic model admission, 2026-10-11

Base-100 corrects the source/result conflation above. The release binaries were
built from `e0dc7bf058b426f16ecf81bdb880213aad724e61` plus the base-100 query
changes, with Rust 1.98.1 on the same M1 Max. The library is MiniZinc 2.10.1 build
33348285743 at
`/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc`. No compiler
acceptance or solver run was performed for these queries.

Limits retain their public fields and finite defaults: nesting 64, work
1,000,000 and collection 100,000. Before the first semantic fact producer
allocates, the complete retained context charges each source token and CST node
once to work. Each node is charged before entering the scan frontier, bounding
that allocation too. Binding, callable and instantiation facts share this
admission; demanding another fact family does not charge the same source scan
again. Actual query-stage scans and output costs still consume work. This is a
source-size admission bound, not a timer or an accounting of every instruction
inside the fact producers. Collection continues to bound produced selections,
results, limitations and expansion frontiers; model source size does not consume
that result allowance.

The retained 88-byte `/tmp/zincite092/recipes.mzn` loads 179 files with 122,661
tokens and 40,420 CST nodes: 163,081 admission work units. The unchanged work
default is sufficient for this small declaration/type inspection:

```sh
stdlib=/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc
zincite-query --stdlib-dir "$stdlib" \
    'filter(name("capacity")) | declarations | types | json' \
    /tmp/zincite092/recipes.mzn
zincite query --stdlib-dir "$stdlib" \
    'filter(name("capacity")) | declarations | types | json' \
    /tmp/zincite092/recipes.mzn
```

Both forms exited **0**, with byte-identical complete JSON and empty stderr.
The single result identifies `capacity` at bytes 5..13, owning syntax bytes
0..18, with parameter `int` type. The focused public API regression also admits
20 unrelated library declarations with collection 1 and work 500 while
requesting declaration, type and instantiation facts. With work 100 the same
model is refused before fact allocation, at query bytes 0..12. A separate tiny
model producing three references under collection 2 is refused at query bytes
0..10. Both failures are explicit located errors and return no candidate. CLI
evaluation propagates these errors before rendering or writing stdout; its
limits remain fixed defaults, without a new override option.

At the base-100 captured build, the retained declaration-uses recipe passed model
admission but still exited
**2** at its actual `uses` stage, query bytes 41..46, with empty stdout and equal
stderr through both forms. That `uses` implementation resolved every retained reference,
including unrelated library references. A temporary public API probe with work
32,000,000 still hit that stage cutoff; with work 64,000,000 it returned an
incomplete result with unrelated library callable limitations. Those probes do
not justify raising the default or claiming complete declaration-use acceptance.
That consumer behavior was retained as a separate acceptance gap for base-092
and was repaired by base-101. The final section records successful acceptance
at unchanged default bounds; these earlier cutoff results remain historical.

The existing 100,000-element synthetic filter rejection was rechecked through
both forms: status **2**, empty stdout and the same collection error at query
bytes 25..48. Low-budget and unknown-predicate regressions and reduction refusal
checks passed with the workspace tests.

Ten sequential fresh-process declaration/type samples per form reused `invoke`
from `scripts/bench-save.py`, with ordinary warm filesystem caches. Standalone
median was **80.11 ms** (79.07–86.10); umbrella median was **80.69 ms**
(79.51–87.16). First invocations were 86.10/87.16 ms; these are not controlled
cold-cache results. A separate macOS `/usr/bin/time -l` standalone probe exited
0 with peak child RSS **28.55 MiB** (29,933,568 bytes). Before the correction, the
retained declaration-uses command exited 2 during admission with empty stdout;
its first invocation was 50.67 ms and separate RSS probe was 11.78 MiB. That
rejection is not a successful-query performance baseline. Measurements and
complete streams remain in `/tmp/zincite100/`; this is one representative query,
not a broad performance or corpus result.

Required validation passed: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` and `git diff --check`.

## Complete structured reduction and write control, 2026-10-11

This follow-up used the existing release binaries at
`8e58ced1d517aca8f033ee09762d97c85be3a41e`; no production source or test changes
were made. The public synthetic control and complete streams are temporary in
`/tmp/zincite092-followup/`. The control model declares an enum-keyed array of
records containing guest sets, a separate topic-indexed interest array and a
string; an integer-indexed group array and a literal reference to its third
group exercise group cleanup and reindexing.

Both `zincite-query` and `zincite query` ran:

```sh
zincite-query --model /tmp/zincite092-followup/control.mzn \
    'reduce_enum("Guests", keep("C", "A"))' \
    /tmp/zincite092-followup/original.dzn
zincite query --model /tmp/zincite092-followup/control.mzn \
    'reduce_enum("Guests", keep("C", "A"))' \
    /tmp/zincite092-followup/original.dzn
```

Both exited 0 with byte-identical complete candidate bytes and empty stderr.
Direct structured inspection established enum order A,C, exactly matching keyed
records, pruned nested friend sets, unchanged interest arrays, and an unchanged
string containing B. The B-only group disappeared, retained singleton groups
remained, and the written `groups[3]` reference became `groups[2]`. Unrelated
Topics and capacity fragments were byte-identical. Comments on surviving entries
remained; comments attached to removed records/groups disappeared.

Each command then ran `--write` on its own temporary original copy. Both exited
0 with empty stdout/stderr, wrote exactly the corresponding preview bytes and
preserved mode 0640. MiniZinc 2.10.1 build 33348285743 accepted the original and
both preview/write candidates with `--solver gecode --instance-check-only` (five
checks, exit 0 and empty streams). No solving occurred. These compiler checks
supplement the direct transformation checks.

The two named private wedding inputs were copied to temporary files solely to
check incomplete write refusal. Both command forms returned status 1, empty
stdout, and the same four located computed enum-indexed dependencies in the
read-only model; all four temporary copies remained byte-identical. The model
and original inputs still match the previously recorded SHA-256 hashes. Actual
preview/compiler/data/performance evidence remains the retained
`/tmp/zincite092/` evidence; it was not rerun.

This supplies the complete structured-control/write evidence and explicit
real-preview write-refusal evidence required by the revised contract. The
retained actual private-preview compiler/data/performance checks remain
applicable; this follow-up did not replay them. The independently reviewed
base-101 repair and final navigation evidence below complete the normal-library
resource checks. Whole-task verification and task completion remain separate
workflow steps.

## Normal-library navigation after the use-consumer repair

Base-101 was independently reviewed and committed at
`e2e702c55c070f821ebf25763485c839df43f1a7`. Final parent acceptance rebuilt
`zincite` and `zincite-query` in release mode at that commit. The focused repair
uses existing binding identities, candidate sets and normalized names to avoid
resolving references proven unrelated to a nonempty declaration selection. It
preserves potentially relevant unresolved/ambiguous overload outcomes, conservative
empty-selection behavior, upstream/model limitations, source identity and ranges,
per-declaration written order and duplicates. It leaves the public callable
resolver and finite defaults unchanged.

The independent base-101 verifier ran these paired recipes with the installed
MiniZinc 2.10.1 build 33348285743 library:

```sh
stdlib=/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc
zincite-query --stdlib-dir "$stdlib" \
    'filter(name("capacity")) | declarations | uses | json' \
    /tmp/zincite092/recipes.mzn
zincite query --stdlib-dir "$stdlib" \
    'filter(name("capacity")) | declarations | uses | json' \
    /tmp/zincite092/recipes.mzn
```

Both exited **0** with byte-identical complete JSON, no limitations and empty
stderr. The single result is the written `capacity` reference at bytes 63..71
in the constraint, resolved to the declaration at bytes 5..13. Paired
`filter(name("capacity")) | declarations | json` and
`filter(name("capacity")) | declarations | types | json` likewise exited 0
with equal complete output and empty stderr; the declaration has parameter
`int` type. Final parent acceptance additionally ran
`filter(name("capacity")) | declarations | instantiations | json` through
both forms at the committed release build: status 0, equal complete JSON, no
limitations, empty stderr and `par` with no unknown reason.

All use default nesting 64, work 1,000,000 and collection 100,000, without a CLI
limit override. The independent verifier retained both finite refusals: the
100,000-element filter returned status **2**, empty stdout and a collection error
at query bytes 25..48; repeated declaration/use stages returned status **2**,
empty stdout and an actual uses-stage work error at bytes 239..244. Focused public
checks cover selected overload and shadowing identity, relevant ambiguity and
unresolved references, upstream unknown/model limitations, empty-selection
compatibility, ordering/duplicates, BOM/CRLF ranges and pre-allocation admission.

Independent complete streams and the passing base-101 review are retained in
`/tmp/zincite101/verifier-evidence.json`, `verifier-stagework-evidence.json` and
`verifier-result.json`. Final instantiation streams and original-hash confirmation
are in `/tmp/zincite092-followup/final-navigation-evidence.json`. The private model
and original input hashes still match their recorded values. The unchanged
private previews, direct-data/compiler checks, complete public reduction/write
control and representative/growing-data measurements were retained without
replaying private models or duplicating compiler/performance checks. No solver,
corpus sweep or formatter-save recheck was performed; shared syntax/layout was
unchanged. No new performance improvement or full semantic coverage is claimed.

Final parent validation passed `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` and `git diff --check`. No new tests were added for this
report update; the reviewed child repair supplies focused public regressions.
The report covers the implemented inspection, structured edits, reduction and
semantic navigation stages while retaining best-effort dependencies and finite
refusals. The two named real reductions remain incomplete status-1 previews
with unavailable writes; their computed enum-index evaluation is excluded by
the adopted contract. This report does not assert full MiniZinc evaluation,
satisfiability or completion of the separate corpus/thesis gate.

## Shared multi-feature reports, 2026-10-13

The successful syntax and semantic programs cost less than their matched separate
invocations, reusing input parsing and requested binding facts. The 10,000-element
literal report rejects under default limits; its timings describe refusal costs.

The consumer source is `2efee89986860f7c898537f79026dabfaadfb524` and the exact
Zirql Git dependency is `9c3ce81dca3d35cde55b2fba77567d73e431ae54`. The release
root command reports `zincite 0.1.0 (2efee8998686)`. Both binaries come from:

```sh
cargo build --release --locked --target-dir /private/tmp/zincite128-build -p zincite -p zincite-query
```

Full binary, source, program and loaded-dependency hashes are in `environment.json`
and `report.json` under `/private/tmp/zincite128-measurements-20261013`. The release
`zincite` SHA-256 is
`4b829d631b2f05827a0a046be260a02d6907007e49815c1a426e9f9762e59812`;
`zincite-query` is
`6c8134a42a9a489680b7619eb31f30b51878f9ea20ab6afd3ed3cb680d628aa6`.
The standalone command does not implement `--version`.

The host is an Apple M1 Max MacBook Pro with 10 CPU cores and 64 GB memory,
arm64, macOS 26.6.2 (25G83), and Rust 1.98.1 (`48a229cea`, 2026-09-01).
The semantic case explicitly uses MiniZinc 2.10.1 build 33348285743 at
`/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc`. It loads
179 files totalling 452,118 bytes, including the 88-byte root and 178 standard
library files. No solver or compiler instance check is part of this run.
All programs retain default work 1,000,000, results 100,000, nesting 64 and
stream items 100,000.

### Inputs and matched reports

`syntax-cookbook` is the exact 253-byte `model.mzn` in
[the query cookbook](../docs/query-cookbook.md#create-the-inputs), with LF and a
final LF. Its SHA-256 is
`26c2e89def652fe4d2e646205184ccc2d10b9d3472bb639a2f02337d2646ab06`.
Its written include is not loaded for this syntax report:

```text
source selected = query("filter(kind(\"constraint\"))");
source n = selected | query("count");
source histogram = selected | query("expressions | call_names | tally");
report (count: n, histogram: histogram, selected: selected);
```

The result has count 3, histogram `{"abs":2,"bool2int":1,"forall":1}` and
all three root constraints with their exact text, file and byte ranges.
Each control reports one field and keeps only that field's binding dependencies.

`semantic-capacity` is this 88-byte public model, with SHA-256
`e69532cc4da408b8f4d5added90006f2f2cbb6835f6b7e3adc08101878e5fc19`:

```minizinc
int: capacity = 2;
var int: occupancy;
constraint occupancy <= capacity;
solve satisfy;
```

```text
source selected = query("filter(name(\"capacity\")) | declarations");
source types = selected | query("types");
source instantiations = selected | query("instantiations");
source uses = selected | query("uses");
report (selected: selected, types: types, instantiations: instantiations, uses: uses);
```

Its four controls keep `selected` and, where needed, the one derived binding.
The result contains the `capacity` declaration at bytes 5..13, its parameter
`int` type, `par` instantiation and written constraint reference at bytes 63..71.
All reports are complete with no limitations. Native IDs are compared within their
root owner; file/range/source identify observations across independently loaded
owners. Numeric IDs are context-local.

`literal-weights-10000` is generated as:

```python
('weights=[' + ','.join(str((-1, 0, 1)[i % 3]) for i in range(10000)) + '];\n').encode()
```

It has 23,345 bytes and SHA-256
`12e42069caeb054341f33a2664eabbe5efb7c8a57795f8cbb679b148484e8a2f`.
There is one assignment named `weights`, containing 3,333 positive scalar ones:

```text
source n = query("count");
source names = query("names");
source weights = literal("weights");
let value_count = n;
let value_names = names;
let positives = [v | v in weights where v > 0];
report (count: value_count, names: value_names, positives: positives);
```

The count control keeps `source n`, `let value_count` and its single-field
report; names keeps its corresponding source and pure binding. Positives keeps
only the literal source, comprehension and report. Explicit `let` bindings give
count and names the same constructed tags in the main report and controls.
Bare native controls would have different field shapes.

The main program and positives control both exit 2 with empty stdout and
`result limit exceeded`, at the generator binder `v`: main program bytes
162..163 (line 6, column 22), control bytes 58..59 (line 2, column 22).
Count and names controls exit 0, reporting constructed integer 1 and the
positional string array containing `weights`. The rejected program returns no
`ProgramResult`: its 3,333-element output and field parity remain unavailable.
The positive count above comes from inspecting the input.

A separately named `literal-weights-1000-companion` uses the same generator
with `range(1000)`, 2,345 bytes and SHA-256
`19b9638d856ada07cdccb91033feab83f93b624c041665aeaac1e4f799a34838`.
With the same programs and defaults, complete native/rendered comparisons succeed:
count 1, names `weights`, and 333 integer ones on positional keys 1..333 in source
order. This shape control does not establish the original case's acceptance or an
exact cutoff.

Both CLI forms were captured once for every main and control program, including
the companion. Full stdout, stderr and statuses match for each paired invocation.
Comparisons preserve envelopes, limitations, native kinds, order, duplicates,
source text, file/ranges and axes. Only the controls' outer record wrappers are
assembled; their fields and limitations stay intact. Main/control stdout
bytes are 833/925 for syntax and 1,367/1,505 for semantics, where control bytes
are summed over separate outputs. Original literal bytes are 0/403, with the
positives error retained separately. Companion bytes are 36,505/36,651.

### Fresh command costs

Ten main processes and ten sequential control groups per named case ran in one
coordinated quiet window. Odd repetitions ran main then controls; even repetitions
ran controls then main. Controls stay in report-field order. Each child has a
45-second deadline and complete file captures. All 60 sample records and 130
children finished; there were no timeouts, unfinished samples or incomplete
results. Syntax and semantic main/control groups each have ten complete outcomes.
Original literal groups each have ten rejected outcomes: their count and names
children succeed, while all ten positives children reject.

The existing `scripts/bench-save.py::file_invoke` is unchanged. Its exact-child
`os.wait4` supplies CPU time and peak RSS; the small established `waitid` bridge
keeps the child reserved for the sole reaper. RSS is reported in bytes on macOS
and converted to MiB below. Main wall time spans child launch through completion.
A control group's wall time spans all sequential captures, including capture
bookkeeping between children; the component times remain in the raw records.
Group RSS is the maximum child peak, not a sum of nonconcurrent peaks.

| Case and variant | First sampled wall ms | Median wall ms (min..max) | Median peak RSS MiB (min..max) | Outcome |
| --- | ---: | ---: | ---: | --- |
| syntax-cookbook, program | 5.09 | 5.03 (4.56..14.49) | 2.44 (2.41..2.45) | complete, 10/10 |
| syntax-cookbook, separate controls | 15.22 | 15.29 (14.43..25.65) | 2.38 (2.36..2.41) | complete, 10/10 |
| semantic-capacity, program | 60.43 | 58.57 (56.83..60.43) | 26.20 (25.09..26.97) | complete, 10/10 |
| semantic-capacity, separate controls | 174.06 | 176.53 (170.03..190.48) | 25.85 (24.91..27.11) | complete, 10/10 |
| literal-weights-10000, program | 13.48 | 13.04 (12.47..13.90) | 9.26 (9.19..11.84) | rejected, 10/10 |
| literal-weights-10000, separate controls | 28.73 | 28.31 (27.26..32.43) | 10.09 (9.14..12.22) | rejected, 10/10 |

These are first samples after build and correctness probes, with ordinary warm
filesystem caches. They are not controlled cold I/O or first-ever binary use.
Ten observations support this bounded comparison, not p95 or a formatter-save
budget claim. The earlier acceptance and base-122 measurements use different
workloads or commits and are not matched regression baselines.

### Retained input and requested facts

Three uninstrumented processes per case parse Input/main Program/controls once,
then record first and ten repeated requests. Semantics adds ten fresh ModelFacts
owners against the same loaded context. The companion has three API observations
and no ten-sample CLI campaign. Input bytes and syntax/tree addresses stay unchanged;
all successful first/repeat/control native fields and rendered envelopes agree,
including fresh semantic owners within their loaded context.

The following medians are milliseconds. Parse/load and first-request columns
have three observations; repeat and fresh-owner columns contain 30 requests
nested within those three processes. Full min/max spreads and every phase row
are in `report.json`.

| Case | Input / main / controls parse | Model load | First evaluate / render | Repeat evaluate / render | Fresh owner evaluate / render |
| --- | ---: | ---: | ---: | ---: | ---: |
| syntax-cookbook | 0.046 / 0.040 / 0.024 | not needed | 0.045 / 0.020 | 0.012 / 0.009 | not needed |
| semantic-capacity | 0.032 / 0.048 / 0.030 | 20.857 | 28.813 / 0.036 | 1.492 / 0.021 | 26.856 / 0.031 |
| literal-weights-10000 | 1.317 / 0.091 / 0.040 | not needed | 5.884 / unavailable | 5.389 / unavailable | not needed |
| literal-weights-1000-companion | 0.181 / 0.084 / 0.038 | not needed | 0.912 / 0.561 | 0.792 / 0.445 | not needed |

Input/program parse timers exclude file reads. Model loading includes its source
reads and parsing. Evaluation and one render are timed separately; native/output
comparisons run outside those timers, and each result/output is dropped before
the next request. Fresh-owner timing excludes the small Rc/ModelFacts constructor.
Literal-10000 evaluation rows end in rejection, so render timings are unavailable.

The semantic ProgramResult retains the supplied ModelFacts owner and the same
borrowed BindingFacts across requests. This declaration's type and `par`
instantiation are supplied by bindings: `SemanticResult::instantiation_facts()`
returns None because the deeper occurrence-instantiation family is not demanded.
A narrow direct `Query::evaluate_with_facts` probe for the identical declaration
instantiation pipeline confirms the same borrowed bindings and None deeper facts
on ten requests in each of the three corrected processes. No unused family is
initialized for the final observations. Internal parser or fact-producer call
counts are not instrumented; public identity demonstrates retained input and
borrowed-result reuse, not free evaluation or rendering.

An original pointer check forced `ModelFacts::instantiations()` after the first
semantic evaluation and in the independent-root allocator probe. Those observations
are retained but superseded by a separate corrected driver, one allocator run and
three semantic API processes in a narrow quiet window. CLI samples and unaffected
API rows were not repeated. An earlier panic on the optional absent accessor remains
recorded as an observer failure.

### Allocation and root lifetime

A separate CountingAllocator build records requested/live heap bytes, allocation
calls and cumulative requested bytes. It adds no allocator overhead to the timing
builds. Baselines include already retained probe/report buffers; JSON for each
measured scope is built afterwards. After every repeated result/output drop,
live bytes are stable, and every case returns to its baseline after Input,
Program, facts and context are dropped. A rejected result holds only its error;
it has no rendered report.

| Case | Input + Program retained bytes | After model load | First result/error + output retained | After repeated drops | Peak live heap bytes | After complete drop |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| syntax-cookbook | 11,801 | 11,801 | 14,433 | 11,801 | 23,272 | 0 |
| semantic-capacity | 7,012 | 9,595,632 | 21,788,790 | 21,786,368 | 22,978,793 | 0 |
| literal-weights-10000 | 1,904,903 | 1,904,903 | 1,904,924 | 1,904,903 | 6,059,065 | 0 |
| literal-weights-1000-companion | 214,784 | 214,784 | 276,035 | 214,784 | 1,207,859 | 0 |

For the semantic case, dropping the caller's owner after results are released
reduces retained bytes from 21,786,368 to 9,693,488; dropping the ModelContext
leaves only the 7,012-byte parsed input/program allocation delta. The first demand
adds retained binding facts; fresh owners rebuild them. Loading and first binding
construction account for most of this case's fresh-process cost. Repeated requests
still traverse and copy their selected results and render a new output.

Two simultaneously loaded capacity roots have identical 88-byte source but
different paths, distinct ModelContexts, ModelFacts and borrowed bindings. Each
retained result keeps its root's owner alive after the caller Rc is dropped.
Both Weak pointers stop upgrading after their final result is dropped; dropping
the Weak removes the small remaining control block. The contexts outlive their
results. Their staged live-byte deltas are:

| Drop boundary | Live bytes above baseline |
| --- | ---: |
| Both contexts loaded | 19,186,467 |
| Both first results and outputs retained | 43,572,867 |
| Both outputs dropped, results retained | 43,570,109 |
| First caller Rc dropped | 43,570,109 |
| First final result and Weak dropped | 31,476,174 |
| First context dropped, second root live | 21,789,698 |
| Second caller Rc dropped | 21,789,698 |
| Second final result and Weak dropped | 9,695,703 |
| Both contexts dropped | 9,209 |
| Both Inputs and Program dropped | 0 |

Each root also has ten repeated result/output drops while its first result remains
live, with stable retained bytes. The subsequent ten fresh-context requests
alternate the two paths; every complete result/owner/context drop returns exactly
to the retained Input/Program baseline. No owner is shared across roots. Repeated
loading retains 358 files / 904,236 source bytes; unique canonical paths contain
180 files / 452,206 bytes. The duplicated 178 standard-library files explain the
additional independent-root storage.

Counters exclude stack, allocator metadata, other native allocations and residency.
RSS is a process high-water value and need not fall on a drop. All allocation-call,
requested-byte and Weak rows remain available. There is no observed accumulation
or matched prior-version regression requiring another profile; the timing spread
does not establish a product regression.

The original literal refusal warrants one direct bounded follow-up: inspect
produced/copied result charging for this exact 10,000-element comprehension and
its positives-only control before deciding on an implementation change. The
recorded refusal occurs during evaluation at the generator binder. This run does
not identify each internal charge or justify relaxing defaults, adding a cache
or changing semantic behavior. The successful companion establishes shape parity
at its own size only.

### Retained evidence and validation

Zincite owns this run; Zirql base-010 reuses these artifacts instead of repeating
its cases. Everything except this report remains temporary under
`/private/tmp/zincite128-measurements-20261013`: `cases.json`, all inputs/programs,
`parity.json`, `companion-parity.json`, complete `.stdout`/`.stderr` streams,
`runs/shared-reports/` with configuration and per-sample records, `environment.json`
and `report.json`. The original `observe.rs` and corrected `observe-demand.rs`,
compile commands/logs, `api-observations.json`, `api-demand-observations.json`,
`memory-observations-current.json` and `memory-demand-observation.json` distinguish
original and corrected probes. Earlier failures and superseded observations remain
in their own files. `report.json` selects the final evidence explicitly.

The temporary runner saves named samples atomically, resumes unfinished work and
rejects changed configurations. `capture-smoke.json` records preview, bounded
collection, unchanged completed records on resume, configuration rejection, timeout
capture and analysis without target binaries. Start/resume and analyze with:

```sh
python3 /private/tmp/zincite128-measurements-20261013/capture.py run --name shared-reports --repetitions 10 --timeout 45
python3 /private/tmp/zincite128-measurements-20261013/capture.py analyze --name shared-reports
```

The executable script also has the uv script header and a working uv entry point
using a temporary cache. Child arguments and cwd are explicit. Disposable Rust
drivers pass rustfmt and compile with `-D warnings`; the waitid bridge compiles
with `-Wall -Wextra -Werror`. Final input, source, dependency and binary hashes
match the captured versions, and `git diff --check` passes. No repository Rust,
manifest, tests or measurement service changed; this report needs no new product
tests or repeat workspace Rust checks. Earlier rejected and incomplete acceptance
results above remain unchanged.

## Model-backed data batches

The base-127 implementation adds atomic `create`/`replace` programs ending in
`emit_document;`. Validation used the approved public two-entity model and data
from `crates/zincite-query/tests/fixtures/`, on the source checkout over
`a8cc0cd66f6ca7cfa44a4055b4b82c767a1f692c`, with Zirql pinned at
`9c3ce81dca3d35cde55b2fba77567d73e431ae54`. No solver or corpus sweep ran.

The library, `zincite-query -f double.zq` and `zincite query -f double.zq`
produced the same 257-byte candidate. Its SHA-256 is
`d1c0bbbd134e55f59c0e658a4c6539b6b38ef2f706710f803770a2bd7fcee01c`.
A subsequent public literal report checked entity keys `1..4`, weights
`7,11,7,11`, peers `2,1,4,3`, groups `1,1,2,2`, all eight capacity keys in
`1..4 × 1..2` with values `3,4,5,6,3,4,5,6`, and group keys `1..2` with
memberships `{1,2}` and `{3,4}`. Both evaluations retained the supplied
`Rc<ModelFacts>` owner. Sources and `let` bindings read the original input;
final target axes used the generated counts.

Both command forms passed source preview, `--diff` and explicit `--write`.
Previews left the original bytes intact; writes matched the library candidate
and retained permissions. A failed second replacement returned status 2 with
empty stdout and no write. Incomplete model context returned status 1 and
refused writing; stdin and symlink writes returned status 2. Model bytes stayed
unchanged. The native checks used a temporary library containing an empty
`std/stdlib.mzn`, sufficient for this model's fixed literal declarations.

MiniZinc 2.10.1 build 33348285743 accepted the original, the documented doubled
data and the actual generated candidate with `--instance-check-only`. These
checks returned status 0 with empty diagnostics. They supplement the explicit
key and reference checks; they do not establish satisfiability.

Focused library controls cover existing and initially empty data documents,
array/matrix/record replacements, nominal member spelling, known empty axes,
BOM/CRLF and detached comments, written integer scalar domains, conflicting
targets, unsupported or computed values, internal RHS comments and stale final
scalar-dependent coverage. Existing report and legacy edit controls also pass.
The final source passed workspace formatting, clippy with warnings denied,
workspace tests and `git diff --check`.

Raw commands, statuses, output files and binary/input hashes remain under
`/private/tmp/zincite127-evidence/`. `proof-final.json` records the library and
15 CLI/compiler controls; `mapped.json` retains the public typed report, and
`validation-final.json` records final workspace commands and source hashes.
The earlier logs are retained separately and do not describe the final
bounded-domain addition. Baseline compiler evidence is in
`/private/tmp/zincite127-public-baseline/proof.json`.

## Shared legacy edit adoption

The base-010 acceptance uses Zincite `3b9869ab96bbc838cdc33539aee0407b84e52af5`
plus the terminal-result adapter change, with Zirql pinned at
`cef521687d81a1f23ab909cf2ea2e7d54a52784c`. The shared evaluator owns every
legacy query's common stages, including the selection leading into an edit.
Zincite retains `Document` and `Reduction` candidates in its pipeline context;
assignment replacement, collection filtering, enum reduction, candidate
validation and explicit writes remain native host operations. Optional model
interpretation for enum reduction does not turn a data request into semantic
model inspection. Program `create`/`replace` continues to use the existing
shared report/data comprehension evaluator and coherent-candidate checks.

Seventeen current main/control pairs, through both rebuilt CLI forms, reproduce
the complete stdout, stderr and statuses of all four base-128 cases above.
The 10,000-element literal program and positives control still reject at the
same binder ranges; the 1,000-element companion still succeeds. These are
current compatibility captures, not new timing samples. The base-128 timings,
corrected demand observations and allocation scopes retain their original
consumer `2efee899` / core `9c3ce81` revisions and environment labels.

Eleven legacy recipes compare the preserved baseline and rebuilt commands:
replacement and removal, empty selection, element filtering, model-only enum
reduction, preview/diff/write, comment and stage refusals, and program-only
limit-option refusals. Full output, status, resulting file bytes and permissions
agree for each command form. Usage errors keep each command's own name prefix.
Ten public library observations against a rebuilt baseline additionally compare
native candidate kinds, emitter aliases, written predicate depth versus literal
depth, exact work/collection/nesting error locations and semantic-edit refusal.
Original input bytes remain unchanged by evaluation.

The existing focused query and CLI coverage passes, followed by workspace
formatting, warning-fatal Clippy and workspace tests. Candidate constructors and
the approved data grammar are unchanged, so base-008's accepted native,
MiniZinc instance-only and atomic-write evidence remains applicable. No solver,
full measurement rerun, dependency update or publication is part of this check.
Raw current captures, baseline/library build commands and hashes are retained
under `/private/tmp/zirql010-author-evidence`; the historical base-128 artifacts
remain read-only at their original location.

## Explicit worklists and reducers (2026-10-15)
The declaration graph and destructured fold/scan fixtures pass through text,
`Program::from_values`, both query commands, inline programs and program files.
The graph returns true for A→A and A→D and false for A→E, including its cycle
and duplicate route. Native target inspection returns B then C. The identity
comparison and identity-set membership agree. Native queue aliases and earlier
scan states retain their original owners and lengths. Focused controls cover
uncertain and ambiguous adjacency, a known intrinsic endpoint, BOM locations,
wrong elements, empty-input reducer validation, simultaneous state updates,
nested transfers, tail-call exits, limits and output buffering. Rendering a
queue/set directly or inside a record refuses before CLI stdout is committed.
The reducer check also confirms that annotated tuple results retain the `tuple`
JSON tag rather than being tagged as sets.

The inspected product source is Zincite
`9eebe3c2d69bdba201d3d1dc523a2aaa1c45bb72`, based on
`8e2b14982efd4c91337facb2bcbab047ddcc7acb`. Its exact Zirql Git pin is
`ea30035e010cea2e12047a1aa4e5bf39a3ac983e`; the baseline uses
`6734a8535702298ad257203c69fec88ce3a9c6d6`. No solver, rewrite effect, release
or publication runs. The frozen source passes workspace formatting, Clippy with
warnings denied, all workspace tests, and the source site's check/build.

### Measurement method and identities

The temporary probe uses the existing counting `System` allocator from the
formatter phase probe and the existing `scripts/bench-save.py::file_invoke`
exact-child `wait4` wrapper. Both variants use release builds, Rust 1.98.1,
identical locked transitive versions apart from the intended Zirql pin, and
matching allocation counters. Measurements ran serially in a coordinated quiet
window on an Apple M1 Max (10 CPU cores, 64 GB RAM), arm64 Darwin 25.6.0.
Allocator counters add overhead; these are instrumented comparisons, not
production CLI latency claims.

The final capture contains 231 complete records: one separately retained
first-use observation and ten measured processes for each of 21 cases. The two
small queries alternate baseline/candidate order. Each small process batches 100
queries; its phase result is a per-query mean, and the table summarizes the ten
process means. The growing cases execute once per process. All 231 outputs pass
their expected-result/refusal checks: 198 successes and 33 separate refusals,
including first-use observations. There are no timeouts or unfinished records.

`parse` times `Program::parse`, including CST lowering and syntax preflight.
`evaluate` includes request preparation, native admission, demanded model facts
and shared execution; the public API does not separate preparation from execution.
Input parsing and model loading occur outside those phase timers, but inside the
whole-process wall/RSS boundary. `render` is measured separately for renderable
reports. Native alias/scan cases retain library results and inspect them after the
evaluation timer; rendering is deliberately absent from their throughput samples.
Every raw record includes process argv, exact input/query hashes, full captured
streams, status, phase/allocation metrics and result-drop telemetry. Peak RSS is
the exact child's Darwin high-water value, converted from bytes to MiB.

The baseline probe binary SHA256 is
`672d981c73f9aa1ae0ed690763d3b3b0a8cc0f115f637b1ddc22565ff9b7aff7`;
the candidate probe binary SHA256 is
`2c39e707bb29e4a7add3bd6bc314a4baf90326a3e514faa82622dbf94e345c7c`.
These binaries contain temporary measurement code and are distinct from shipped
commands. Their shared probe source SHA256 is
`3a19a14355bc5bffd8794094dd351057f56296d59b510bc61ff928e230fca376`.
The local evidence is under
`target/benchmarks/base046/runs/final/`, with the resumable driver in
`target/benchmarks/base046/measure.py`. The complete saved identity, command and
environment record is `/private/tmp/zirql046-measurement-identities.json`.
`run --name final` resumes unchanged work, while `analyze --name final`
regenerates statistics from saved records. A disposable smoke run confirmed
unchanged completed records, rerunning one missing record with partial streams
retained, rejection of a changed configuration, and analysis without execution.

### Small-query comparison

Times are microseconds per query: median [minimum–maximum] over ten process
means. The tiny report sums three integers. The eight-read report repeatedly
reads the same 64-element binding. Both variants produce identical output.

| Case | Variant | Parse µs | Prepare/evaluate µs | Render µs |
| --- | --- | ---: | ---: | ---: |
| tiny | baseline | 7.290 [7.151–8.001] | 5.024 [4.920–5.276] | 2.620 [2.480–2.880] |
| tiny | candidate | 7.292 [7.164–7.695] | 5.047 [4.886–5.234] | 2.467 [2.408–2.530] |
| eight_reads | baseline | 40.033 [39.126–41.883] | 35.060 [34.619–36.684] | 15.231 [14.354–16.421] |
| eight_reads | candidate | 40.551 [40.313–42.804] | 34.553 [34.461–36.492] | 14.592 [14.369–15.017] |

The phase ranges overlap. Tiny evaluation changes by +0.023 µs (+0.46%);
eight-read evaluation changes by −0.507 µs (−1.45%). These observations do not
establish a meaningful small-query speed change.

### Growing native work

Successful rows below explicitly enlarge work to 100,000,000 and aggregate
results to 20,000,000. Nesting stays 64 and the stream ceiling stays 100,000.
Defaults remain 1,000,000 work and 100,000 results. The wide graph has N leaf
declarations, A pointing to every leaf, every leaf pointing back to A, and
unreachable E. It therefore has N+2 declarations and 2N written edges. The
input files contain 3,413 / 17,813 / 73,815 bytes. Other cases use N syntax-node
items in 1,280 / 7,280 / 31,780-byte files. Queue alias retains the original
queue while draining another view. Set alias retains a populated set while
performing duplicate insertions. Scan retains all N queue snapshots.

Evaluation times are milliseconds, median [minimum–maximum] over ten processes.
Requested allocation bytes count traffic during preparation/evaluation, including
freed temporary storage; peak RSS covers the complete process.

| Case | N | Prepare/evaluate ms | Requested MB | Maximum RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| wide_cycle | 100 | 2.516 [2.493–2.555] | 2.226 | 5.89 |
| wide_cycle | 500 | 12.525 [12.347–12.936] | 10.598 | 8.62 |
| wide_cycle | 2000 | 67.021 [66.636–69.527] | 41.662 | 19.22 |
| queue_alias | 100 | 0.612 [0.606–0.667] | 0.565 | 4.58 |
| queue_alias | 500 | 2.365 [2.333–2.471] | 2.622 | 5.42 |
| queue_alias | 2000 | 8.814 [8.728–9.007] | 9.639 | 7.78 |
| set_alias | 100 | 1.272 [1.240–1.339] | 1.327 | 4.89 |
| set_alias | 500 | 5.471 [5.407–5.642] | 6.327 | 6.77 |
| set_alias | 2000 | 21.646 [21.293–23.593] | 24.385 | 13.64 |
| scan | 100 | 0.495 [0.486–0.521] | 0.607 | 4.66 |
| scan | 500 | 1.864 [1.847–2.007] | 2.902 | 6.44 |
| scan | 2000 | 6.965 [6.876–7.184] | 10.842 | 11.98 |

The queue-drain allocation traffic grows with input size while its original
queue remains retained. Scan similarly retains lengths 1 through N without
changing earlier states. Source inspection confirms that optional-pop tuples
first enter `admit_computational`, which checks the owner and element metadata
without enumerating the remaining queue. Host copies share computational roots.
This combination gives allocation and native-behavior evidence against copying
or rescanning the remaining queue at every pop; it is not a timing assertion in
the permanent tests.

All queue/set alias and scan evaluation-retained bytes are freed when their
result drops. For example, scan at N=500 retains and frees exactly 304,552 bytes.
The wide graph retains an additional 1,024 / 4,096 / 16,384 bytes after result
drop. Those amounts match the lazy line-index capacities owned by the retained
model's parsed source, outside the result. A separately labelled control that
preconstructs binding facts before evaluation removes this difference: its
evaluation-retained/drop-freed amounts are exactly 2,056 / 8,456 / 32,456 bytes.
The probe's top-level live-byte field also includes accumulated telemetry and
must not be read as result storage.

### Host selection cost and refusals

The wide query's initial `declarations | filter(name(...))` selections each
convert every item to declarations. The existing host conversion scans the
retained declaration table for each syntax-node subject, before the loop starts.
That node-by-declaration cost remains in the reported timings. Binding-fact
construction alone is 0.132 / 0.518 / 2.025 ms in a five-process-per-size
attribution control, so it does not explain the extra largest-size cost.

A second bounded control uses the same frozen candidate binary, graph, limits
and output, moving only each initial name filter before `declarations`. Five
samples per size give 2.440 / 11.444 / 44.700 ms evaluation medians, compared
with 2.516 / 12.525 / 67.021 ms above. This isolates the broad initial selection
cost. The implementation preserves existing selection semantics. These graphs
use proven value references; they do not measure overload-heavy callable
resolution. The raw attribution records are
`/private/tmp/zirql046-attribution.json` and
`/private/tmp/zirql046-selection-attribution.json`.

Default allowances are measured separately at N=2000. Wide reachability refuses
with `query work limit exceeded`; the set-alias computation refuses with
`result limit exceeded`. Queue alias and scan complete under defaults and keep
their separately labelled successful rows. Direct nested computational rendering
refuses at the responsible query range. Every refusal has ten measured samples
plus its first-use observation and is excluded from successful throughput
statistics. The existing broad initial declaration conversion remains a named
host scaling concern; the worklist runtime and defaults are unchanged.
