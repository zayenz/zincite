# Layout measurement performance checkpoint

The formatter stops a prefix preview after it has rendered the first relevant
line and established that a remainder exists. The final release comparison
reduces the dense 966,669-byte save p95 from 137.99 to 125.57 ms with defaults,
and from 138.61 to 125.11 ms with nested EditorConfig files. Formatted bytes and
exit statuses remain identical on the measured cases. The 100 ms / 64 MiB
budget for this size remains unmet.

## Change and measurement

`prefix_exceeds_width` previously rendered the entire subtree, then used only
its first non-leading line and the presence of a remainder. The renderer now
stops emitting and visiting children once those facts are known. A byte cursor
scans only newly appended text, avoiding repeated scans of an unbroken line.
Leading newlines keep their existing treatment. A terminal newline alone does
not end the preview: the suffix still counts when the remainder is empty.
Full node measurements still inspect the first and final lines. The actual
renderer, its layout choices and its public API keep their existing contracts.

The baseline is HEAD `4ea6430c4b10740d0d1fb9cb82013c3ae05f17c8`; the final
candidate changes only `crates/zincite-fmt/src/lib.rs`. Measurements used Rust
1.98.1 release builds, Python 3.14.6 and macOS 26.6.2 arm64 on the host described
in the [lexer checkpoint](lexer-dispatch-performance.md) as an Apple M1 Max
with 64 GiB RAM. Builds used Cargo's existing release settings. The retained
CLI binary SHA256s are:

- Before: `ec1c637bd1158fdd4febaa60dcf2cc79f7505d14d4b37620d00bfbc7d78a9cbc`.
- Final: `0986e6f9d9250bb9aff95f3eccb58d51aae1b031f4ba24d2f15a0f60aef0de17`.

Each table entry has 50 fresh CLI processes. The unchanged `bench-save.py`
driver includes startup, EditorConfig lookup, stdin parsing/formatting and
stdout capture. It alternates the two settings within each case. Defaults and
the nested configuration both resolve to four spaces, tab width four, LF and
width 120. Measurements ran sequentially, with ordinary warm filesystem caches
and no competing benchmark, profile, build or corpus check.

Build and save commands, with `VERSION` equal to `before` or `final`:

```sh
cargo build --release -p zincite-fmt --bin zincite-fmt --example profile-phases
python3 scripts/bench-save.py \
  --binary target/benchmarks/base046/VERSION/zincite-fmt --samples 50 \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/ATSP/atsp.mzn \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/java-routing/trip_7_4.mzn \
  --output target/benchmarks/base046/VERSION.json
```

Raw reports, pinned inputs/binaries and logs are under
`target/benchmarks/base046/`. `pin.json` and `final-pin.json` identify the source,
binaries and inputs. `before.json` and `final.json` contain every timed sample.
The earlier marker-only candidate remains separately recorded as `after.json`;
it is not the final measurement.

## Save results

All arrows show before → final. Latency is milliseconds; RSS is MiB from a
separate save invocation for each case/settings pair.

### Defaults

| Case | Input bytes | p50 ms | p95 ms | Peak child RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| ordinary-changed | 1163 | 4.33 → 4.15 | 5.09 → 4.77 | 2.02 → 1.97 |
| ordinary-formatted | 1324 | 4.40 → 4.65 | 5.02 → 5.15 | 2.03 → 1.98 |
| invalid-edited | 33 | 4.54 → 4.05 | 5.06 → 4.38 | 1.77 → 1.75 |
| matrix | 676 | 4.72 → 4.32 | 5.28 → 4.64 | 2.11 → 2.08 |
| nested | 1133 | 4.98 → 4.81 | 5.41 → 5.48 | 2.27 → 2.19 |
| matrix-grown | 6971 | 7.42 → 6.92 | 8.95 → 7.26 | 3.52 → 3.45 |
| nested-grown | 4493 | 26.30 → 26.45 | 28.38 → 27.74 | 4.39 → 4.45 |
| dense-10000 | 9669 | 5.26 → 5.27 | 6.05 → 5.75 | 3.38 → 3.34 |
| dense-100000 | 96669 | 15.93 → 15.26 | 16.87 → 15.72 | 17.33 → 17.31 |
| dense-1000000 | 966669 | 128.04 → 121.11 | 137.99 → 125.57 | 141.19 → 141.22 |
| dense-2000000 | 1933338 | 251.02 → 240.36 | 262.20 → 245.71 | 278.39 → 279.92 |
| external-0-atsp.mzn | 6899 | 5.46 → 4.96 | 5.99 → 5.60 | 2.62 → 2.48 |
| external-1-trip_7_4.mzn | 85229 | 14.49 → 13.68 | 17.93 → 14.32 | 10.98 → 10.92 |

### Nested EditorConfig

| Case | Input bytes | p50 ms | p95 ms | Peak child RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| ordinary-changed | 1163 | 4.44 → 4.26 | 4.96 → 4.58 | 2.08 → 2.02 |
| ordinary-formatted | 1324 | 4.49 → 4.66 | 5.30 → 5.32 | 2.08 → 2.08 |
| invalid-edited | 33 | 4.62 → 4.10 | 5.04 → 4.72 | 1.80 → 1.80 |
| matrix | 676 | 4.79 → 4.42 | 5.27 → 4.74 | 2.11 → 2.16 |
| nested | 1133 | 5.08 → 4.91 | 5.62 → 5.32 | 2.33 → 2.23 |
| matrix-grown | 6971 | 7.44 → 6.99 | 9.72 → 7.40 | 3.50 → 3.48 |
| nested-grown | 4493 | 26.39 → 26.52 | 27.59 → 28.02 | 4.48 → 4.41 |
| dense-10000 | 9669 | 5.32 → 5.39 | 5.61 → 5.85 | 3.39 → 3.38 |
| dense-100000 | 96669 | 16.00 → 15.35 | 17.28 → 15.96 | 17.34 → 17.33 |
| dense-1000000 | 966669 | 127.72 → 121.48 | 138.61 → 125.11 | 142.52 → 142.77 |
| dense-2000000 | 1933338 | 251.10 → 240.37 | 258.09 → 248.66 | 278.42 → 279.98 |
| external-0-atsp.mzn | 6899 | 5.60 → 5.04 | 5.95 → 5.66 | 2.62 → 2.55 |
| external-1-trip_7_4.mzn | 85229 | 14.54 → 13.75 | 17.70 → 14.61 | 10.98 → 10.94 |

The first candidate had noisy small-matrix and dense-10 KiB tails. Separate
50-sample repeats remain in `repeat-before.json` and `repeat-after.json`;
they are not pooled into the tables. A 100-process-per-binary alternating
matrix check gave p50 4.366 → 4.328 ms and p95 5.274 → 4.948 ms for that
candidate. The final full run no longer shows its matrix tail increase.
The final formatted-file median rose slightly in the full sequential run;
a separate 100-process-per-binary alternating check gave p50 4.082 → 4.064 ms
and p95 4.931 → 4.953 ms. These small differences do not establish a regression
or an improvement on unaffected cases. Raw paired samples remain in
`matrix-alternating.json` and `formatted-alternating.json`.

Doubling dense input from 966,669 to 1,933,338 bytes changes final default p50
from 121.11 to 240.36 ms (1.98×), versus 128.04 to 251.02 ms (1.96×) before.
The 96,669 → 966,669-byte increase is 10× input and 7.94× final p50, versus
8.04× before. The generated dense outputs contain 263,338, 2,633,338 and
5,266,672 bytes at those three sizes. The fourfold nested-depth increase
produces 24,168 → 694,968 output bytes and 4.81 → 26.45 ms final p50.
It shows little save improvement; this change does not resolve repeated
full-node measurements in deeply nested layouts.

The full scan exposed another slow shape:
`2023/table-layout/p2000_m16_r1000_c200.dzn`, with 27,775,980 input bytes and
78,976,026 formatted bytes. Five alternating before/final fresh-process pairs
give median wall times 4,001.8 → 3,941.8 ms, ranges 3,866.0–4,597.1 →
3,888.0–4,080.8 ms, and median child CPU 3,973.7 → 3,926.3 ms. All ten saves
exit 0 with exactly equal output bytes. This small control does not show a
systematic save regression; it cannot establish a p95 improvement. It reads
stdin from the original file and writes stdout to retained files, so it is
separate from the 50-sample IPC benchmark. Per-child RSS is 3,984–4,029 MiB
before and 3,932–3,933 MiB final. `large-table-layout/paired.json` retains each
command, output, status, timing, RSS and original hash.

Every measured case up to 100 KiB stays within 50 ms p95 and 32 MiB RSS.
The dense 966,669-byte case exceeds both its 100 ms and 64 MiB budgets; the
1,933,338-byte case still takes about 249 ms p95 and 280 MiB. The first-use
ordinary save is recorded separately: 473.12 ms wall / 5.868 ms child CPU
before and 360.70 ms / 6.712 ms final. Those isolated observations do not
establish a first-use improvement. Tasks base-047 and base-048 retain the
first-use and memory work.

## Attribution and memory

Native samples of the existing allocation probe's repeated format phase show
distinct prefix and full-node preview branches before the change. The probe
source is unchanged; each executable links its corresponding formatter.
In the dense sample, the principal prefix branch has 292 sampled
frames before and 18 final. The full-node branch remains substantial. These
are inclusive sampled stacks from separate runs, not exclusive percentages;
recursive nested stacks cannot be added together to estimate total cost.
`before-*-native.sample.txt` and `final-*-native.sample.txt` retain the graphs.

The separate one-shot allocation probes give:

| Format phase | Allocation calls before → final | Requested bytes before → final | Retained / peak delta bytes final |
| --- | ---: | ---: | ---: |
| dense-1000000 | 80 → 62 | 73936450 → 65547874 | 2633338 / 31993466 |
| nested-grown | 3665 → 3651 | 77458519 → 75885751 | 694968 / 1841848 |

Dense lexing and parsing keep their existing counts; parsing requests
215,961,645 bytes and retains 140,464,461 bytes. Allocation counters add
overhead, requested bytes include successive allocations, and these phase
deltas are not process RSS or fresh-save latency. The native format loop
reuses a CST and likewise cannot substitute for save measurements.

The benchmark's `/usr/bin/time -l` RSS attempts failed because the sandbox
denied `sysctl kern.clockrate`; their stderr remains in each raw save report.
Separate `os.wait4` probes record each child's actual `ru_maxrss` in Darwin
bytes, divided by 1,048,576 for MiB. Child status and output hashes match the
timed saves. RSS remains essentially unchanged. Initial sandboxed native
sampling failed too; those status files remain alongside successful bounded
native samples collected with approved process inspection.

The profiler and comparisons remain separate, following the scenario
distinctions in [Ruff](https://github.com/astral-sh/ruff/blob/main/CONTRIBUTING.md),
[uv](https://github.com/astral-sh/uv/blob/main/BENCHMARKS.md) and
[ty](https://github.com/astral-sh/ruff/blob/main/scripts/ty_benchmark/README.md).
Zincite's fresh-save budgets govern this checkpoint.

## Preservation and validation

The 26 case/settings pairs preserve exact formatted bytes and expected exit
statuses across every before/final timed sample and separate output probe.
Invalid edited input exits 2 with no formatted bytes; its diagnostic is
identical after substituting only the temporary stdin filepath. All 22
existing public formatter checks pass, including tabs/Unicode, unlimited
width, comments, expanded layouts, matrices and protected text.

A 1,054-byte, 256-operand unbroken sum control is accepted by MiniZinc 2.10.1
`--model-check-only` without solving. Before, first-candidate and final CLIs
produce the same 1,086 output bytes. `long-unbroken.json` retains commands,
hashes and the small allocation-phase checks. Their one-shot timings are
diagnostic observations, not a separate speed claim.

The final corpus scan uses the current checker/runner copied to
`target/corpus/base046-final/pinned/`, with a 10-second per-file deadline and
no `--rules`. It finishes in 399.61 seconds with 6,417 unique rows: 6,347
completed checks, 25 deadlines, 36 parse assessments, four data files containing
model items and five invalid UTF-8 inputs. Every completed check preserves
structure, spelling and protected bytes, reparses without errors and is stable
on the second pass. There are no completed-input preservation failures or
checker crashes. All 6,417 original files were independently rehashed after
the run and match the preceding base-045 inventory. Source, runner and checker
hashes still match the final pin.

The base-027 checkpoint had 6,345 completed checks and 27 deadlines; its other
cause counts match. Three formerly timed-out files complete in the final run,
while one formerly completed file reaches the deadline. The earlier fixed
first-candidate scan remains in `target/corpus/base046/`: it completed 6,349
checks with 23 deadlines, two fewer than final. The per-path transitions remain
in `target/corpus/base046-final/reconciliation.json`; results from these
different binaries/runs are not combined. Deadlines remain unobserved cases,
and the retained syntax assessments still need their existing follow-ups.
The runner exits 1 for these explicit gaps, not a runner failure. This scan
does not establish full syntax or semantic coverage.

The large table-layout file completes the final checker in a separate
10.485-second replay with a finite 30-second deadline. It preserves structure,
spelling and protected bytes, reparses cleanly and is idempotent. The first
candidate's full run completed it in 9.678 seconds; the final full run reached
its 10-second deadline. The supplemental replay confirms preservation while
the paired CLI results do not show a consistent save slowdown. Its check cost
sits around the original deadline, and its large memory/latency cost remains
explicit. `target/corpus/base046-final/large-table-layout-check.json` records
this replay; the original full-scan row and its counts remain unchanged.

Final `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` pass. Logs and ownership evidence are linked from
`/private/tmp/zincite-base046-validation.json`. The initial preview-unwind
failure remains in `first-public.log`; the existing behavior checks exposed
and verified its correction without adding another test.
