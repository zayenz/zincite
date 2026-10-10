# Query acceptance, 2026-10-11

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
Resolving those accesses is an acceptance gap; it requires dependency analysis
beyond this validation task.

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
the model-fact collection limit and exited 2 with empty stdout. This resource
gap prevents claiming complete semantic-query acceptance with that library.
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
