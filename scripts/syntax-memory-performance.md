# Syntax memory performance checkpoint

Finished CST child buffers now use exact-length owned slices. On the dense
966,669-byte save, retained parse allocation falls from 140,464,461 to
66,132,733 bytes, and separate native child RSS falls from about 143 to
91 MiB. The 100 ms / 64 MiB save budget still fails. This checkpoint records
that remaining acceptance blocker; it does not claim final performance acceptance.

The parser still builds the same recursive CST, owns the original source and
retains every token index, byte range, node distinction and recovery path.
`children()` and `child_nodes()` keep their borrowed, source-ordered contracts.
The root, ordinary/error nodes and empty matrix rows finalize their child storage.
Atom heads start with capacity one, so common single-token atoms avoid growing
to four slots and then shrinking. The formatter gathers and validates protected
rendered ranges, then `LexedSource::into_source()` releases its temporary tokens
before allocating converted output. Formatting continues to borrow the caller's
`ParsedFile`; the caller can inspect and lint it afterward.

No parser/CST replacement, unsafe packing, new crate, allocation/layout test or
timing test was added. The existing behavior checks cover the changed contracts.

Measurements ran on 2026-10-01, macOS 26.6.2 arm64, Apple M1 Max with 64 GiB
RAM, Rust 1.98.1 and Python 3.14.6, using Cargo's existing release settings.
The baseline is HEAD `95bd773bd307c323fd64d154951cd4aa36552dbb`.
The final formatter SHA256 is
`36df78340edd292aebdd729039870a13e6bf6f1ebb0374e7b15a58c5177aaede`;
the baseline SHA256 is
`0986e6f9d9250bb9aff95f3eccb58d51aae1b031f4ba24d2f15a0f60aef0de17`.
`target/benchmarks/base047/pin.json` and `final-pin.json` pin inputs, sources and
binaries. Builds, saves, probes and corpus checks ran sequentially. Unrelated
host processes remained active; no controlled cold filesystem claim is made.

The unchanged `bench-save.py` alternates default and nested EditorConfig
settings, both resolving to four spaces, tab width four, LF and width 120.
Every case/settings entry has 50 fresh processes and includes startup,
configuration lookup, stdin transfer, parsing, formatting and full stdout capture.
`before.json`, first-candidate `after.json`, `final.json` and one bounded
`final-repeat.json` retain all samples. The tables compare before → final repeat;
no samples are pooled or removed. Peak RSS comes from separate matched file-I/O
saves in `before-rss.json` and `final-rss.json`: Darwin `os.wait4` reports each
child's `ru_maxrss` in bytes, divided by 1,048,576 for MiB. The driver's native
`time -l` attempts remain unavailable because the sandbox denies
`sysctl kern.clockrate`; their error output remains in the raw reports.

Each entry below shows before → final repeat; latency and spread are ms,
throughput is input MiB/s, and RSS is native MiB. D means defaults and N means
nested EditorConfig. Output sizes are identical in every compared save.

| Case | Settings | Input / output bytes | p50 | p95 | min–max | RSS | MiB/s |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: |
| ordinary-changed | D | 1,163 / 1,324 | 5.20 → 5.27 | 5.60 → 6.95 | 4.56–5.77 → 4.61–9.03 | 1.97 → 1.97 | 0.22 → 0.20 |
| ordinary-changed | N | 1,163 / 1,324 | 5.20 → 5.30 | 5.74 → 7.43 | 4.61–6.30 → 4.72–11.67 | 2.02 → 2.08 | 0.21 → 0.20 |
| ordinary-formatted | D | 1,324 / 1,324 | 5.29 → 5.09 | 5.94 → 7.00 | 4.75–6.25 → 4.46–9.21 | 2.00 → 2.02 | 0.24 → 0.24 |
| ordinary-formatted | N | 1,324 / 1,324 | 5.27 → 5.21 | 6.05 → 7.62 | 4.85–6.38 → 4.49–9.42 | 2.08 → 2.11 | 0.24 → 0.24 |
| invalid-edited | D | 33 / 0 | 4.75 → 4.76 | 5.42 → 5.12 | 4.07–5.63 → 4.10–5.98 | 1.75 → 1.77 | 0.01 → 0.01 |
| invalid-edited | N | 33 / 0 | 4.72 → 4.82 | 5.26 → 5.20 | 4.16–6.82 → 4.10–5.29 | 1.80 → 1.81 | 0.01 → 0.01 |
| matrix | D | 676 / 1,076 | 5.11 → 5.24 | 5.55 → 6.13 | 4.56–6.24 → 4.32–8.27 | 2.11 → 2.09 | 0.13 → 0.12 |
| matrix | N | 676 / 1,076 | 5.10 → 5.25 | 5.48 → 5.79 | 4.60–5.76 → 4.41–5.81 | 2.16 → 2.12 | 0.13 → 0.12 |
| nested | D | 1,133 / 24,168 | 5.68 → 5.65 | 6.31 → 7.16 | 5.09–6.45 → 4.92–8.02 | 2.19 → 2.23 | 0.19 → 0.19 |
| nested | N | 1,133 / 24,168 | 5.77 → 5.84 | 6.16 → 6.66 | 5.27–6.68 → 5.01–7.15 | 2.23 → 2.30 | 0.19 → 0.19 |
| matrix-grown | D | 6,971 / 10,596 | 7.88 → 7.34 | 11.10 → 8.14 | 7.51–15.10 → 6.71–8.41 | 3.53 → 3.16 | 0.79 → 0.90 |
| matrix-grown | N | 6,971 / 10,596 | 8.06 → 7.42 | 10.24 → 8.06 | 7.55–10.56 → 6.89–8.40 | 3.48 → 3.19 | 0.80 → 0.90 |
| nested-grown | D | 4,493 / 694,968 | 27.87 → 27.36 | 29.18 → 28.03 | 27.25–31.69 → 26.19–41.92 | 4.36 → 4.56 | 0.15 → 0.16 |
| nested-grown | N | 4,493 / 694,968 | 28.14 → 27.41 | 29.06 → 29.23 | 27.58–30.94 → 26.18–43.40 | 4.47 → 4.52 | 0.15 → 0.15 |
| dense-10000 | D | 9,669 / 26,338 | 6.42 → 5.63 | 7.88 → 6.24 | 5.54–8.08 → 4.94–6.51 | 3.34 → 2.89 | 1.42 → 1.64 |
| dense-10000 | N | 9,669 / 26,338 | 6.41 → 5.68 | 8.43 → 6.11 | 5.71–10.79 → 5.08–6.23 | 3.38 → 2.92 | 1.37 → 1.63 |
| dense-100000 | D | 96,669 / 263,338 | 17.20 → 15.32 | 20.73 → 16.86 | 16.34–22.33 → 14.59–17.23 | 18.62 → 12.22 | 5.27 → 5.97 |
| dense-100000 | N | 96,669 / 263,338 | 17.22 → 15.45 | 19.84 → 16.44 | 16.36–21.62 → 14.61–16.93 | 17.31 → 12.25 | 5.25 → 5.95 |
| dense-1000000 | D | 966,669 / 2,633,338 | 128.59 → 116.39 | 143.19 → 119.10 | 122.32–156.18 → 109.66–120.89 | 142.77 → 91.31 | 7.06 → 7.96 |
| dense-1000000 | N | 966,669 / 2,633,338 | 129.54 → 116.78 | 142.53 → 119.85 | 123.16–168.47 → 107.97–120.50 | 142.78 → 91.58 | 7.01 → 7.97 |
| dense-2000000 | D | 1,933,338 / 5,266,672 | 261.49 → 225.22 | 278.46 → 232.77 | 246.71–283.43 → 205.00–235.41 | 279.97 → 177.50 | 7.03 → 8.23 |
| dense-2000000 | N | 1,933,338 / 5,266,672 | 259.99 → 225.18 | 276.33 → 230.89 | 247.04–301.40 → 207.86–238.20 | 280.48 → 176.00 | 7.03 → 8.23 |
| external-0-atsp.mzn | D | 6,899 / 6,305 | 6.09 → 5.01 | 8.18 → 5.46 | 5.25–16.61 → 4.77–6.30 | 2.55 → 2.45 | 1.01 → 1.30 |
| external-0-atsp.mzn | N | 6,899 / 6,305 | 6.27 → 5.05 | 9.15 → 5.57 | 5.50–14.23 → 4.88–5.59 | 2.58 → 2.50 | 0.98 → 1.29 |
| external-1-trip_7_4.mzn | D | 85,229 / 182,774 | 15.45 → 14.34 | 21.97 → 15.98 | 14.00–38.33 → 12.97–17.50 | 10.92 → 7.84 | 5.00 → 5.67 |
| external-1-trip_7_4.mzn | N | 85,229 / 182,774 | 15.32 → 14.30 | 21.68 → 16.03 | 13.92–29.76 → 13.01–16.53 | 10.39 → 7.86 | 4.93 → 5.64 |

Every representative case up to 100 KiB passes 50 ms / 32 MiB in the bounded
repeat. The 966,669-byte dense case fails both 100 ms and 64 MiB with both
settings. The 1,933,338-byte dense case is outside those budgets and remains
supported; it shows approximately linear latency and RSS growth from 1 MiB.
The public table below is a distinct large shape, not a size-matched dense array.

The first full final run had severe scheduling tails: nested-grown p95 was
206.23/226.20 ms, and dense-1000000 p95 was 2,071.32/1,806.56 ms.
Its first-use ordinary save took 1,311.15 ms wall versus 16.99 ms child CPU.
The bounded repeat's first-use took 6.23 ms wall / 3.78 ms child CPU;
first-candidate first-use was 6.72 / 3.53 ms. Baseline first-use took
291.84 ms wall / 4.17 ms child CPU and remains separate in `before.json`. These observations neither establish
controlled cold-start behavior nor attribute the outliers to the source change.
Host load observations around the repeat were 31.79/34.99/38.71 and
22.06/31.62/37.19; the coordinator also observed 42.11/36.97/39.52.
The process-name snapshot showed unrelated CPU activity. The repeat and one
bounded alternating control investigate the noisy run; they do not erase it.

The alternating control has 50 before/final processes per affected case.
Nested-grown gives p50 26.03 → 26.14 and p95 28.05 → 27.61 ms;
dense-1000000 gives p50 126.80 → 109.58 and p95 133.61 → 112.36 ms.
It preserves exact output hashes/statuses and records child CPU separately in
`final-paired-control.json`. The control supports dense improvement and does
not show a nested regression. The small unaffected-case fluctuations remain
within their budgets; no broad speed claim is made for them.

The read-only public
`2023/table-layout/p2000_m16_r1000_c200.dzn` contains 27,775,980 input bytes
and emits 78,976,026 bytes. Five interleaved before/final fresh-save pairs use
identical options and file-backed stdin/stdout. All ten exit 0, retain complete
outputs with SHA256
`0b9603395712dc833f3b4c3c0ea02f55c7ef81022811675838d66b5c2e66b706`,
and match each other and the five initial baseline outputs byte-for-byte.
The original SHA256 remains
`8ebf48c058e194e60154621d7a4eebbb4bb258293910eeca5cad1eff0b462f87`.
The five initial baselines remain in `large-table-layout/before.json`; their
3,402–4,385 MiB RSS variation prompted the matched comparison. These are five
samples per version, so median/range are reported without a p95 estimate.

| Public table | Before | Final |
| --- | ---: | ---: |
| Median wall ms | 4131.09 | 3846.63 |
| Minimum wall ms | 3963.72 | 3800.19 |
| Maximum wall ms | 4594.86 | 3923.46 |
| Median child CPU ms | 4113.00 | 3836.92 |
| Minimum RSS MiB | 3919.62 | 2805.53 |
| Maximum RSS MiB | 3932.36 | 2807.58 |

Allocation counters attribute requested and retained heap bytes; they are not
RSS or fresh-save timing. The dense one-shot phases retain the original source
copy and keep the caller's CST alive during formatting:

| Phase | Allocation calls before → final | Requested bytes before → final | Retained bytes before → final | Additional peak bytes before → final |
| --- | ---: | ---: | ---: | ---: |
| lex | 20 → 20 | 51,298,221 → 51,298,221 | 26,132,493 → 26,132,493 | 26,132,493 → 26,132,493 |
| parse, including lex/source | 333,374 → 333,377 | 215,961,645 → 175,184,581 | 140,464,461 → 66,132,733 | 140,464,461 → 81,409,013 |
| format, CST retained | 62 → 62 | 65,547,874 → 65,547,874 | 2,633,338 → 2,633,338 | 31,993,466 → 29,360,128 |

The first slice candidate doubled dense parse calls to 666,709 because a
single-token atom first grew to four elements, then shrank. Capacity-one atom
buffers remove that redundant allocation. Their final retained storage remains
unchanged. The parse construction peak still exceeds the retained result by
15,276,280 bytes; final-layout re-lexing still contributes a substantial temporary
peak. Those measured seams identify bounded next work for the remaining budget.

A separate token-trimming experiment accounts for 666,671 tokens in 1,048,576
allocated slots, or 9,165,720 bytes of slack. Trimming lowers retained parse bytes
to 56,967,013 but increases final-format requested allocation to 89,548,066 bytes.
A 50-pair control on ordinary, nested and dense cases preserves exact outputs.
Dense 1 MiB RSS is 90.000 → 89.969 MiB, p50 109.98 → 110.35 ms, and p95
113.85 → 127.69 ms. The final candidate keeps token capacity: this experiment
adds copying without a useful peak-RSS reduction. Its sequential noisy samples,
paired samples and probes remain in `trim.json`, `trim-paired.json`,
`trim-dense-all.txt` and `untrim-dense-all.txt`.

The probe's three independent parse/format/lint scopes return live allocation
exactly to 968,495 bytes for dense input and 2,992 for ordinary input, before
and after. Formatting leaves the retained parse result usable for source access
and linting. Native CLI checks use separate canonical files, with expected exit 1
because formatting would change them. Their high-water RSS is:

| Independent dense files | Before MiB | Final MiB |
| ---: | ---: | ---: |
| 1 | 141.14 | 89.95 |
| 3 | 238.00 | 217.92 |
| 10 | 717.11 | 665.91 |
| 20 | 1407.09 | 729.61 |

The allocation scopes demonstrate that per-file application allocations are
released. Native RSS grows for the early files; final growth slows sharply
between ten and twenty. That excess is consistent with allocator retention,
but these measurements do not identify allocator internals or prove a bound for
every larger batch. `ru_maxrss` is a high-water metric and cannot show RSS
falling after a drop. `final-independent.json`, `independent-file-growth.json`
and the drop logs retain distinct evidence. The earlier repeated-path control
was deduplicated by input discovery and proves nothing about multiple files;
it remains labeled separately in `final-deduplicated-path-control.json`.

`save-reconciliation.json` checks all 1,300 timed samples per version and the
26 separate output/RSS probes against the baseline. Outputs and expected
statuses are identical; invalid edited input exits 2 with no stdout and the
same diagnostic after substituting only the temporary stdin filepath.

The representative shared-include replay retains 18 exact before/final checker
reports: default syntax/format/lint, thesis and all over two roots sharing one
file and a root with a missing dependency. It preserves findings, per-rule
outcomes, errors, limitations and file counts. Native default/thesis/all CLI
runs process the two roots independently. Default lint remains syntax-only;
semantic runs retain 11 combined limitations in this deliberately small core
fixture rather than claiming complete analysis. The missing dependency retains
its error and Limited rule outcomes. `shared-includes/replay.json` and its CLI
stderr files retain these results; no full semantic corpus campaign was run.

The fresh pinned syntax/format runner checks all 6,417 read-only corpus inputs
without `--rules` in 391.04 seconds. It completes 6,349 checks, preserving exact
token/leaf coverage, spelling, structure and protected bytes, clean formatted
reparse and second-pass stability. Its explicit remaining gaps are 23 timeouts,
36 syntax assessments, four data files containing model items and five invalid
UTF-8 files. No completed preservation failure or checker crash appears.
All 6,417 paths, sizes and input hashes match base046-final, and all originals
are rehashed unchanged after the scan. All 6,347 jointly completed reports match
on behavior. Only two cause rows change: the proteindesign
`2DHC.14p.19aa.usingEref_self.dzn` and public table complete after old timeouts.
The runner's exit 1 records these known gaps; it is not full language or semantic
coverage. A separate public-table checker completes in 9.626 seconds with a
finite 30-second deadline and passes all preservation/reparse/idempotence checks.
`target/corpus/base047-final/` retains inventory, every result, summary,
reconciliation and supplemental checker evidence.

`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` pass. Existing checks cover lexical/tree coverage,
recovery, protected comments/literals, BOM/EOL handling, matrices, idempotence,
file-local identity/suppression and independent roots. No tests were added.
`target/benchmarks/base047/validation.json` names the commands, statuses and logs.

The remaining concrete save blocker is dense 966,669-byte input at roughly
112–120 ms p95 and 91 MiB RSS. A bounded follow-up must use allocation evidence
on large collection child construction and the rendered token buffer, retain
only a demonstrated local improvement, and recheck 100 ms / 64 MiB along with
the smaller cases and public large shape. It must preserve the current parser,
owned source, borrowed traversal and protected bytes. The base-048 acceptance
gate must remain blocked while these budgets miss. First-use and busy-host
observations also remain explicit for that gate; no budgets were relaxed.

Repeatable commands (VERSION is before or final):

```sh
cargo build --release -p zincite-fmt --bin zincite-fmt \
  --example profile-phases --example check-corpus-file \
  -p zincite-lint --bin zincite-lint
python3 scripts/bench-save.py \
  --binary target/benchmarks/base047/VERSION/zincite-fmt --samples 50 \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/ATSP/atsp.mzn \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/java-routing/trip_7_4.mzn \
  --output target/benchmarks/base047/NEW-REPORT.json
python3 target/corpus/base047-final/pinned/check-corpus.py \
  --challenge /private/tmp/portfolio-150-mzn-challenge-a844 \
  --local /Users/zayenz/minizinc \
  --supplement /private/tmp/zincite-base023-mznc2026-probs \
  --binary target/corpus/base047-final/pinned/check-corpus-file \
  --timeout 10 --output target/corpus/NEW-REPORT
```

The ignored `target/benchmarks/base047/helpers/` copies the small supplementary
measurement helpers and their commands; `evidence-pin.json` identifies them.
The six pre-existing foreign files retain exact bytes, modes and unstaged status.
Only four code/probe paths and this report belong to the implementation;
`ownership.json` records the final checks. Task records, lifecycle, index and
commit remain with the coordinator.
