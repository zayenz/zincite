# Lexer symbol dispatch comparison

On 2026-09-30, selecting symbol candidates by their first UTF-8 byte reduced the
measured CPU cost of token-dense input. Candidate groups retain longest-match
order and compare complete Unicode spellings; singleton ASCII punctuation returns
directly. The scanner still owns comments, literals, identifiers, numbers and
malformed-character recovery. No parser/CST, formatter layout or allocation
representation changed.

The comparison used Rust 1.98.1 release builds on an Apple M1 Max with 64 GiB RAM.
The clean before revision was `b5f6f0276abc752e1a1c4a25071e33f1aa9617bd`.
Before editing, the formatter and existing phase probe were rebuilt and copied to
ignored `target/benchmarks/base051-before/`. Their SHA-256 hashes were respectively
`d618d5ac9ea5a50bea5265a6b319b1f1242d65e6a96c8330028dcc7faaa20077` and
`8108c239a2faba313b129c63dea5af81123a56004e83e7fa40a3a646a173158b`.
The after formatter hash is
`4338cac72aa0b31bd9be937c60bbcd748909182910a3dac982141827477ac281`.

```sh
cargo build --release -p zincite-fmt --example profile-phases --bin zincite-fmt
mkdir -p target/benchmarks/base051-before
cp target/release/zincite-fmt target/benchmarks/base051-before/zincite-fmt
cp target/release/examples/profile-phases target/benchmarks/base051-before/profile-phases
python3 scripts/bench-save.py --binary target/benchmarks/base051-before/zincite-fmt --external ~/minizinc/challenge-models/2021/problem-set/mznc2021_probs/ATSP/atsp.mzn --external ~/minizinc/challenge-models/2021/problem-set/mznc2021_probs/java-routing/trip_7_4.mzn --output target/benchmarks/base051-before.json
# Apply the lexer change, rebuild, then run the same cases.
cargo build --release -p zincite-fmt --example profile-phases --bin zincite-fmt
python3 scripts/bench-save.py --external ~/minizinc/challenge-models/2021/problem-set/mznc2021_probs/ATSP/atsp.mzn --external ~/minizinc/challenge-models/2021/problem-set/mznc2021_probs/java-routing/trip_7_4.mzn --output target/benchmarks/base051-after.json
```

Both runs used the unchanged base-022 driver, 50 fresh processes per case/settings,
interleaved default values and equivalent nested EditorConfig settings, complete
stdout capture and separate native child RSS probes. The before batch finished
before building/running the after batch. Native CPU profiles followed both timing
batches; no benchmark or profiler competed with another. These sequential batches
use ordinary warm filesystem caches, not controlled cold I/O or confidence bounds.
Small startup-dominated differences can include ambient scheduling variation.
Input hashes and all successful output hashes match across all 26 case/settings
pairs. Every measured status is accepted, including exit 2 and no source output
for the invalid edited buffer. Native RSS probes also retain the expected statuses.
Original external sources were only read.

Values below are default / nested-configuration pairs, in milliseconds. Each
report retains all 50 samples, min/max spread, child CPU, throughput and RSS units.

| Case | Bytes | Before p50 | After p50 | Before p95 | After p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| ordinary-changed | 1,163 | 6.17 / 6.02 | 4.42 / 4.49 | 8.04 / 7.73 | 4.72 / 4.99 |
| ordinary-formatted | 1,324 | 6.43 / 5.81 | 4.53 / 4.63 | 11.17 / 8.36 | 4.95 / 5.13 |
| invalid-edited | 33 | 5.38 / 5.21 | 4.28 / 4.30 | 8.22 / 8.28 | 4.70 / 4.74 |
| matrix | 676 | 6.13 / 6.64 | 4.70 / 4.75 | 9.45 / 9.48 | 5.02 / 5.07 |
| nested | 1,133 | 6.80 / 6.43 | 5.05 / 5.10 | 9.66 / 9.41 | 5.58 / 5.48 |
| matrix-grown | 6,971 | 10.45 / 10.50 | 7.38 / 7.46 | 14.26 / 14.53 | 7.82 / 8.04 |
| nested-grown | 4,493 | 29.08 / 28.98 | 25.40 / 25.56 | 36.58 / 33.92 | 26.57 / 26.53 |
| dense-10000 | 9,669 | 7.90 / 7.96 | 5.34 / 5.37 | 9.20 / 9.33 | 5.92 / 5.90 |
| dense-100000 | 96,669 | 31.78 / 31.82 | 15.84 / 15.92 | 36.84 / 37.33 | 16.82 / 16.90 |
| dense-1000000 | 966,669 | 274.50 / 274.15 | 122.42 / 121.84 | 300.24 / 313.68 | 126.69 / 125.29 |
| dense-2000000 | 1,933,338 | 519.43 / 519.86 | 241.48 / 241.31 | 560.53 / 578.06 | 249.96 / 254.05 |
| external-0-atsp.mzn | 6,899 | 5.97 / 6.08 | 5.28 / 5.35 | 6.53 / 6.76 | 5.63 / 5.81 |
| external-1-trip_7_4.mzn | 85,229 | 21.60 / 21.60 | 14.30 / 14.40 | 22.11 / 22.48 | 15.00 / 15.09 |

The earlier base-022 966,669-byte p95 was 339.20 / 340.49 ms; this contemporaneous
before run measured 300.24 / 313.68 ms. Use the retained before binary for the
implementation comparison rather than attributing that earlier variation to this
change. The base-022 96,669-byte borderline 50 ms miss did not persist in its own
repeat and also stays below budget here.

| Dense bytes | Median child CPU before | Median child CPU after | RSS before MiB | RSS after MiB |
| --- | ---: | ---: | ---: | ---: |
| 9,669 | 5.69 / 5.73 | 3.56 / 3.61 | 3.36 / 3.38 | 3.34 / 3.38 |
| 96,669 | 28.22 / 28.16 | 13.07 / 13.19 | 17.30 / 18.88 | 17.31 / 17.31 |
| 966,669 | 260.44 / 261.23 | 113.51 / 113.42 | 141.25 / 141.25 | 142.73 / 143.25 |
| 1,933,338 | 500.98 / 501.18 | 225.37 / 225.19 | 280.38 / 279.95 | 280.42 / 280.45 |

RSS comes from `/usr/bin/time -l` outside the timed samples. Darwin reports bytes;
the table divides by 1,048,576. Every native probe succeeded; raw measurements
remain in the JSON. The large-case working set is essentially unchanged. From
96,669 to 966,669 to 1,933,338 bytes, after median child CPU grows about 8.6× then
2.0×, and save p50 grows about 7.7× then 2.0×. These points show no unexplained
superlinear growth; they do not establish every possible input shape.

First use is retained separately: before 1,086.72 ms wall / 5.15 ms child CPU;
after 395.14 ms wall / 3.89 ms child CPU. Both returned the same successful output.
The large wall/CPU gap persists outside the repeated distribution. Its cause is
unknown; do not credit symbol dispatch with eliminating first-launch delay or
call either measurement controlled cold I/O. Existing base-048 owns that save-path
investigation and final acceptance.

## CPU attribution and remaining work

The unchanged developer probe received the driver's exact 966,669-byte dense data.
Native `/usr/bin/sample PID 2 1 -file PATH` sampled separate five-second parse and
format loops after a 0.5-second startup interval. All four probes and samples
returned 0. The repeated instrumented parse processes report native physical
footprints of 1.2G before and 1.5G after while completing different iteration
counts. These diagnostic-loop footprints differ from fresh CLI peak RSS and do
not establish save-command memory use or its cause. Lexer stacks occupied 1,069 of 1,469 before parse samples, then 349 of
1,442 after samples; before format sampling counted 944 of 1,452 in the layout
re-lexing path, then 347 of 1,436 after samples. The former repeated `memcmp` stacks
had 660 parse and 570 format samples; neither after sample contains that stack.
Parse iterations increased from 47 in 5,093.98 ms to 128 in 5,016.11 ms; format
iterations increased from 38 in 5,004.00 ms to 81 in 5,042.53 ms.

The single allocation-counter phase probe reports lex 75.51 → 9.20 ms, parse
100.45 → 34.75 ms and format 129.04 → 61.96 ms. Those measurements include
instrumentation overhead and are attribution evidence, not save latency. All
allocation counts and byte totals remain exactly equal: lex 20 calls, parse
333,374 calls, and format 80 calls. Parse retains 140,464,461 bytes; formatting
adds a 31,993,466-byte peak delta while retaining the CST. These are allocation
measurements, not physical RSS.

The 96,669-byte case and both real models meet the 50 ms / 32 MiB budget in this
run. The 966,669-byte case still misses 100 ms / 64 MiB: p95 is 126.69 / 125.29 ms
and RSS 142.73 / 143.25 MiB. Larger supported input remains about 250 ms / 280 MiB.
After format sampling shows rendering and width-preview work as well as re-lexing;
base-046 owns bounded fit/measurement cost, base-047 owns working-set reduction,
and base-048 owns final save-budget and first-use acceptance. This change does not
relax their budgets or claim complete performance acceptance.

Raw evidence remains ignored under `target/benchmarks/`: `base051-{before,after}.json`,
`base051-{before,after}-allocations.txt`, phase iteration logs and
`base051-{before,after}-{parse,format}.sample.txt`. Profile statuses are in
`base051-profile-status.json`; the probe input is `base051-dense.dzn`.
The existing lossless/recovery, formatting/comment/idempotence and lint checks pass,
with one focused symbol-prefix/Unicode/recovery regression. Required workspace fmt,
clippy and tests pass. No full corpus rerun was needed for this bounded dispatch
change; the measured cases retain identical output bytes.
