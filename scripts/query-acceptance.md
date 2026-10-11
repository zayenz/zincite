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
