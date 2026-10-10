# Save command acceptance checkpoint

The base-048 and base-079 results below are historical. The current base-081
recheck at the end passes the unchanged save budgets on every retained
representative case. Current full-corpus validation remains deferred; the
older full-corpus report does not pin the current formatter source.

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
  --external /private/tmp/zincite-base080-challenge-a844/2021/ATSP/atsp.mzn \
  --external /private/tmp/zincite-base080-challenge-a844/2021/java-routing/trip_7_4.mzn \
  --external /private/tmp/zincite-base080-challenge-a844/2019/groupsplitter/u7g2pref1.dzn \
  --output target/benchmarks/NEW-SAVE-REPORT.json
cargo test --offline -p zincite-fmt --test cli
```

The replay paths use the complete read-only replacement Challenge checkout at
`a8448864fc56162583f24aaf9c25653d93f83765`, whose 2,040 source paths and
hashes match the historical inventory. The old archive root is incomplete.
Historical measurements below retain their original paths and evidence.
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

## Base-081: formatter-only candidate and remaining save gates

The formatter borrows exact scalar cell spelling for an eligible whole matrix,
including adjacent signed numeric literals. It reuses scalar widths and the
current row column instead of rendering and copying every cell. Indexed rows,
headers, comments, nested or compound cells, explicit line breaks and tab-sensitive
spelling retain the existing renderer. Shared breaks, comma padding and complete
output remain unchanged. At this formatter-only stage, no parser/CST change, cache, daemon or new
dependency remained.

The retained wide shape produces the same 1,506,703 output bytes as the baseline.
Matched native checks reduce child CPU from about 388–392 ms to 95.764–97.959 ms.
These commands use `--check`; their wall observations are retained separately
and do not substitute for stdin-save measurements. Formatting allocation calls
fall from 6,250,148 to 384 and requested bytes from 198,862,238 to 10,075,651.
The tracked formatting peak above the retained CST falls from 15,857,904 to
2,474,444 bytes. Requested allocations, tracked live bytes and native RSS are
different measurements.

### Fresh-save campaign

Each row below retains 50 fresh processes per configuration, complete stdout,
separate raw stderr, status, child CPU and unchanged input/configuration pins.
All 1,400 samples complete with the expected status and output hash; no sample
times out or is removed. An additional first-use observation and 28 supplementary
`/usr/bin/time` attempts bring the receipt count to 1,429. The separate native
controls use the exact same commands and actual `.mzn`/`.dzn` destination suffix.

| Case / input bytes | Default p95 ms | Nested p95 ms | Default CPU p95 ms | Nested CPU p95 ms | Default RSS MiB | Nested RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ordinary-changed / 1163 | 206.679 | 181.970 | 5.567 | 5.353 | 2.031250 | 2.062500 |
| ordinary-formatted / 1324 | 174.186 | 147.411 | 5.951 | 5.688 | 2.046875 | 2.046875 |
| invalid-edited / 33 | 184.593 | 174.808 | 5.780 | 5.756 | 1.796875 | 1.828125 |
| matrix / 676 | 213.023 | 232.734 | 5.201 | 5.289 | 1.968750 | 1.968750 |
| nested / 1133 | 196.018 | 202.920 | 5.745 | 6.345 | 2.234375 | 2.234375 |
| matrix-grown / 6971 | 286.736 | 238.402 | 5.994 | 5.905 | 2.546875 | 2.593750 |
| nested-grown / 4493 | 765.407 | 815.383 | 29.309 | 31.894 | 3.578125 | 4.078125 |
| dense-10000 / 9669 | 210.394 | 190.592 | 6.702 | 6.341 | 2.562500 | 2.593750 |
| dense-100000 / 96669 | 527.191 | 456.309 | 14.437 | 14.974 | 9.312500 | 9.375000 |
| dense-1000000 / 966669 | 3346.419 | 3945.075 | 95.444 | 97.926 | 64.000000 | 64.031250 |
| dense-2000000 / 1933338 | 6890.144 | 6518.058 | 192.469 | 190.483 | 124.500000 | 124.500000 |
| external-0-atsp.mzn / 6899 | 301.582 | 264.631 | 6.251 | 6.493 | 2.375000 | 2.406250 |
| external-1-trip_7_4.mzn / 85229 | 551.491 | 551.607 | 14.925 | 15.225 | 6.640625 | 6.671875 |
| external-2-u7g2pref1.dzn / 885733 | 3704.110 | 3621.275 | 109.165 | 108.986 | 75.468750 | 75.609375 |

Every size-gated case misses its wall-time budget in this campaign. The public
wide input also exceeds 64 MiB; the dense 966,669-byte nested control exceeds
it by 0.03125 MiB. The 1,933,338-byte dense case is retained as scaling evidence
outside the interactive size range. Correct output and lower CPU do not satisfy
the unchanged wall-time or memory gates.

The host had substantial concurrent activity: load averages were approximately
594/615/609 at setup and 778/810/760 after the campaign. A separate read-only
process observation counted 228 runnable processes. This establishes concurrent
host activity; it does not identify its exact share of any sample or justify
relaxing a budget. Ordinary changed saves use about 5.4–5.6 ms child CPU at p95
while their measured wall p95 is 182–207 ms. The first ordinary campaign save
is retained separately at 624.346 ms wall / 4.867 ms child CPU, after earlier
invocations of that executable. Historical fresh-copy help controls remain
first-use evidence; this run does not control cold caches or establish a specific
host launch cause.

### Retained memory boundary

An owned held parse-only child reports 71.53125 MiB native high-water RSS. Its
public types are 40-byte `SyntaxNode`, 40-byte `SyntaxElement` and 24-byte `Token`.
The wide CST has 695,645 tokens and 367,170 child arrays: 347,753 request 40 bytes,
19,019 request 360 bytes and 318 request about 25 KiB. Child arrays request
42,512,560 bytes and the token buffer requests 16,695,480 bytes. The tracked
parsed result retains 60,093,773 bytes, with a 60,126,013-byte construction peak.

The held child keeps both the original string and the parsed source copy.
`heap` reports 86,869,056 allocator bytes across 367,365 allocations; its dominant
48-byte, 640-byte and 48-KiB classes closely correlate with those child-array
counts. `vmmap -summary` reports a 71.0-MiB footprint and allocator dirty/swapped
pages separately. These observations expose allocator size-class rounding and
retained CST allocation as necessary investigation boundaries; the histogram
does not establish each allocation address or explain every native RSS byte.

Exact lexer capacity, numeric row reservation and their combination reduce
allocation traffic but leave parsed retained bytes unchanged and do not show
a native memory benefit. At this stage, both experimental syntax files were restored to their exact
originals.
All candidate sources, binaries, noisy timings and unsuccessful experiments
remain under `target/benchmarks/base081`. The formatter-only candidate included no representation rewrite or allocator
replacement. Its remaining native memory miss motivated the private-node
experiment below, with full fidelity, drop and save gates retained.

### Validation and evidence

The six established synthetic/public matrix and table controls retain byte-identical
complete output and expected native statuses. The public wide, synthetic 30,000-cell
matrix and 619,533/27,775,980-byte tables pass clean parse/reparse, token/tree
coverage, spelling, structure, protected-byte and idempotence checks. Three exact
scope-drop iterations return to their baseline allocations and leave `ParsedFile`
usable before drop. The large table fidelity run retains 69.121182 seconds wall /
10.978771 seconds CPU and 3,923.218750 MiB; its separate drop run retains
75.197283 seconds / 12.986823 seconds CPU and 2,226.468750 MiB.

Workspace formatting, Clippy with warnings denied and all 189 tests pass on the
formatter-only candidate. The formatter-only candidate completes the full 6,417-input no-rules syntax/format
recheck: 6,396 clean results and the exact 21 prior exclusions, zero timeout or
unobserved rows. All originals are rehashed unchanged. The runner exits 1 for
those exclusions; it is terminal and reaped. The complete recovered archive and
unchanged local/supplement originals retain the base-080 compiler classifications.

Local raw evidence includes `save-final`, `native-final`, `final-matched`,
`memory-attribution`, `tree-sizes`, `cargo-final`, source/binary pins and the full
corpus receipts. These ignored reports preserve exact commands, finite deadlines,
raw streams and terminal/reaped results. The save acceptance remains open.

## Base-081: selected private node storage

The selected candidate retains the formatter repair above and adds one private
`Box<NodeData>` containing the unchanged node kind, full `usize` range and child
slice. Public syntax variants and accessors remain unchanged. Token-heavy child
slices become smaller, with an additional allocation for each node. No compact
range encoding, arena, cache or dependency is introduced.

On the public wide input, tracked parsed retention falls from 60,093,773 to
49,273,037 bytes; construction peak is 49,285,773 bytes. Parse allocation calls
increase from 427,454 to 794,624. Fresh public parse-only controls measure
57.156250/57.359375 MiB, but child CPU increases to 59.480/57.850 ms from the
prior 52.722 ms observation. Formatting still makes 384 tracked allocation calls,
requests 10,075,651 bytes and peaks at 2,474,444 bytes above the retained CST.
These allocation counters and native RSS are distinct observations.

### Selected fresh saves

All 1,400 fresh formal samples have the expected status and complete output hash,
with zero timeouts. The first-use observation and 28 supplementary time controls
remain separate, giving 1,429 retained receipts. Native RSS below comes from
separate frozen-binary children with the same stdin bytes, destination suffix
and configuration arguments, including `.dzn` for the public wide input.

| Case / input bytes | Default p95 ms | Nested p95 ms | Default CPU p95 ms | Nested CPU p95 ms | Default RSS MiB | Nested RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ordinary-changed / 1163 | 265.812 | 245.293 | 4.945 | 4.858 | 1.953125 | 1.953125 |
| ordinary-formatted / 1324 | 230.182 | 241.287 | 4.983 | 5.276 | 1.984375 | 1.984375 |
| invalid-edited / 33 | 174.435 | 209.545 | 4.949 | 4.467 | 1.734375 | 1.796875 |
| matrix / 676 | 150.078 | 162.128 | 4.558 | 4.653 | 1.921875 | 1.953125 |
| nested / 1133 | 245.311 | 273.288 | 5.557 | 6.067 | 2.125000 | 2.140625 |
| matrix-grown / 6971 | 222.954 | 287.549 | 5.190 | 5.816 | 2.453125 | 2.468750 |
| nested-grown / 4493 | 1151.643 | 987.108 | 29.512 | 28.717 | 3.921875 | 3.828125 |
| dense-10000 / 9669 | 279.109 | 219.576 | 5.838 | 6.089 | 2.531250 | 2.515625 |
| dense-100000 / 96669 | 640.905 | 652.336 | 15.616 | 14.723 | 7.281250 | 7.312500 |
| dense-1000000 / 966669 | 4033.227 | 3951.877 | 97.190 | 97.773 | 53.875000 | 53.875000 |
| dense-2000000 / 1933338 | 7504.853 | 8134.961 | 186.783 | 184.537 | 104.171875 | 104.281250 |
| external-0-atsp.mzn / 6899 | 285.110 | 254.076 | 6.258 | 6.279 | 2.312500 | 2.406250 |
| external-1-trip_7_4.mzn / 85229 | 538.423 | 501.805 | 15.331 | 15.050 | 5.765625 | 5.796875 |
| external-2-u7g2pref1.dzn / 885733 | 3916.040 | 4045.999 | 106.290 | 105.316 | 60.859375 | 60.562500 |

Every size-gated case still misses its wall-time budget. All separate native
memory controls within that size range pass: public wide default/nested RSS is
60.859375/60.562500 MiB and dense 966,669-byte RSS is 53.875 MiB for both settings.
The larger dense scaling case remains outside the interactive gate. Public wide
child CPU p95 is 106.290/105.316 ms; a less busy host is not proof that this
candidate will meet the 100 ms wall limit.

After this campaign, a separate 50-launch diagnostic invokes only `--help` on
the frozen binary. Every child exits 0 with identical complete output and no
timeout. Its p95 is 201.446 ms wall and 5.612 ms child CPU. Help exits before
configuration lookup or input parsing. This demonstrates delay outside formatter
work, but does not establish its precise cause, cold-cache behavior or a budget
exemption. Those launches are excluded from the formal save samples.

### Selected behavior and tradeoffs

All six matched controls retain byte-identical complete output. Four fidelity
controls pass token/tree coverage, clean parse/reparse, spelling, structure,
protected-byte and idempotence checks. Exact scope-drop controls return to their
allocation baselines and leave the parsed result usable. The large table native
check uses about 1,743 MiB and 3.906–3.909 seconds child CPU, lower than the
formatter-only observations. Its multi-parse fidelity control instead increases
RSS from 3,923.218750 to 4,562.750000 MiB, with child CPU decreasing from
10.978771 to 9.432835 seconds. The separate selected drop control measures
1,767.953125 MiB and 12.109203 seconds child CPU. These controls do not substitute
for stdin-save memory measurements, and the fidelity RSS regression remains a
limit of the selected storage choice.

The public wide model/data pair passes original and fully staged Gecode
compile-only checks under the same current MiniZinc/backend/standard-library
settings, with no solver execution. The two raw FlatZinc files differ only in
the command-invocation comment on line 3; every other byte matches. This establishes
this explicit pair's eligibility, without a blanket equivalence claim.

Selected workspace formatting, Clippy with warnings denied and all 189 tests
pass. The selected full no-rules corpus completes all 6,417 unique baseline
inputs: 2,040 Challenge, 4,287 local and 90 supplement. All 6,396 clean rows
pass every fidelity gate; the same 21 prior exclusions remain (five invalid
UTF-8, twelve parse/grammar cases and four data files containing model items).
Every row is reported with checker status 0, with no timeout or unobserved
result. All 6,417 originals and 12,834 raw checker streams are physically
rechecked; input sizes and hashes match the baseline. Encoding exclusions remain
coverage limits, without a compiler-invalid inference. The runner exits 1 for
the classified exclusions and is terminal and reaped. Its aggregate child
high-water is 7,200.343750 MiB, with 4,799.054273 seconds wall and 288.964707
seconds CPU; this is neither a stdin-save measurement nor a source memory floor.
Earlier formatter-only corpus results above remain separate evidence.
Raw selected evidence is under `save-selected`, `help-selected`, `after8-controls`,
`after8-matched`, `wide-compile`, `cargo-selected` and `full-corpus-selected`, with
frozen source/binary pins and complete terminal child receipts. Save acceptance
remains open because the wall-time gates are unmet.


## Base-081: current save recheck, 2026-10-10

The existing renderer and syntax storage meet the unchanged save budgets on the
retained public wide input and all established representative cases in this
recheck. No further source change was needed. This is current save evidence;
it does not complete the outstanding current full-corpus validation.

The checkout began at `9fa76a84a3cbb78a3623a6b9be0dcf9ae4276deb`.
The measured build is pinned at `596a170c7c89e3cf32d30529dc9627e5b5691967`,
which added only task records and the generated task index. Formatter and parser
source stayed unchanged during this work. The release formatter SHA256 is
`2d276437aa58bb798808489b8aabe789a12f0ea331bb02947309d42d0b4d88f3`.
The host is the user's M1 Max with 64 GiB, macOS 26.6.2 arm64, Rust 1.98.1 and
Python 3.14.6. Host load and warm filesystem caches remain uncontrolled.
The sandbox denied the supplementary hardware `sysctl` query; the hardware
identity comes from the retained host record.

The clean campaign uses the unchanged `scripts/bench-save.py` case generator,
settings, 50 fresh processes per case/configuration and nearest-rank p95. A
private wrapper captures every complete stdout and raw stderr, status and child
CPU. Builds, tests and profiling ran outside this campaign. The earlier `before`
campaign accidentally overlapped the baseline formatter test at its start; all
of its samples remain recorded as a noisy attempt, and none substitutes for the
clean campaign below. The two campaigns have identical complete outputs and
statuses. No sample was removed or pooled.

Separate native stdin saves retain complete streams and exact-child Darwin
`wait4` RSS, with a 30-second watchdog and reaped children. They receive the same
input bytes and default/nested settings, with the actual destination suffix.
Their output bytes and statuses match all 1,400 formal samples. The driver's
28 denied `/usr/bin/time -l` attempts remain in the receipts; the table uses
available separate `wait4` observations. D and N both resolve to four spaces,
tab width four, LF, a final newline, trimming, UTF-8 and width 120.

| Case / input bytes | D p95 ms | N p95 ms | D RSS MiB | N RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| ordinary-changed / 1,163 | 5.009 | 5.103 | 1.968750 | 2.015625 |
| ordinary-formatted / 1,324 | 5.092 | 5.379 | 2.000000 | 2.062500 |
| invalid-edited / 33 | 5.088 | 5.133 | 1.781250 | 1.796875 |
| matrix / 676 | 4.971 | 4.972 | 1.828125 | 1.843750 |
| nested / 1,133 | 6.010 | 5.989 | 2.203125 | 2.218750 |
| matrix-grown / 6,971 | 5.577 | 5.548 | 2.296875 | 2.312500 |
| nested-grown / 4,493 | 27.398 | 27.694 | 3.718750 | 3.453125 |
| dense-10000 / 9,669 | 5.945 | 5.919 | 2.375000 | 2.390625 |
| dense-100000 / 96,669 | 13.307 | 13.406 | 6.625000 | 6.656250 |
| dense-1000000 / 966,669 | 87.134 | 87.118 | 47.796875 | 47.812500 |
| dense-2000000 / 1,933,338 | 167.650 | 167.321 | 91.984375 | 91.984375 |
| external-0-atsp.mzn / 6,899 | 6.296 | 6.230 | 2.312500 | 2.328125 |
| external-1-trip_7_4.mzn / 85,229 | 14.142 | 14.324 | 5.484375 | 5.484375 |
| external-2-u7g2pref1.dzn / 885,733 | 89.184 | 89.121 | 51.406250 | 51.375000 |

Every case up to 100 KiB passes 50 ms / 32 MiB; both larger representative
cases within 1 MiB pass 100 ms / 64 MiB. Wide p50 is 87.512/87.506 ms and its
min–max is 86.448–89.799/86.539–90.175 ms. Dense 966,669-byte p50 is
84.639/84.335 ms, with min–max 83.228–87.405/83.251–89.016 ms.
The nested-grown maximum of 73.201 ms remains visible although its 27.694 ms
p95 passes. Per-case p50, spread, all samples and child CPU remain in the raw
report. Wide child CPU p95 is 80.816/80.855 ms; it does not replace wall time.
The 1,933,338-byte dense case remains supported and outside the interactive
size range; its roughly doubled latency/RSS shows no unexplained superlinear
increase for this size pair.

### Current local attribution and behavior

Two current native wide `--check` runs take 76.495/76.131 ms child CPU and
51.000000/51.921875 MiB RSS. All six retained matrix/table controls produce
complete output byte-identical to the historical pre-repair output. All checks
exit 1 with empty streams; output runs exit 0 with empty stderr. The public wide
output remains 1,506,703 bytes. These file-backed checks are separate from stdin
save measurements.

The existing phase probe records 384 formatting allocation calls, 10,075,651
requested bytes and 2,474,444 peak bytes above the retained CST on the wide
input, matching the earlier scalar-cell repair's counters. The retained
pre-repair result had 6,250,148 calls, 198,862,238 requested bytes and a
15,857,904-byte peak. Current parsed retention is 46,646,349 tracked bytes,
with a 46,652,845-byte construction peak. This checkout also contains later
syntax storage changes; the current native memory improvement cannot be
attributed solely to the formatter repair. Tracked bytes and native high-water
RSS remain separate measurements.

A separate three-second sample of the owned format-only probe retains 2,186
stacks: 1,043 pass through item rendering and 811 through layout's lexer path.
The six-second probe completes 155 uncached format iterations over one retained
CST. This identifies remaining matrix traversal and lexical layout work; it is
not an end-to-end save benchmark or a whole-process CPU percentage. The first
sandboxed sampler attempt exits 255 because it cannot examine the child; that
attempt and its completed/reaped probe remain recorded. A scoped retry examining
only its own child succeeds. Passing current gates gives no reason to add another
renderer optimization.

Current public wide, 30,000-column synthetic, 619,533-byte table and
27,775,980-byte table checks pass clean parse/reparse, token/tree coverage,
spelling, structure, protected bytes and idempotence. Three exact scope-drop
iterations per input return to their baseline allocations and leave `ParsedFile`
usable before drop. The large table fidelity and drop runs remain separate
multi-parse controls, not save-memory measurements. All external control inputs
are rehashed unchanged. Native/checker/drop children have finite 120/300-second
caps, complete streams and terminal reaped receipts.

### First use and remaining validation

The clean campaign's first ordinary invocation is retained separately:
5.943 ms wall / 3.761 ms child CPU. A new identical help executable copy takes
4.674 ms wall / 3.168 ms CPU, then 4.016/2.660 ms. A new identical save copy
takes 5.069/3.509 ms, then 4.294/2.916 ms. These bounded observations show no
material startup delay here. They neither control cold caches nor explain the
historical host launch delays; that earlier external limitation remains.

No corpus sweep ran, as explicitly requested by the user. Fresh full no-rules
corpus validation is deferred. The retained selected corpus report pins different
formatter/parser sources and cannot establish current full-corpus fidelity.
The current focused controls above establish only their named inputs. No new
layout, parser, CST or public API change was introduced in this recheck.

Current workspace formatting, Clippy with warnings denied and all 298 workspace
tests pass. No new test was added for this measurement and documentation update.
All raw attempts, distributions, physical capture comparisons, native resources,
allocation/drop output, profiler stacks, first-use controls, source/binary pins
and command wrappers remain ignored under `target/benchmarks/base081-current`.
Repeat the clean driver protocol with a new output path and retain complete
captures; run native/profiler controls and tests outside that timed campaign.
