# Current bounded save checkpoint

The 2026-10-12 release campaign completes all fourteen retained save cases with
their expected output or error status. Each named ordinary editing case meets
the 100 ms end-to-end p95 target clarified on 2026-10-15, using the retained
measurements below. This documentation update adds no new timings and does not
establish performance of a later build.

The ordinary set is `ordinary-changed`, `ordinary-formatted`, `invalid-edited`,
`matrix`, `external-0-atsp.mzn` and `external-1-trip_7_4.mzn`. They cover changed
and already formatted models, comments, malformed edits and an ordinary matrix.
`matrix-grown` measures matrix growth; `nested` and `nested-grown` are synthetic
nesting stress cases. All `dense-*` cases and the public wide matrix
`external-2-u7g2pref1.dzn` measure generated/dense data throughput, memory and
scaling outside the ordinary-file target. These are workload classifications,
not a byte-size cutoff. All cases remain supported and retained.

The default 966,669-byte dense result remains 111.014 ms p95. It exceeded the
former size-based criterion used when this campaign ran; it does not create an
ordinary-file acceptance failure or require an optimization task solely for
exceeding 100 ms.

This base-129 campaign is separate from the
[historical save checkpoints](save-performance.md). It ran on 2026-10-12 from
`90c069eee88d76118010c60049b438175a15b7b3`, with no Rust changes, on an Apple
M1 Max with 64 GB memory, macOS 26.6.2 arm64, Rust 1.98.1 and Python 3.12.6.
The release formatter SHA256 is
`7ae298328785d56d0f99de175026af4db44d8fee2235613ba61a0b5a6c0fcb31`.
Builds, tests and other workers' measurements were held outside the timed window.
Ordinary warm filesystem caches and unrelated host activity were uncontrolled.

## Protocol and retained results

The unchanged `scripts/bench-save.py` runs 50 fresh processes per case and
settings, with stdin input and complete stdout/stderr capture. Its p95 is the
48th sorted sample. D uses defaults; N looks up nested EditorConfig selecting the
same four spaces, tab width four, LF, final newline, trimming, UTF-8 and width 120.
Latency includes launch, transfer, lookup, parsing and formatting. No samples
were removed or pooled.

A local capture wrapper compares every timed output and status with the complete
retained streams for that case and settings. Separate matched stdin saves retain
28 exact-child `os.wait4` peak RSS observations, complete streams, statuses and
reaped-child receipts, each with a 30-second SIGALRM deadline. Darwin RSS is bytes,
divided by 1,048,576 for MiB. The wrapper substitutes this native measurement for
the driver's `/usr/bin/time` probe; it leaves the latency loop and inputs intact.
RSS is a separate single observation per row, not a percentile or allocation count.

| Case | Settings | Input bytes | p50 ms | p95 ms | min–max ms | RSS MiB |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| ordinary-changed | D | 1,163 | 4.806 | 5.402 | 4.237–5.601 | 2.000000 |
| ordinary-changed | N | 1,163 | 4.941 | 5.349 | 4.274–5.462 | 2.046875 |
| ordinary-formatted | D | 1,356 | 4.903 | 5.424 | 4.182–5.604 | 2.015625 |
| ordinary-formatted | N | 1,356 | 4.991 | 5.467 | 4.381–5.633 | 2.125000 |
| invalid-edited | D | 33 | 4.352 | 4.918 | 3.887–5.609 | 1.812500 |
| invalid-edited | N | 33 | 4.411 | 5.082 | 3.997–5.328 | 1.828125 |
| matrix | D | 676 | 4.380 | 5.113 | 3.953–5.223 | 1.890625 |
| matrix | N | 676 | 4.462 | 5.002 | 3.953–5.276 | 1.906250 |
| nested | D | 1,133 | 5.395 | 5.853 | 4.763–5.887 | 2.125000 |
| nested | N | 1,133 | 5.391 | 5.927 | 4.858–6.105 | 2.156250 |
| matrix-grown | D | 6,971 | 5.225 | 5.735 | 4.661–5.983 | 2.312500 |
| matrix-grown | N | 6,971 | 5.295 | 5.794 | 4.760–7.232 | 2.328125 |
| nested-grown | D | 4,493 | 33.297 | 33.918 | 32.691–34.072 | 3.578125 |
| nested-grown | N | 4,493 | 33.394 | 33.987 | 32.785–34.340 | 3.703125 |
| dense-10000 | D | 9,669 | 5.530 | 6.124 | 4.925–6.417 | 2.453125 |
| dense-10000 | N | 9,669 | 5.524 | 6.209 | 5.038–6.238 | 2.421875 |
| dense-100000 | D | 96,669 | 13.315 | 13.766 | 12.374–14.255 | 6.671875 |
| dense-100000 | N | 96,669 | 13.257 | 13.955 | 12.399–14.120 | 6.703125 |
| dense-1000000 | D | 966,669 | 85.011 | 111.014 | 83.526–256.909 | 47.812500 |
| dense-1000000 | N | 966,669 | 85.322 | 95.367 | 83.982–225.935 | 47.828125 |
| dense-2000000 | D | 1,933,338 | 170.419 | 172.800 | 163.242–174.095 | 92.000000 |
| dense-2000000 | N | 1,933,338 | 170.769 | 174.655 | 163.989–275.357 | 92.015625 |
| external-0-atsp.mzn | D | 6,899 | 7.913 | 9.205 | 6.809–9.309 | 2.359375 |
| external-0-atsp.mzn | N | 6,899 | 7.871 | 9.088 | 6.963–9.282 | 2.406250 |
| external-1-trip_7_4.mzn | D | 85,229 | 16.592 | 17.331 | 15.235–18.575 | 5.546875 |
| external-1-trip_7_4.mzn | N | 85,229 | 16.520 | 17.293 | 15.324–17.612 | 5.562500 |
| external-2-u7g2pref1.dzn | D | 885,733 | 91.700 | 93.651 | 90.454–95.687 | 51.234375 |
| external-2-u7g2pref1.dzn | N | 885,733 | 91.816 | 93.113 | 90.581–94.287 | 51.234375 |

## Ordinary latency and data scaling

The six named ordinary cases have per-case p95 values from 4.918 to 17.331 ms
across D/N settings. Each row has 50 samples, and each meets the 100 ms target;
invalid edits retain exit 2 with no stdout. The table preserves sample spread
and RSS alongside latency.

Dense/generated data and stress cases retain their latency observations without
an ordinary-file pass/fail gate. The default 966,669-byte dense median is
85.011 ms, with a 256.909 ms maximum; the nested-settings maximum is 225.935 ms.
No samples were removed, and these observations do not identify the tail's cause.

For the 966,669-byte dense case, median input throughput is about 10.8 MiB/s in
both settings and RSS is 47.812500/47.828125 MiB. At 1,933,338 bytes, throughput
is about 10.8/10.7 MiB/s and RSS is 92.000000/92.015625 MiB. Median latency and
RSS roughly double across this pair. The 885,733-byte public wide matrix has
about 9.2 MiB/s median input throughput and 51.234375 MiB RSS in both settings.
Throughput here is input MiB divided by the retained median seconds. These
points do not establish bounds for other large shapes or batch memory
accumulation. Unexplained superlinear growth or accumulation still needs a
bounded investigation.

When measured on 2026-10-12, every case up to 100 KiB met the former 50 ms /
32 MiB criteria, while default dense p95 exceeded the former 100 ms criterion
for inputs through 1 MiB. Those dated comparisons remain historical evidence;
the 2026-10-15 contract applies 100 ms only to the named ordinary workloads.

All 1,400 timed saves and 28 native saves complete without a timeout. Successful
cases exit 0 with empty stderr; invalid syntax exits 2 with zero stdout and a
located diagnostic. Every timed capture matches its retained case/settings
streams and the matched native save. The already formatted buffer stays
byte-identical. Successful D/N outputs are identical to each other.

| Case | Output bytes | Status |
| --- | ---: | ---: |
| ordinary-changed | 1,356 | 0 |
| ordinary-formatted | 1,356 | 0 |
| invalid-edited | 0 | 2 |
| matrix | 1,076 | 0 |
| nested | 22,036 | 0 |
| matrix-grown | 10,596 | 0 |
| nested-grown | 609,316 | 0 |
| dense-10000 | 26,338 | 0 |
| dense-100000 | 263,338 | 0 |
| dense-1000000 | 2,633,338 | 0 |
| dense-2000000 | 5,266,672 | 0 |
| external-0-atsp.mzn | 6,309 | 0 |
| external-1-trip_7_4.mzn | 182,774 | 0 |
| external-2-u7g2pref1.dzn | 1,506,703 | 0 |

Current ordinary formatted output is 1,356 bytes; the historical base-081 output
was 1,324 bytes. Layout has changed, so this campaign checks current output
consistency and stability rather than claiming all historical outputs match.
The report retains exact input and output SHA256 values, sample status, diagnostic
text, child CPU and distributions. The three read-only external files were
rehashed against their measured input hashes and remain unchanged.

First use is recorded separately at 5.468 ms wall and 3.358 ms child CPU for an
ordinary save. It passes with complete output. This one observation does not
control cold filesystem caches or explain historical startup delays.

## Source preservation and scope

The existing focused controls pass:

```sh
cargo test --offline -p zincite-fmt --test formatting --test cli
```

The 31 formatting controls compare reparsed structure, significant spelling,
comments and protected bytes, and second-pass stability across the settled
layouts. They include paired implicit/scalar/tuple comprehensions at widths 40
and 120 with two-space, four-space and tab indentation, quantified conditions,
matrices and directives. The seven CLI controls cover language selection,
EditorConfig/CLI precedence, opaque comment bytes and failure behavior.
All ten before/after examples in the [style gallery](../docs/formatting-style.md)
were checked against this release and their second passes match.

The examples are syntax excerpts. No compiler or solver run was needed, and no
full-corpus sweep ran. These named controls do not establish full language,
full-corpus or batch-memory acceptance. No test, formatter option or Rust code
was added for the documentation and measurement work.

## Repeat the bounded run

Build and test outside the measurement window, then use a new report path:

```sh
cargo build --offline --release -p zincite-fmt --bin zincite-fmt
python3 scripts/bench-save.py --binary target/release/zincite-fmt --samples 50 \
  --external /private/tmp/zincite-base080-challenge-a844/2021/ATSP/atsp.mzn \
  --external /private/tmp/zincite-base080-challenge-a844/2021/java-routing/trip_7_4.mzn \
  --external /private/tmp/zincite-base080-challenge-a844/2019/groupsplitter/u7g2pref1.dzn \
  --output target/benchmarks/NEW-SAVE-REPORT.json
```

The explicit external inputs are from the read-only Challenge checkout pinned at
`a8448864fc56162583f24aaf9c25653d93f83765`. Only these three files are read;
there is no directory discovery. The standard driver retains output hashes;
retain complete captures and separate exact-child RSS when repeating this
checkpoint's full protocol.

Local evidence is retained under
`/private/tmp/zincite-base-129/target/benchmarks/base129-current/`.
`campaign/save.json`, `save.log`, `native.json`, `capture-save.py` and the input,
stdout/stderr and native streams retain the successful campaign. `gallery.py`
and `gallery.log` retain example checks. The initial attempt failed in the local
RSS helper because Python 3.12 lacks `os.waitid`; its helper, log and completed
ordinary-save captures remain under `attempt-initial/`. Its partial samples do
not substitute for the successful campaign. The corrected wrapper uses SIGALRM
and `wait4`. Reports and helper scripts remain untracked; the historical
checkpoint is unchanged.
