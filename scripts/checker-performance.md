# Interactive checker measurements

The existing two-job workshop schedule met the 100 ms p95 checking target on
2026-10-28: native checking took 0.008834 ms p95; the browser assignment-to-report
round trip took 0.10 ms p95. Each uses 60 individual checks of a prepared model.
These measurements cover the checker and JSON work, with no DOM rendering.
They do not establish a response-time guarantee for other models.

## Workload and timing

The ordinary case is `examples/checker-compilation/model.fzn`, used by the
[graphical workshop example](../examples/checker-browser/README.md). Its five
value slots, nine named value references (including aliases) and five constraints
complete finish times and an early-start Boolean from supplied starts. The
measured edit cycle is:

| State | Assignment | Established result |
| --- | --- | --- |
| Feasible | `start=[1,4]` | Complete, valid |
| Infeasible | `start=[8,5]` | Complete, invalid: finish 11 outside its domain, precedence sum 6 > 0, start sum 13 > 10 |
| Omitted | No entries | Incomplete, no violations; 9 unresolved references and 14 blocked checks |
| Incomplete invalid | Only first start = 8 | Invalid and incomplete: finish 11 outside its domain; 4 unresolved references and 7 blocked checks |
| Recovered | `start=[1,4]` | Complete, valid again |

This cycle includes the cost of reporting missing input and known violations.
The feasible state supplies only starts: completion derives the Boolean too.
Ten warmups precede 60 measured samples, giving twelve samples per cycle position.
The 10- and 100-copy cases put independent renamed copies **inside one model**;
all copies receive the same state in each edit. Their longer reports keep the
original source labels and paths. They are synthetic scaling cases, not larger
compiled application models. No compiler or solver runs during collection.

The Rust helper uses `Instant` around `prepare`, `PreparedModel::from_json` and
one `check` call in separate phases. File reads, assignment construction,
serialization, report comparisons and output writes are outside those intervals.
Preparation and load stop before dropping the result; individual checks stop
before dropping their report. A separate ten-call batch includes report disposal
and saves normalized costs. Native process launch time is never a checking metric.

The real browser uses `performance.now()`. Serialized load times the constructor
on already fetched text and excludes `free()`. Individual `edit_ms` times
`JSON.stringify(assignment)`, `model.check` and `JSON.parse(report)` together.
Normalized ten-call batches separately time assignment encoding, the Wasm string
boundary (assignment parsing, Rust checking, report serialization and transfers),
and report parsing. Their percentiles describe **batch averages**, not individual
call tails. Native checking and this Wasm boundary have different scopes; their
difference is not a measurement of binding overhead alone.

Wasm initialization, static fetches, model construction and full report equality
checks precede repeated checking. Native correctness compares the original
prepared model, a fresh loader for each state and one reused loaded model.
Browser correctness compares all five full reports with native results before
timing. Recovered/omitted states check that earlier values and violations do not
leak into later calls.

## Results

All times below are milliseconds, with nearest-rank p50/p95 and observed min–max.
Each cell summarizes 60 samples after ten warmups. There were four expected
records (three native cases and one browser collection), four completed records,
zero failures and zero unfinished records. Samples share a process and input;
they describe this session, without statistical confidence bounds.

| Individual operation | 1 copy: p50 / p95 [min–max] | 10 copies | 100 copies |
| --- | ---: | ---: | ---: |
| Native preparation | 0.095375 / 0.099917 [0.094916–0.146625] | 0.925708 / 1.03533 [0.91825–1.06846] | 9.21429 / 9.50833 [9.19062–10.5868] |
| Native serialized load | 0.040083 / 0.0405 [0.039708–0.04225] | 0.396666 / 0.412792 [0.394208–0.885834] | 4.06896 / 4.09442 [4.05733–4.10617] |
| Native check | 0.007625 / 0.008834 [0.006959–0.009083] | 0.074042 / 0.0875 [0.067209–0.088084] | 0.73625 / 0.829084 [0.682167–0.845292] |
| Browser serialized load | 0.1 / 0.2 [0–0.3] | 1 / 1.1 [0.9–1.2] | 9.6 / 9.7 [9.5–9.7] |
| Browser assignment → parsed report | 0 / 0.1 [0–0.3] | 0.3 / 1 [0–1] | 2.5 / 9.6 [0.6–10] |

The browser clock exposed roughly 0.1 ms steps. Zero means below that resolution,
not zero work. The ordinary-case p95 is far enough below 100 ms that this clock
limit does not affect the target comparison.

| Normalized ten-call component | 1 copy: p50 / p95 [min–max] | 10 copies | 100 copies |
| --- | ---: | ---: | ---: |
| Native check with report disposal | 0.0076917 / 0.0125666 [0.00745–0.0143625] | 0.0754208 / 0.124717 [0.0728375–0.129204] | 0.756888 / 1.18948 [0.727429–1.2353] |
| Browser assignment encoding | 0 / 0 [0–0.01] | 0 / 0.01 [0–0.01] | 0.01 / 0.02 [0–0.02] |
| Wasm boundary/check/report | 0.03 / 0.07 [0–0.09] | 0.15 / 0.45 [0.07–0.46] | 1.44 / 4.63 [0.69–4.65] |
| Browser report parsing | 0.01 / 0.06 [0–0.1] | 0.1 / 0.49 [0–0.49] | 1.01 / 4.81 [0–4.83] |

The incomplete reports carry many repeated source references. At 100 copies,
the omitted-input report is 1,870,958 compact UTF-8 bytes, while a valid report
is 130 bytes. Report parsing is therefore a material part of the browser cost;
the small valid report alone would give a misleading workload. This measured
cost does not justify incremental evaluation or a report-format redesign here.

| Size / memory scope | 1 copy | 10 copies | 100 copies |
| --- | ---: | ---: | ---: |
| Slots / named references / constraints | 5 / 9 / 5 | 50 / 90 / 50 | 500 / 900 / 500 |
| FlatZinc source bytes | 4,415 | 40,595 | 408,515 |
| Prepared JSON bytes | 22,341 | 224,181 | 2,252,008 |
| Saved five-assignment JSON bytes | 1,277 | 11,624 | 115,214 |
| Compact infeasible / omitted report bytes | 3,710 / 18,118 | 36,753 / 185,246 | 369,589 / 1,870,958 |
| Native helper peak RSS, bytes | 5,128,192 | 12,959,744 | 89,309,184 |
| Wasm linear-memory bytes before → after timed phases | 1,310,720 → 1,310,720 | 2,162,688 → 2,162,688 | 9,109,504 → 9,109,504 |

RSS comes from macOS `/usr/bin/time -l` around the whole helper, including source,
prepared/loaded models, correctness reports, preparation and serialization.
It is not the check's incremental allocation cost. Wasm numbers are the allocated
linear-memory buffer (64 KiB pages), after correctness and after timing, in one
instance across ascending case sizes. They exclude JS heap, JIT and browser RSS;
freed models need not shrink this buffer. These metrics cannot be compared as
native-versus-browser memory usage. They show bounded growth over these sizes,
with no additional linear-memory growth during each case's timed phases.

Preparation, load and check times grew approximately with the replicated model
and report sizes. No missed target or unexplained superlinear scaling was found,
so no profiler or runtime optimization was needed. A future model that misses
its budget should first profile its actual checks, serialized reports and caller
work; this run does not select an incremental algorithm.

## Build and host

Apple M1 Max, arm64, 64 GiB RAM (`hw.memsize=68719476736`), macOS 27.0.1 build
26A434. Rust 1.98.1 (`48a229cea`, 2026-09-01), default Cargo release profile,
wasm32-unknown-unknown; wasm-bindgen library/CLI 0.2.129. Collection used the real
Chrome browser through the CUA extension, version **156.0.8078.47**, reported by
User-Agent Client Hints in the saved browser record. The shortened user agent
says 156.0.0.0. Node 24.22.1 supplies correctness smoke results, with no browser timing.

The runtime baseline is `e81ddd9c496d34d5c442726ab0fd75649cfc238d`, with this task's
help, tests, documentation and workload-order correction. The evaluator is
unchanged. The run records hashes of the measurement sources, helper, bindings
and inputs. The library audit uses MiniZinc
`c3448c48e7f05f13805616cfd1b9e94972eef58c` and Gecode
`186551447db0f74639d65cb8de5af190f15ac2c2`; `library-revisions.json` saves those
identities beside the raw measurements. Neither library runs during timing.

Native collection preceded browser collection around 13:24–13:26 UTC on
2026-10-28. Ordinary desktop activity was uncontrolled. No heavy builds ran during
timing; no thermal, CPU-frequency or cold-cache controls were applied. This is a
warm steady-state measurement.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Native release measurement helper | 3,420,784 | `6491ebf821909fc67d2e55013d93e991d15ff32dbc2c82c851b13f895695a049` |
| Browser Wasm | 1,817,323 | `714b160f6ce97123be02d3e5d7f4a4977d82e435b0a9b2ee35c9db8e00df1402` |
| Browser glue | 9,789 | `f9c1a95939f070043f65bc105d47bcf9f1ee222b7e2ff8850f7b64c7833dd8c5` |
| Ordinary source | 4,415 | `3c89f0a8a6479349f2367a871ead21a1eecf1acf7f32b02c7048628df1196212` |
| Ordinary prepared JSON | 22,341 | `cdd19d0ad891af4d53e0960ff4fff5b40e73e99a094d89aae0b37a9b693bfc29` |

## Repeat, resume and analyze

Use Rust, wasm-bindgen-cli 0.2.129 and `uv` on PATH. From the repository root:

```sh
cargo build --locked --release -p zincite-check --example measure-checking
cargo build --locked --release -p zincite-check --lib --target wasm32-unknown-unknown --no-default-features --features wasm
mkdir -p target/checker-browser
wasm-bindgen --target web --out-dir target/checker-browser/web target/wasm32-unknown-unknown/release/zincite_check.wasm
uv run --script scripts/bench-checker.py run --name m1-max-release-2026-10-28-full-acceptance-v2 --dry-run
uv run --script scripts/bench-checker.py run --name m1-max-release-2026-10-28-full-acceptance-v2
```

Use a new name for a new experiment. The default is 60 samples, ten warmups,
batches of ten and 1/10/100 copies. `--help` lists the bounded pilot controls.
`--limit` limits collection now without changing the planned cases. The run
configuration records host/tool versions and hashes of the helper, bindings,
measurement sources and input templates; case records identify generated files.
The three sizes take a few seconds each on this host; the default timeout is
120 seconds per native case. Only one writer may use a name. A matching run skips completed native success,
failure and timeout records unchanged. Missing/malformed records are unfinished
and their case outputs are replaced on retry. Changed material settings or hashes
require a new name. Keep the browser version and host load comparable yourself;
resume cannot reconstruct their earlier state.

Reuse the workshop's local static server, or start it if none is running:

```sh
python3 -m http.server 8000 --bind 127.0.0.1 --directory target/checker-browser
```

Open <http://127.0.0.1:8000/m1-max-release-2026-10-28-full-acceptance-v2/>, collect once, and download
`browser.json`. The page loads static files before timing and makes no requests
inside the timing loops. Import the downloaded file, then analyze saved records:

```sh
uv run --script scripts/bench-checker.py import-browser --name m1-max-release-2026-10-28-full-acceptance-v2 /path/to/downloaded/browser.json
uv run --script scripts/bench-checker.py analyze --name m1-max-release-2026-10-28-full-acceptance-v2
```

Browser import checks the saved configuration, case coverage, finite timing
arrays and memory fields. An existing browser record cannot be replaced by a
different collection. Analysis uses saved records alone and shows failures and
unfinished cases explicitly; it needs no native executable, Wasm or browser.
Default raw files live under ignored
`target/checker-browser/m1-max-release-2026-10-28-full-acceptance-v2/`, including complete reports,
individual timing arrays, stdout/stderr and `analysis.txt` from this session.
`--results-root` must contain sibling `web/` bindings for collection; the default
lets the existing workshop server serve both pages.

The first named run, `m1-max-release-2026-10-28-full-acceptance`, saved one
ordinary success and two scaling failures. The generator concatenated complete
copy blocks, putting declarations after constraints; the full FlatZinc parser
correctly rejected their item order. The correction writes all renamed
parameters, then variables, then constraints. It leaves the single-copy model,
edit cycle, timing boundaries and runner record format unchanged. The failed
records are saved under their original name and supply no scaling timings.

A five-sample pilot, `checker033-scaling-pilot`, prepared and checked all three
sizes. Resuming it skipped every completed record with unchanged SHA-256 values.
The final `v2` experiment then collected the full workload above, and saved-only
analysis reported four completed records, zero failures and zero unfinished
records. Existing runner durability checks cover malformed records, changed
settings, failures, timeouts and browser import; this generation correction did
not change those contracts.
