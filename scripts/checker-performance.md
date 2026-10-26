# Interactive checker measurements

The existing two-job workshop schedule met the 100 ms p95 checking target on
2026-10-26: native checking took 0.0083 ms p95; the browser assignment-to-report
round trip took 0.20 ms p95. Each uses 60 individual checks of a prepared model.
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
| Native preparation | 0.080333 / 0.081083 [0.07975–0.084208] | 0.776667 / 0.79825 [0.773167–0.904667] | 8.00308 / 8.63162 [7.96908–8.67258] |
| Native serialized load | 0.038458 / 0.038833 [0.038042–0.044083] | 0.38675 / 0.424917 [0.381375–0.500584] | 4.101 / 4.27158 [4.08708–4.40479] |
| Native check | 0.006916 / 0.008292 [0.006167–0.008666] | 0.068292 / 0.086292 [0.062542–0.087791] | 0.666916 / 0.791 [0.634875–0.792292] |
| Browser serialized load | 0.1 / 0.3 [0–0.3] | 1 / 1.1 [0.9–1.1] | 9.9 / 10.1 [9.8–10.2] |
| Browser assignment → parsed report | 0 / 0.2 [0–0.2] | 0.3 / 1.1 [0–1.1] | 2.7 / 10.7 [0.6–11] |

The browser clock exposed roughly 0.1 ms steps. Zero means below that resolution,
not zero work. The ordinary-case p95 is far enough below 100 ms that this clock
limit does not affect the target comparison.

| Normalized ten-call component | 1 copy: p50 / p95 [min–max] | 10 copies | 100 copies |
| --- | ---: | ---: | ---: |
| Native check with report disposal | 0.0069 / 0.0118792 [0.0067375–0.0119125] | 0.0699917 / 0.12305 [0.0666625–0.127704] | 0.7275 / 1.22635 [0.668154–1.25767] |
| Browser assignment encoding | 0 / 0 [0–0.01] | 0 / 0.01 [0–0.02] | 0.01 / 0.02 [0–0.02] |
| Wasm boundary/check/report | 0.02 / 0.06 [0–0.07] | 0.15 / 0.47 [0.06–0.48] | 1.44 / 4.8 [0.66–4.81] |
| Browser report parsing | 0.01 / 0.06 [0–0.07] | 0.12 / 0.58 [0–0.59] | 1.21 / 5.73 [0–5.77] |

The incomplete reports carry many repeated source references. At 100 copies,
the omitted-input report is 1,874,097 compact UTF-8 bytes, while a valid report
is 130 bytes. Report parsing is therefore a material part of the browser cost;
the small valid report alone would give a misleading workload. This measured
cost does not justify incremental evaluation or a report-format redesign here.

| Size / memory scope | 1 copy | 10 copies | 100 copies |
| --- | ---: | ---: | ---: |
| Slots / named references / constraints | 5 / 9 / 5 | 50 / 90 / 50 | 500 / 900 / 500 |
| FlatZinc source bytes | 4,415 | 40,605 | 408,615 |
| Prepared JSON bytes | 22,262 | 224,176 | 2,252,788 |
| Saved five-assignment JSON bytes | 1,277 | 11,624 | 115,214 |
| Compact infeasible / omitted report bytes | 3,710 / 18,118 | 36,777 / 185,512 | 369,874 / 1,874,097 |
| Native helper peak RSS, bytes | 4,276,224 | 11,747,328 | 86,556,672 |
| Wasm linear-memory bytes before → after timed phases | 1,245,184 → 1,245,184 | 2,097,152 → 2,097,152 | 8,650,752 → 8,650,752 |

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
wasm32-unknown-unknown; wasm-bindgen library/CLI 0.2.129. Collection used a real
Playwright-controlled Chrome 156.0.8078.47, Chromium revision
`1d17a2e2ee147bb7d6ae2ab6abdb3a2510c17775`, V8 15.6.75.19. The shortened user agent
says 156.0.0.0; the full version came from User-Agent Client Hints and CDP
`Browser.getVersion`. Node 24.22.1 was available but did not measure browser work.

Runtime source was `43b8e555d5976cfaa2242777fec79385fb836bfa`; the new measurement
sources are saved alongside this report. Native then browser collection ran
serially around 13:12 UTC, with ordinary desktop activity uncontrolled. No heavy
builds ran during timing, and no thermal, CPU-frequency or cold-cache controls
were applied. This is a warm steady-state measurement.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Native release measurement helper | 1,534,112 | `8d6055f3f7d05c15b0dc306c8de28a597ce1546a59d612a6e75da96887ea5083` |
| Browser Wasm | 514,799 | `86f4c02db0bf0e5f653013b32495c8286af20a082171476564af7a27d0fbe428` |
| Browser glue | 8,872 | `6d2b06da149c2350ead9e2c2965e3b08e02462df4b1f37fc9ddc2dbe0ff05195` |
| Ordinary source | 4,415 | `3c89f0a8a6479349f2367a871ead21a1eecf1acf7f32b02c7048628df1196212` |
| Ordinary prepared JSON | 22,262 | `f67d2c9ce10fd54a8448fef0473fe72df42ab4358582eea28069b5b7453abf20` |

## Repeat, resume and analyze

Use Rust, wasm-bindgen-cli 0.2.129 and `uv` on PATH. From the repository root:

```sh
cargo build --locked --release -p zincite-check --example measure-checking
cargo build --locked --release -p zincite-check --lib --target wasm32-unknown-unknown --no-default-features --features wasm
mkdir -p target/checker-browser
wasm-bindgen --target web --out-dir target/checker-browser/web target/wasm32-unknown-unknown/release/zincite_check.wasm
uv run --script scripts/bench-checker.py run --name m1-max-release-2026-10-26-final --dry-run
uv run --script scripts/bench-checker.py run --name m1-max-release-2026-10-26-final
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

Open <http://127.0.0.1:8000/m1-max-release-2026-10-26-final/>, collect once, and download
`browser.json`. The page loads static files before timing and makes no requests
inside the timing loops. Import the downloaded file, then analyze saved records:

```sh
uv run --script scripts/bench-checker.py import-browser --name m1-max-release-2026-10-26-final /path/to/downloaded/browser.json
uv run --script scripts/bench-checker.py analyze --name m1-max-release-2026-10-26-final
```

Browser import checks the saved configuration, case coverage, finite timing
arrays and memory fields. An existing browser record cannot be replaced by a
different collection. Analysis uses saved records alone and shows failures and
unfinished cases explicitly; it needs no native executable, Wasm or browser.
Default raw files live under ignored
`target/checker-browser/m1-max-release-2026-10-26-final/`, including complete reports,
individual timing arrays, stdout/stderr and `analysis.txt` from this session.
`--results-root` must contain sibling `web/` bindings for collection; the default
lets the existing workshop server serve both pages.

Bounded manual checks exercised dry-run, partial collection and resume, unchanged
completed-record hashes, malformed-record retry (including valid JSON with a wrong metrics type),
changed-sample rejection, a
captured intentional exit-42 failure, failure reuse, duplicate browser import,
and analysis after removing the disposable failure executable. No timing
assertions were added to Cargo tests.
