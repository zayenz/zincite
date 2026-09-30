# Save-path performance benchmark

Build the real formatter and run the Python standard-library driver from any
working directory:

```sh
cargo build --release -p zincite-fmt
python3 scripts/bench-save.py
```

The driver measures a fresh `zincite-fmt --stdin-filepath PATH` process for every
sample. Its timer surrounds process creation, stdin transfer, complete stdout and
stderr capture, and process completion. Input construction, hashing, summary
calculation and native RSS probes are outside the measured interval. Nothing
writes the input files. Reports default to ignored `target/benchmarks/`; an
existing report path is refused. `--output PATH` selects a local report.

The named cases are deliberately small:

| Case | Input | Purpose |
| --- | --- | --- |
| ordinary-changed | Existing `tests/fixtures/integration.mzn` | Comments, declarations, collections and control expressions |
| ordinary-formatted | Output from the first ordinary invocation | Save an already formatted buffer; output must equal input |
| invalid-edited | Malformed declaration after a valid constraint | Expected exit 2, diagnostics, no partial source |
| matrix | Synthetic 20 × 12 values | Matrix alignment |
| matrix-grown | Synthetic 200 × 12 values | Matrix growth |
| nested / nested-grown | Forty / 160 nested conditional expressions | Nested layout and fit growth |
| dense-10000 through dense-2000000 | `values=[0,1,...,99,0,...];`, scale/3 entries | Token/CST density, three near-decade sizes and a >1 MiB growth point |

The report records exact bytes and input hashes rather than approximate scale
names. `--external PATH` adds a read-only model/data case; repeat it for a few
explicit representative files. No external contents are copied into the repository
or report. Case IDs use basenames; commands and local reports may contain paths,
so review a report before sharing it. An external parse failure remains a failure,
even when it returns quickly. This driver is not a full corpus acceptance check.

Each case alternates two settings variants for 50 samples each: controlled
default values, and three nested EditorConfig files with realistic matching and
explicit values equal to the defaults. Both temporary trees have `root = true`
to exclude incidental host settings; the default variant still performs the
save path's filesystem lookup. Plain-stdin timing is not substituted for this
comparison. There is no result cache or persistent formatter process.

The first driver invocation is retained separately before deriving the formatted
case. It is first use in this run, not necessarily the binary's first launch and
not controlled cold filesystem I/O. Repeated samples use ordinary warm filesystem
caches. Stop other heavy work while measuring. The report retains raw timings,
statuses, bounded diagnostics and output hashes, plus Unix child CPU time, p50, nearest-rank p95,
min/max, throughput and interleaved pair batch wall time. Fifty samples describe
this run's distribution; they do not establish confidence bounds.

Peak child RSS comes from a separate `/usr/bin/time` invocation with output still
captured. Darwin `-l` reports bytes; Linux `-v` reports KiB. The report retains the
native units, converted MiB and probe status. An unsupported or blocked native
probe is explicitly unavailable; it is never a zero-memory result. Probe overhead
is outside the save latency samples. On macOS the sandbox may deny `kern.clockrate`
or profiling; use an authorized unsandboxed measurement rather than inventing RSS.

For a bounded smoke check, use:

```sh
python3 scripts/bench-save.py --samples 1 --case ordinary-changed --case invalid-edited --output /tmp/zincite-smoke.json
```

Counts below 50 are smoke evidence only. `--timeout SECONDS` bounds each process;
timeouts and unexpected statuses remain failures. Successful invalid-buffer
responsiveness means the expected error status, not successful formatting.
Budget comparisons apply per case: up to 100 KiB, p95 ≤50 ms and RSS ≤32 MiB;
above 100 KiB through 1 MiB, p95 ≤100 ms and RSS ≤64 MiB. Larger inputs report
throughput, memory and growth rather than claiming the interactive budget.

## Developer phase probe

The small Rust example counts allocation/reallocation calls, requested bytes,
retained live requested bytes and the phase's peak above its starting live bytes.
Requested bytes include each realloc's complete new request, so they are traffic,
not retained bytes or OS RSS. System allocator metadata, page residency and stack
memory are excluded. Source copies are included in lex/parse; formatting keeps
the parsed CST live. The input buffer itself is read before phase measurement.

```sh
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release -p zincite-fmt --example profile-phases
target/release/examples/profile-phases /tmp/dense.dzn
# Repeat just one phase long enough to attach a native CPU sampler:
target/release/examples/profile-phases /tmp/dense.dzn format 10
# In another terminal, using the process ID above:
/usr/bin/sample PID 2 1 -file /tmp/zincite-format.sample.txt
```

Select `lex`, `parse` or `format` for repetition; omit the phase to report one
pass through all three. Parse includes lexing; format-only retains one parsed
input. Native CPU sampling separates phase stacks, including allocation and drop
work. Atomic counters perturb timings, so these phase times are attribution
probes and must not replace the uninstrumented CLI baseline. Profiling adds no
production instrumentation, allocator or dependency.

## Release baseline, 2026-09-30

Source revision `ed4f940d11b73d78dae5b73347a01bf51bef1d23`; Rust 1.98.1;
stock optimized release build; Apple M1 Max, arm64, 64 GiB RAM; macOS 26.6.2.
Production source was unchanged. Other heavy work was paused. Each row has 50
fresh-process samples per settings variant; all expected statuses were accepted.
The ordinary, matrix, nested and dense cases are generated or repository-owned.
The two external files are Challenge 2021 ATSP and java-routing `trip_7_4.mzn`,
read from an external archive without Git metadata; source hashes are in the
local JSON report. These two files do not establish corpus coverage.

| Case | Bytes | Default p50 / p95 ms | Default RSS MiB | Configured p50 / p95 ms | Configured RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| ordinary-changed | 1,163 | 8.06 / 10.42 | 1.97 | 8.06 / 11.34 | 2.02 |
| ordinary-formatted | 1,324 | 7.78 / 15.34 | 2.05 | 7.87 / 13.19 | 2.08 |
| invalid-edited | 33 | 8.11 / 13.94 | 1.75 | 8.40 / 12.52 | 1.78 |
| matrix | 676 | 8.09 / 10.90 | 2.09 | 8.16 / 11.46 | 2.12 |
| nested | 1,133 | 7.90 / 11.26 | 2.34 | 8.09 / 12.73 | 2.34 |
| dense-10000 | 9,669 | 9.37 / 11.31 | 3.36 | 9.75 / 12.42 | 3.38 |
| dense-100000 | 96,669 | 38.49 / 53.79 | 17.12 | 39.50 / 51.21 | 17.38 |
| dense-1000000 | 966,669 | 262.54 / 339.20 | 143.19 | 262.33 / 340.49 | 141.23 |
| dense-2000000 | 1,933,338 | 521.20 / 544.65 | 280.45 | 521.58 / 563.09 | 280.48 |
| external-0-atsp.mzn | 6,899 | 5.98 / 7.19 | 2.56 | 6.04 / 7.35 | 2.61 |
| external-1-trip_7_4.mzn | 85,229 | 22.01 / 23.20 | 10.44 | 22.07 / 23.14 | 11.20 |
| matrix-grown | 6,971 | 8.40 / 9.11 | 3.58 | 8.45 / 8.96 | 3.50 |
| nested-grown | 4,493 | 26.53 / 27.42 | 4.64 | 26.47 / 27.79 | 4.98 |
| dense-100000 (repeat) | 96,669 | 30.49 / 38.29 | 17.31 | 30.54 / 37.79 | 17.27 |

The complete local samples are `target/benchmarks/base022-release.json` and
`target/benchmarks/base022-growth-confirmation.json`. The second report adds the
matrix/nested growth points and repeats the marginal dense 96.7 KB result.
Its repeated p95 is below 50 ms, so the initial 51–54 ms miss is sensitive to run
noise and is not a confirmed regression. Dense 966.7 KB still substantially
misses both its 100 ms and 64 MiB budgets. From 966.7 KB to 1.93 MB, median
latency and fresh child RSS roughly double; this run shows no superlinear growth
at those points. The larger case has no interactive budget claim.

Default and configured medians are close; neither this comparison nor the small
invalid-buffer timings justify configuration caching or diagnostic indexing.
Invalid edited input returns exit 2 and no stdout. The already formatted case
returns identical bytes. Throughput and batch wall times, min/max and failure
counts remain available in the JSON rather than being pooled across cases.

A separate native cross-check on dense 966,669-byte input returned success and
2,633,338 output bytes: `/usr/bin/time -l` reported 0.26 s real, 0.22 s user,
0.02 s system and 149,635,072 maximum resident bytes (142.70 MiB). This agrees
with the captured save samples and the separate native RSS probes.

The primary run's first invocation was 8.01 ms. To examine the earlier first-use
outlier, two separate target directories were rebuilt without launching their
new binaries beforehand. Their first invocations were 424.25 ms and 280.77 ms;
the latter used only 3.78 ms child CPU. The next identical invocation was 4.71 ms
wall and 2.96 ms child CPU. The delay is predominantly waiting/launch overhead,
not repeated input-dependent CPU work. Its exact macOS I/O, security or scheduling
cause remains unestablished; task base-048 must retain this first-execution
limitation. This is not controlled cold-filesystem evidence. Supplemental raw
reports are `base022-fresh-first-use.json` and `base022-first-use-cpu.json` under
`target/benchmarks/`; child CPU fields were added after the primary run.

### CPU and allocation attribution

The separate counter probe for 966,669-byte dense input produced these stable
allocation counts. It includes allocation and reallocation calls together:

| Public phase | Calls | Requested bytes | Retained delta bytes | Peak delta bytes |
| --- | ---: | ---: | ---: | ---: |
| lex, including source copy | 20 | 51,298,221 | 26,132,493 | 26,132,493 |
| parse, including lex/source copy | 333,374 | 215,961,645 | 140,464,461 | 140,464,461 |
| format, while parsed input remains live | 80 | 73,936,450 | 2,633,338 | 31,993,466 |

The retained parse allocation is about 114 MB beyond standalone lexing; the
333,374 requests demonstrate per-node/child-vector allocation traffic rather
than inferring an allocation count from RSS. The format phase adds another live
working set while holding the CST. These observations support the existing
bounded storage/capacity/lifetime work in base-047. They do not prescribe a new
representation or equate requested bytes with physical memory. Raw phase output
is `target/benchmarks/base022-allocations.txt`; its timings include counter overhead.

Native `/usr/bin/sample` at 1 ms intervals for 2 s during 5 s repeated phases
found 1,042 of 1,450 parse-phase samples under `lexer::scan`, predominantly prefix
comparison/memcmp. In format-only sampling, 933 of 1,453 samples passed through
`layout::apply_layout` re-lexing, again dominated by the lexer. `node_exceeds_width`
and rendering/fit stacks were visible too, supporting base-046's bounded fit work.
The samples are local `/tmp/zincite-base022-parse.sample.txt` and
`/tmp/zincite-base022-format.sample.txt`. Repeated instrumented-process footprints
are not the fresh save RSS measurements.

The lexer currently tests the complete longest-first `SYMBOLS` list for each
punctuation token; comma occurs late in that list. Dense input exercises this
path during initial parsing and again during final layout conversion. A bounded
follow-up should dispatch symbol candidates by their first character while
preserving longest-match behavior, Unicode operators, spelling/ranges and
malformed-input recovery, then compare release save timings on this same set.
This CPU bottleneck lies outside base-046's fit work and base-047's working-set
scope. No optimization was made while collecting this baseline.

The measurement choices follow the distinction between uncached CLI work,
microbenchmarks and profiles in [Ruff's contributing guide](https://github.com/astral-sh/ruff/blob/main/CONTRIBUTING.md),
the cold/warm labels in [uv's benchmark guide](https://github.com/astral-sh/uv/blob/main/BENCHMARKS.md),
and first versus later diagnostics in [ty's benchmark guide](https://github.com/astral-sh/ruff/blob/main/scripts/ty_benchmark/README.md).
The [Ruff formatter infrastructure](https://github.com/astral-sh/ruff/blob/main/crates/ruff_formatter/src/lib.rs),
[uv cache semantics](https://docs.astral.sh/uv/concepts/cache/) and
[ty incremental analysis](https://docs.astral.sh/ty/features/language-server/#fine-grained-incrementality)
are design references; the save command adds no persistent state or semantic work.

# Full corpus correctness check

Build the one-file developer checker, then inventory and check every `.mzn` and
`.dzn` file in the supplied trees:

```sh
cargo build --release -p zincite-fmt --example check-corpus-file
python3 scripts/check-corpus.py --challenge /tmp/mzn-challenge --local ~/minizinc --supplement /tmp/mznc2026_probs
```

Use a complete checkout of [MiniZinc/mzn-challenge](https://github.com/MiniZinc/mzn-challenge).
The [2026 published problems](https://www.minizinc.org/challenge/2026/mznc2026_probs.tar.gz)
supply instances absent from the Git archive at the baseline revision. Extract
that archive outside this repository and pass its directory with `--supplement`.
Keep originals private and read only. The command does not fetch, compile models,
run solvers, discover include dependencies or rewrite any source.

The standard-library Python driver includes ignored files and nested repositories;
it skips only `.git` metadata and records directory aliases/cycles. It processes
content duplicates individually and reports their hashes and first matching path.
The inventory records source/year/subtree counts, the Git revision, missing tracked
inputs and years without archive data. Missing directories are usage errors;
filesystem inventory errors remain explicit coverage gaps. Extension-selected
`.dzn` checks retain data grammar. A file rejected in data mode but accepted as a
model with declarations is reported separately, never silently reinterpreted.

Each fresh child has a ten-second timeout (`--timeout SECONDS`). Reports default
to ignored `target/corpus/latest/`; `--output DIRECTORY` selects another location
outside the input trees. A repeated command replaces that report directory's
three report files. `inventory.json` records every discovered input,
`results.jsonl` streams one result per file, and `summary.json` reconciles the
counts. Reports can contain local paths and bounded diagnostic/token snippets;
review them before sharing. No input fixtures are copied into tracked source.

The child accepts UTF-8 with an optional leading BOM, removes that encoding marker
before syntax checks, and checks contiguous token coverage and exactly ordered CST leaves,
then parse, default library formatting, reparse, preservation, second-pass
stability and both current syntax lint rules. Lint warnings count as successful
runs. Parser errors skip formatting and lint for that file. EditorConfig and CLI
configuration are covered by the existing focused CLI tests, not this library
acceptance run.

Preservation compares exact original token/literal/comment spelling counts
independently of include sorting. Ordered CST events then allow only the existing
bounded include groups to reorder with their attached comments. They retain all
other item, token and comment order. A separate comparison retains the ordered original and formatted protected-range
byte slices, including boundary trivia and range count. Skip/off spans must match
verbatim before any layout allowances apply. Outside protected text, whitespace, grammar-optional trailing commas
at supported list closings, and the optional final top-level semicolon are
normalized; nested statement separators and singleton tuple structure remain
checked. Include groups reuse the existing sorter only for expected order, so a
sorting defect can still require a separate targeted check; it cannot hide
spelling or comment loss.

Exit 0 means all inputs passed, 1 means recorded failures or inventory gaps, and
2 means invalid command arguments. Timeouts, crashes, encoding errors, data-mode
model items, parse failures requiring assessment, directives, formatted parse
errors, preservation and unstable second passes remain distinct. Compiler tests
with an explicit `!Error` expectation in their opening test block receive a negative-test hint;
that hint does not establish invalid syntax or excuse a Zincite failure. Inspect
compiler expectations and known model/data pairings before making that judgment.
This command reports the current compatibility baseline and makes no fixes.
