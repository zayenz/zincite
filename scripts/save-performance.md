# Save command acceptance checkpoint

The base-048 case set below is historical. The base-079 wide-matrix recheck at
the end adds a representative shape that fails the unchanged save budget.
Final save acceptance therefore remains blocked despite the earlier passing set.

The existing one-shot release command meets the unchanged repeated-save budgets
on the user's M1 Max. No Rust, CLI configuration or benchmark-driver change was
needed. The [editor recipe](../docs/format-on-save.md) uses the edited buffer on
stdin, its destination in `--stdin-filepath`, separate stdout/stderr and buffer
replacement only after status 0 and complete output capture.

Measurements ran on 2026-10-02 at HEAD
`4a1b2aac38f050d259f096f0a82ccd78c50e3963`, macOS 26.6.2 arm64,
Apple M1 Max with 64 GiB, Rust 1.98.1 and Python 3.14.6. Cargo's release build
settings are unchanged. The pinned formatter SHA256 is
`c18cb07fb523a61078dd665778c17cfcc40a3dae84b721c3c421c7f4d49c9a2a` and matches base-077's final executable. Builds and timed
campaigns ran sequentially; coordinator reviews read existing source/reports.
Host load and ordinary warm filesystem caches were uncontrolled.

## Repeated saves

The unchanged `scripts/bench-save.py` supplies all 13 named cases, including
ordinary changed/already formatted buffers, invalid syntax, matrices, comments,
nested expressions, dense size families and two read-only Challenge models.
A private wrapper retains every fully captured stdout and decoded UTF-8 stderr
instead of retaining only output hashes. Each case/settings row has 50 fresh
processes; p95 is the nearest-rank 48th sorted sample. Latency includes launch,
stdin/stdout transfer, configuration lookup, parsing and formatting. No samples
are removed or pooled. Default (D) and nested EditorConfig (N) both select four
spaces, tab width four, LF, a final newline, trimming, UTF-8 and width 120.

Separate matched saves retain raw stdout/stderr files and exact-child `wait4`
RSS. On Darwin `ru_maxrss` is bytes, divided by 1,048,576 for MiB. Each has a
30-second watchdog and is reaped. The driver's denied `/usr/bin/time -l`
probes remain visible: `sysctl kern.clockrate` is unavailable in this sandbox.
That limitation does not affect the available native `wait4` measurements.

| Case | Settings | Input bytes | p50 ms | p95 ms | min–max ms | Native RSS MiB | Input MiB/s |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: |
| ordinary-changed | D | 1,163 | 4.485 | 5.139 | 4.224–5.473 | 1.984375 | 0.24 |
| ordinary-changed | N | 1,163 | 4.566 | 5.250 | 4.257–5.638 | 1.984375 | 0.24 |
| ordinary-formatted | D | 1,324 | 4.548 | 5.458 | 4.222–6.569 | 1.984375 | 0.27 |
| ordinary-formatted | N | 1,324 | 4.597 | 5.203 | 4.294–5.914 | 2.062500 | 0.27 |
| invalid-edited | D | 33 | 4.408 | 5.186 | 4.082–5.795 | 1.796875 | 0.01 |
| invalid-edited | N | 33 | 4.488 | 5.045 | 4.124–5.173 | 1.828125 | 0.01 |
| matrix | D | 676 | 4.517 | 5.173 | 4.334–5.608 | 2.031250 | 0.14 |
| matrix | N | 676 | 4.575 | 5.152 | 4.380–5.416 | 2.093750 | 0.14 |
| nested | D | 1,133 | 5.080 | 6.274 | 4.835–6.682 | 2.218750 | 0.21 |
| nested | N | 1,133 | 5.151 | 6.166 | 4.920–6.413 | 2.250000 | 0.20 |
| matrix-grown | D | 6,971 | 7.149 | 7.605 | 6.809–8.238 | 2.921875 | 0.93 |
| matrix-grown | N | 6,971 | 7.143 | 7.938 | 6.859–9.530 | 2.968750 | 0.91 |
| nested-grown | D | 4,493 | 26.035 | 27.575 | 25.745–28.179 | 3.578125 | 0.16 |
| nested-grown | N | 4,493 | 26.216 | 27.223 | 25.704–32.634 | 3.609375 | 0.16 |
| dense-10000 | D | 9,669 | 5.174 | 5.852 | 4.838–6.460 | 2.531250 | 1.75 |
| dense-10000 | N | 9,669 | 5.307 | 6.093 | 5.059–6.250 | 2.578125 | 1.71 |
| dense-100000 | D | 96,669 | 13.035 | 14.094 | 12.679–14.234 | 9.578125 | 7.03 |
| dense-100000 | N | 96,669 | 13.143 | 14.268 | 12.832–16.753 | 9.328125 | 6.92 |
| dense-1000000 | D | 966,669 | 91.267 | 93.275 | 89.108–94.379 | 63.968750 | 10.08 |
| dense-1000000 | N | 966,669 | 91.389 | 93.267 | 89.274–101.281 | 63.968750 | 10.05 |
| dense-2000000 | D | 1,933,338 | 178.466 | 186.165 | 174.313–189.570 | 124.437500 | 10.29 |
| dense-2000000 | N | 1,933,338 | 179.152 | 185.997 | 174.951–198.953 | 124.453125 | 10.26 |
| external-0-atsp.mzn | D | 6,899 | 5.349 | 6.213 | 5.130–6.593 | 2.328125 | 1.20 |
| external-0-atsp.mzn | N | 6,899 | 5.438 | 6.055 | 5.213–6.221 | 2.437500 | 1.19 |
| external-1-trip_7_4.mzn | D | 85,229 | 13.614 | 14.682 | 13.254–24.022 | 6.562500 | 5.83 |
| external-1-trip_7_4.mzn | N | 85,229 | 13.822 | 14.611 | 13.311–15.363 | 6.656250 | 5.85 |
| invalid-config | D | 8 | 3.987 | 4.566 | 3.729–5.602 | 1.687500 | — |
| invalid-config | N | 8 | 3.956 | 4.439 | 3.663–5.083 | 1.687500 | — |

Every case at or below 100 KiB passes 50 ms / 32 MiB; the 966,669-byte dense
case passes 100 ms / 64 MiB in both settings. Its 63.968750 MiB RSS leaves
32 KiB of observed headroom. The nested dense maximum is 101.281 ms, retained
as one sample outside 100 ms; its p95 still passes. This is an observed local
result, not a guarantee for other machines or every possible input shape.

All 1,300 timed case outputs match their native counterparts and base-077's
final output hashes/statuses. Invalid syntax exits 2 with zero stdout and a
located diagnostic. The additional 100 invalid-configuration saves also exit
2 with zero stdout and complete configuration diagnostics. None times out.
The 1,933,338-byte dense case exceeds the budget's size range but remains
supported: roughly twice the latency/RSS of the 966,669-byte case and about
10 MiB/s input throughput. This size pair shows no unexplained superlinear
growth; it does not establish a bound for all larger shapes.

## Edited-buffer acceptance

A separate small external-formatter adapter sends unsaved in-memory bytes that
differ from the saved disk file to a fresh process. It captures stdout before
changing the buffer and retains the original on failure. The supplied destination
path resolves nested EditorConfig: the model's section selects six spaces and
CRLF, while a `.dzn` destination selects data syntax and two spaces/CRLF.

Six checks pass: changed model, already formatted model, changed data, invalid
syntax, invalid directive and invalid configuration. Success uses the complete
expected bytes with empty stderr. Each failure exits 2 with empty stdout, retains
the original buffer byte-for-byte and keeps the existing disk file unchanged.
The accepted changed output is stable on the next save. Raw inputs, outputs,
diagnostics and resulting buffer snapshots remain under `editor-final/streams`.
This exercises the external-formatter adapter contract; no particular editor
integration or plugin is installed or claimed to have been tested.

The first adapter attempt had an incorrect expected range spelling (`1..2`
instead of `1 .. 2`), so its private assertion failed after a successful formatter
run. Its script, stdout and failure log remain under `acceptance-initial.py`,
`editor/` and `acceptance.log`. The corrected expectation passes in
`acceptance-final.py`; no formatter behavior changed.

## First use

First invocations remain separate from the repeated-save distribution:

| Observation | Wall ms | Child CPU ms |
| --- | ---: | ---: |
| Campaign, first ordinary save from new executable copy | 352.902 | 7.817 |
| New identical copy, first `--help` | 397.300 | 5.867 |
| Same help copy, second invocation | 4.095 | 2.606 |
| Another identical copy, first ordinary save | 194.288 | 3.762 |
| Same format copy, second invocation | 4.570 | 2.999 |

The identical bytes, large wall/child-CPU gap and delay on `--help` demonstrate
that configuration lookup, parsing and formatting are not necessary for this
material first-use delay. `--help` returns before those operations. The controls
change the executable's filesystem identity and do not control host scheduling,
filesystem caches or OS executable checks. They narrow the problem to the host
launch/first-execution boundary, but do not identify a specific macOS service or
prove cold filesystem I/O. Host-level attribution is an explicit external
limitation of this run. No startup cache, daemon or speculative CLI fix was added.
A bounded future investigation would trace a fresh executable's launch on the
host while comparing `--help` and stdin saves, if first-use latency becomes a
release blocker. It must retain the repeated-save budgets unchanged.

## Scope and repeating the checks

The command's stdin branch reads one buffer, resolves only its supplied path's
EditorConfig, parses/formats it completely and then writes stdout. It performs
no workspace discovery, include loading, semantic lint or compiler invocation.
Existing CLI checks already cover path-selected syntax, EditorConfig precedence,
protected bytes, invalid settings and no output on errors. They pass again; no
new test or Rust change is justified by these acceptance results.

The measurement approach follows the brief's references: fresh uncached work
from [Ruff's formatter](https://astral.sh/blog/the-ruff-formatter), explicitly
cache-qualified scenarios from [uv's benchmark guide](https://github.com/astral-sh/uv/blob/main/BENCHMARKS.md),
and doing only needed computation from [ty's language-server documentation](https://docs.astral.sh/ty/features/language-server/#fine-grained-incrementality).
Zincite's one-shot save has no previous query state.

From the repository root, repeat the standard latency driver with a new output
path; keep builds, tests and other measurements outside the timed run:

```sh
cargo build --offline --release -p zincite-fmt --bin zincite-fmt
python3 scripts/bench-save.py --binary target/release/zincite-fmt --samples 50 \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/ATSP/atsp.mzn \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/java-routing/trip_7_4.mzn \
  --output target/benchmarks/NEW-SAVE-REPORT.json
cargo test --offline -p zincite-fmt --test cli
```

The external paths refer to the read-only Challenge checkout from earlier tasks.
`target/benchmarks/base048` retains hardware/toolchain/source/binary/input pins,
all command receipts, save distributions and streams, the native RSS helper,
invalid-config samples, adapter snapshots and first-use controls. The private
`save-campaign.py`, `acceptance-initial.py`, `acceptance-final.py` and
`first-use.py` record exact supplementary commands and configuration contents;
they use exclusive output paths, so copy them to a new report directory before
replaying. These local reports are ignored by Git.

This task establishes the save-command acceptance, not full corpus or batch
acceptance. The [base-077 checkpoint](syntax-memory-performance.md#base-077-current-dense-save-budget-results)
retains 44 corpus gaps, large public-table results and native RSS growth across
independent files despite released tracked allocations. Those limitations remain.
Task records, index, completion and commit belong to the coordinator. The six
pre-existing foreign files remain unchanged and unstaged.


## Base-079: wide-matrix save gate

The local row-classification repair preserves exact matrix output and meets its
bounded native `--check` targets. The newly measured public wide matrix still
fails the unchanged format-on-save gate. Its input is 885,733 bytes:
`2019/groupsplitter/u7g2pref1.dzn` from the read-only Challenge checkout.
Fifty fresh stdin processes per default/nested EditorConfig entry retain full
stdout, separate diagnostics, status and child CPU. Separate matched native stdin
saves capture raw complete streams and exact-child `wait4` RSS. Each save exits 0
and matches the prior formatting output. No samples are removed or pooled.

| Shape | Default p95 ms | Nested p95 ms | Default RSS MiB | Nested RSS MiB | 100 ms / 64 MiB |
| --- | ---: | ---: | ---: | ---: | --- |
| Public wide matrix | 380.463 | 382.080 | 106.296875 | 105.125000 | FAIL both |
| Dense 966,669-byte array | 90.949 | 90.624 | 63.953125 | 63.968750 | pass |

All 13 established named cases remain in the same 50-sample campaign; each
representative input up to 1 MiB retains its passing per-case gate. The new wide
shape prevents a final acceptance claim. The [matrix checkpoint](batch-performance.md#matrix-width-repair--base-079)
records the native targets, exact outputs, allocation/drop observations, full
native repeats, corpus gaps and bounded follow-up recommendation. Actual source
and binary pins plus all 1,400 samples and separate native streams remain under
`target/benchmarks/base079`. The existing driver/probe are unchanged.

First campaign use is 5.383 ms wall / 3.131 ms child CPU after earlier matched
invocations on that executable. First `--help` observations for immutable copies
remain separate. The base-048 fresh-copy launch limitation remains; this recheck
does not establish controlled cold-cache startup performance.
