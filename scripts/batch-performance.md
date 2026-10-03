# Batch performance checkpoint

The [base-078 source-location repair below](#source-location-repair--base-078) records fresh current measurements and coverage. Base-074 and base-049 remain historical evidence; their finite cutoffs and limitations are preserved.

The base-049 sequential default-lint command visited all 6,417 discovered roots in 146.1–162.3 seconds. Formatter checks and semantic lint commands retain their finite deadline results below; incomplete commands have no completed-root count or full-batch throughput. The targeted controls identify repeated source-prefix scans in lint and repeated matrix row classification in formatting. Their follow-ups remain separate from the existing dense-save work.

Base-049 adds an explicit `batch` branch to `scripts/bench-save.py` and a matching developer observation branch to `profile-phases`. Production parser, formatting and lint APIs and commands are unchanged. The existing save branch remains available. This is a measurement checkpoint, with current errors, limitations and Unobserved outcomes retained. It does not establish full semantic or performance acceptance.

Measurements ran on 2026-10-01 at HEAD `86995b2f2a7a9e98761e438561d8ca14ac821c50`, macOS 26.6.2 arm64, Apple M1 Max, 64 GiB RAM and 10 logical CPUs, using the workspace release settings, Rust 1.98.1 and Python 3.14.6. `target/benchmarks/base049/` holds commands, binary hashes, original hashes, complete streams, native usage, allocation rows and reconciliation. Cargo logs identify the exact build and checks. Host load remained uncontrolled; the team ran builds, checks, timings and profiles sequentially. First uses and scheduling outliers remain in the raw rows.

## Commands and accounting

Run from the repository root after building the release bins and example:

```sh
cargo build --release -p zincite-fmt -p zincite-lint --example profile-phases --bins
python3 scripts/bench-save.py batch \
  /private/tmp/portfolio-150-mzn-challenge-a844 \
  /Users/zayenz/minizinc \
  /private/tmp/zincite-base023-mznc2026-probs \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  --repeat 2 --timeout 300 --probe-timeout 300 \
  --output target/benchmarks/base049/NEW-DIRECTORY
```

The driver invokes native `zincite-fmt --check` and `zincite-lint --rules default|thesis|all`, with the same semantic stdlib/include options as the companion probe. Processes are fresh; filesystem caches are ordinarily warm. Trial 0 is first use within that case, not controlled cold I/O. With two native batch repeats, the report gives both observations and spread, not p95 or statistical confidence. The existing save budgets and 50-sample save protocol are unchanged.

Native stdout/stderr go directly to files. The timer surrounds launch through child completion. Blocking `waitid(WNOWAIT)` observes completion; the watchdog is cancelled and joined while the PID remains unreaped, then one `wait4` obtains exit status, child CPU and that child's RSS. Darwin `ru_maxrss` is bytes, divided by 1,048,576 for MiB. `wait-smoke/` verifies fast completion and a 50 ms deadline. The separate `/usr/bin/time -l` capability attempt fails because the sandbox denies sysctl; its full failure remains there. No unavailable value is treated as zero.

The Rust example uses `discover_inputs`, preserving canonical deduplication and first path spelling. It follows native semantic lint even for rejected `.mzn` input: `load_model`, `analyze_model`, `write_analysis`. Source-local/data paths preserve the native read/decode/BOM/parse behavior. JSON root records retain full per-rule outcomes, reasons and finding counts, source structure, loaded dependencies and phase allocation counters. Each context/result/input leaves scope before the drop snapshot and next root. Report construction and Python dependency unions sit outside the load/analyze/render snapshots. Allocation traffic, live bytes and instrumented phase timings are attribution evidence, separate from native wall/CPU/RSS.

The current manifest has 6,417 canonical roots and 641,084,859 bytes, rather than an assumed historical denominator. One dangling non-source file in the personal tree produces a separate discovery error. It is outside the root denominator and contributes aggregate status 2. The driver records root/stdlib hashes for 7,439 originals; final verification is reported below. Two observed BACP `.mzn.model` dependencies fall outside that source-extension inventory. Their before-campaign hashes were not captured; a later bounded hash/replay is separate evidence and cannot repair that historical omission.

## Full native results

| Selection / trial | Wall, s | Child CPU, s | Peak RSS, MiB | Status | Attempted roots | Attempted MiB/s |
| --- | ---: | ---: | ---: | --- | ---: | ---: |
| format / 0 | 300.072 | 295.624 | 5949.7 | deadline / −9 | unknown | unclaimed |
| format / 1 | 300.059 | 285.058 | 4324.6 | deadline / −9 | unknown | unclaimed |
| default / 0 | 146.083 | 143.838 | 4981.9 | 2 | 6,417 | 4.185 |
| default / 1 | 162.258 | 146.418 | 4540.7 | 2 | 6,417 | 3.768 |
| thesis / 0 | 300.013 | 299.291 | 366.0 | deadline / −9 | unknown | unclaimed |
| thesis / 1 | 300.010 | 295.404 | 345.3 | deadline / −9 | unknown | unclaimed |
| all / 0 | 300.019 | 294.835 | 359.0 | deadline / −9 | unknown | unclaimed |
| all / 1 | 300.014 | 296.168 | 352.1 | deadline / −9 | unknown | unclaimed |

Both formatter checks stop at 300 seconds with 4.22–5.81 GiB peak RSS. Their captured last diagnostic identifies an emitted message, not a processed prefix or the root currently executing: successful clean and changed `--check` files are silent. The bounded attribution below establishes a formatter cost without assigning the whole cutoff to that file or function.

Status 1 means findings for lint or changed output for formatter checks; status 2 means errors, including discovery/input/syntax/dependency failures. A normal native loop can process every discovered root while rejecting some. Throughput over attempted root bytes is not throughput of completed semantic analyses. A killed child has unknown completed-root count and no claimed full-root throughput. Its deadline is a censored lower bound, not its completed latency.

The matching default probe completed all 6,417 root observations and matched both native stderr streams and status exactly. It reported 235,106 warnings, 286 root-level errors and no limitations. Each source-local rule had 6,372 Completed, 40 NotRun and five Unobserved UTF-8 failures. Root states were 3,871 data, 1,123 complete, 1,386 fragments, 31 rejected model syntaxes, five input errors and one multiple-solve root. Data syntax errors remain in rule/error fields rather than being hidden by the data structural category. Dependency resolution is unassessed for default lint.

The largest observed default read/parse allocation belongs to the 64,697,404-byte public `2018/proteindesign12/1CM1.17p.19aa.usingEref_self.dzn`: 2,855,017,016 retained bytes and a 2,917,576,656-byte peak above that phase's baseline, with no parse errors. Its completed root scope releases to baseline. This distinguishes a large current tree from unused model work retained between roots; it does not explain all native RSS high-water behavior or establish a general memory bound.

Thesis's finite companion observed 517 roots: 435 data and 82 complete models, of which 27 had resolved dependencies and zero completed all selected rules. It retained 4,738 warnings, 820 errors and 1,012 limitations. Each of its 14 rule partitions has the same denominator 6,417 and includes 5,900 Unobserved roots. The all-preset companion also observed 517 roots, with the same 435 data/82 complete/27 resolved/zero all-selected-completed counts. It retained 8,270 warnings, 820 errors and 1,012 limitations; each of its 16 rule partitions includes 5,900 Unobserved roots. Both semantic companions reached their 300-second deadline. Their observed closures repeat 25,178 loaded files and 50,783,908 bytes, versus a union of 567 files and 1,403,085 bytes. These counts describe only completed companion root observations, not the full intended closure work. Companion observed counts are its own scope counts; they are not inferred completed counts for timed-out native commands. All reasons, per-rule partitions and unfinished-root entries remain in `full/report.json` and JSON-lines streams.

All 6,417 default, 517 thesis and 517 all-preset drop snapshots return exactly to the discovery/options baseline. The first bounded run showed a constant 64-byte offset after first stderr use. An empty stderr write before the baseline removed that process-wide initialization cost, with native text/status unchanged. Both the original `bounded/` rows and corrected `bounded-final/` rows remain. No root context was warmed or retained to obtain that result.

## Bounded growth and current coverage

`bounded-final/` exercises shared/missing includes, standalone data, rejected syntax, BOM, UTF-8 failure and continued later roots. For default/thesis/all, both native repeats and the companion have byte-identical full stderr and identical status 2. Default reports three warnings/two errors; thesis reports one warning/four errors/25 limitations; all reports four warnings/four errors/25 limitations. Three semantic roots are complete/resolved, but none completes every selected rule. `dedup/` records three arguments, including a repeated path and symlink, as one canonical root.

The independent family uses equal-content 966,669-byte data files at distinct canonical paths. Times are seconds and native RSS is MiB; both fresh observations remain in each report.

| Distinct roots | Root bytes | Formatter wall | Formatter RSS | All lint wall | All lint RSS |
| ---: | ---: | --- | --- | --- | --- |
| 1 | 966,669 | 0.099 / 0.099 | 91.3 / 91.5 | 0.049 / 0.047 | 61.4 / 61.5 |
| 3 | 2,900,007 | 0.295 / 0.280 | 221.3 / 217.8 | 0.125 / 0.126 | 143.0 / 143.0 |
| 10 | 9,666,690 | 0.982 / 0.960 | 667.2 / 665.8 | 0.408 / 0.412 | 431.8 / 429.6 |
| 20 | 19,333,380 | 1.972 / 1.909 | 731.4 / 731.4 | 0.824 / 0.835 | 635.6 / 635.6 |

The native wall work grows roughly with distinct root count. Every companion input/lint scope returns to baseline; the per-root retained parse maximum stays 67,099,402 bytes and peak parse delta 82,375,682 bytes. Semantic rules on these data files are Inapplicable. RSS still rises across released scopes and grows more slowly from 10 to 20 roots. It measures process high-water/allocator behavior and does not show that every prior tree remains live. These bounded counts do not establish an allocator plateau for arbitrary workloads or certify the censored formatter's lifetime behavior.

Shared roots each include the same common file twice. The native all-preset command takes about 1.15 / 3.45 / 11.5 / 23.05 seconds for 1/3/10/20 roots, with eight limitations and one warning per root. Repeated loaded-file counts are 24/72/240/480; union counts are 24/26/33/43. Repeated loaded bytes are 359,838 / 1,079,514 / 3,598,380 / 7,196,760, while union bytes are 359,838 / 359,956 / 360,369 / 360,959. Each closure deduplicates canonical files internally, then a new root loads its closure again. Selected consumers share conditional facts within the current root; there is no cross-root cache. Peak native RSS stays about 26–34 MiB and every scope drops to baseline.

The diamond family adds 1/3/10/20 unique dependencies inside one root. Loaded counts become 25/27/34/44 while the common include stays deduplicated. Native all-preset times remain 1.15–1.27 seconds and RSS about 25–28 MiB. Core analysis dominates these small added files; the controls demonstrate no material superlinear growth from this dependency dimension.

| Unlabelled constraints / warnings | Input bytes | Native default wall, seconds | Native child CPU, seconds | Native RSS, MiB |
| ---: | ---: | --- | --- | --- |
| 100 | 1,715 | 0.008 / 0.008 | 0.006 / 0.006 | 2.03 / 2.02 |
| 1,000 | 17,015 | 0.037 / 0.038 | 0.035 / 0.035 | 2.97 / 2.92 |
| 10,000 | 170,015 | 0.999 / 0.997 | 0.988 / 0.989 | 9.80 / 9.81 |
| 25,000 | 425,015 | 5.236 / 5.308 | 5.212 / 5.224 | 21.22 / 21.19 |

All diagnostic counts are exact, with status 1, zero errors/limitations and full capture. At 25,000, the separate probe spends 5.15 ms reading/parsing, 4,631.97 ms analysing and 580.25 ms rendering. Analysis requests 11,744,991 bytes, retains a 6,275,080-byte result and peaks 7,847,872 bytes above its pre-analysis baseline. Allocations and output grow with warnings; analysis CPU grows faster.

Equal effective EditorConfig settings on 100 tiny roots/1,500 bytes give 13–15 ms and about 2.3 MiB RSS for both lookup shapes, with five fresh observations each. No material configuration cost was demonstrated at this scale. The initial EditorConfig row accidentally included one generated alias (101 roots); it remains as a confounded row. `configuration-editorconfig-matched/` repeats the intended 100-root comparison after only that generated alias was removed.

The public `2010/wwtp_random/wwtpp.mzn` control has 13,082 bytes and current default/thesis/all warning counts 200/121/321. Native all takes 1.427–1.428 seconds at 32.4–32.7 MiB, with zero errors and 28 limitations. All full native stderr/status pairs match their probes. This complete/resolved model still does not complete every selected semantic rule.

## Attribution and required follow-ups

The diagnostic profile uses a separate 50,000-warning extension of the same family. Its selected two-second interval has 1,445 top-of-stack samples, all in `SourceLocation::new` under source-local analysis. A separate 20-shared-root semantic interval has 991 of 1,439 top samples there, 250 in `callables::find_node` and 100 in unused-declaration analysis. These are interval counts, not whole-command CPU percentages. The first 25,000-warning attachment missed the already-finished process; that failure remains alongside the successful longer profile. Profiled command wall time includes sampling interference and is excluded from comparison tables.

`SourceLocation::new` walks `source[..range.start]` for each requested position. The targeted growth, CPU and native stacks establish repeated prefix scans as a concrete cost in both diagnostic and semantic-location work. Full default allocation attribution totals about 19.11 seconds reading/parsing, 116.31 analysing and 5.73 rendering; it does not assign all that analysis time to the sampled function. A bounded follow-up must avoid repeated prefix scans while preserving byte ranges, BOM offsets, CRLF handling, Unicode columns and exact diagnostics/rule outcomes. Its matched targets are at most 2.5 seconds for 25,000 warnings, 0.6 seconds for 10,000, and 12 seconds for the 20-root semantic control, with no material allocation/RSS regression. Other sampled functions are observations, not separate optimization proposals.

The formatter family fixes three matrix rows and increases columns, using `values = [|` followed by three comma-separated rows of zeroes and `|];`. These are distinct generated inputs, not repeated paths. Native `--check` runs use a 30-second deadline and preserve status 1 (would change), CPU and complete streams in `formatter-attribution/`.

| Columns in each of three rows | Input bytes | Native wall, seconds | Child CPU, seconds | RSS, MiB | Separate format phase, ms |
| ---: | ---: | --- | --- | --- | ---: |
| 5,000 | 30,014 | 0.275 / 0.270 | 0.273 / 0.268 | 7.03 / 7.06 | 264.7 |
| 15,000 | 90,014 | 2.350 / 2.354 | 2.346 / 2.350 | 18.50 / 18.50 | 2,397.7 |
| 30,000 | 180,014 | 10.799 / 10.818 | 10.785 / 10.808 | 37.19 / 33.80 | 10,612.2 |

At 30,000 columns, the separate probe parses in 7.0 ms and formats in 10.6 seconds. Formatting requests 66,565,829 bytes, retains a 284,233-byte output and peaks 13,107,200 bytes above the retained CST. Requests for 5,000/15,000 columns are 12,207,003 / 33,282,062 bytes. Allocation traffic grows roughly with columns while native CPU grows much faster.

The public `2019/groupsplitter/u7g2pref1.dzn` has 885,733 bytes and takes 9.478 / 9.455 seconds, 9.452 / 9.439 child CPU seconds and 128.8 / 129.0 MiB. The public 619,533-byte `2023/table-layout/p1000_m20_r200_c20.dzn` countercontrol takes 0.087 / 0.086 seconds and 62.5 / 63.4 MiB. The established 27,775,980-byte `p2000_m16_r1000_c200.dzn` takes 4.054 / 3.789 seconds and 3,151.5 / 2,805.5 MiB. The first larger-table RSS observation stays in the report; these two checks do not replace base-047's five matched save pairs or establish p95.

The first public-wide sampling attachment missed its exited target; its failure and 9.918-second native metadata remain. One extended 60,000-column control supplied a successful two-second interval before its 40-second cutoff. All 1,389 sampled stacks are in `matrix_layout`'s width closure/iterator: 950 in its maximum-width fold and 439 in the closure, reached through `prefix_exceeds_width` and `expression_contents`. This is an interval observation; the extended command's interfered/censored wall time is not a comparison sample.

`matrix_layout` requests column widths by scanning child rows for each column, calling `matrix_row_has_index` each time. That helper scans a row's child tokens for a colon. The source loop, native CPU growth and stacks establish repeated row classification inside matrix width work as a concrete cost. A bounded follow-up should compute each row's classification once for that layout scope, preserving indexed rows, nested values, alignment and width decisions. Matched targets are at most 0.5 seconds for the 30,000-column control and one second for the public wide matrix, with the fixed-row growth becoming roughly proportional to columns. These optimization targets do not replace the brief's save budgets. The follow-up must also replay the full native formatter batch under a finite adequate deadline and record any remaining cutoff or high-water problem; this interval does not prove the sole cause of the original 300-second result.

The current public wide and synthetic 30,000-column inputs pass the existing finite fidelity checker: token/tree coverage, zero parse/reparse errors, spelling, structure, protected bytes and idempotence. Its 25.80 / 29.78-second elapsed times include these extra checks and stay separate from native timing. Three independent public-wide parse/format/lint scopes return exactly to their 887,587-byte input baseline; the borrowed parsed file remains usable after formatting. `formatter-fidelity/` retains the 90-second deadlines, complete streams and unchanged input hashes. Released allocations in these controls do not certify the incomplete full formatter's peak-memory behavior.

The matrix cost is distinct from base-077's measured child-buffer and rendered-token construction work. The existing base-075 include-loading, base-076 callable/semantic and base-077 dense-save blockers remain. The coordinator must keep base-050 blocked by the confirmed performance follow-ups as well. No numeric batch budget is invented, no save budget is relaxed, and no hard root is excluded to obtain a completed result.

## Validation and evidence

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass after the final probe change; logs are `fmt-final.log`, `clippy-final.log` and `tests-final.log`. No timing, object-layout or reporting-only tests were added. CLI smokes reject non-finite deadlines, existing output and output inside input roots. The unchanged save branch passes ordinary and invalid-buffer controls. One initial save smoke briefly overlapped tests; it remains excluded from timing claims, with a sequential replay saved separately.

`bounded-reconciliation.json` compares every completed bounded native/probe stderr/status pair, per-rule denominators and original hashes. `growth-summary.json` links all growth cases. `final-reconciliation.json` checks full stream hashes, default native/probe equality, all 6,417 rule denominators, explicit missing observations and deadline rows with null completed counts/throughput. All 7,439 inventoried originals and production/probe binaries are unchanged after the full run. `formatter-attribution/` and `formatter-fidelity/` also rehash their original inputs unchanged. The final ownership record checks the six foreign files against their original bytes/modes, an empty index and the three attributable paths. The subsequent two-parent BACP replay matches both native stderr/status repeats to thesis/all probes, retaining errors and Limited outcomes. The two `.mzn.model` dependency hashes match before and after that replay in `bacp-source-hashes.json`. Their absent pre-full-campaign hashes remain an explicit limitation, and the replay does not alter the 6,417-root full denominator.

Historical base-045 semantic reports and base-047 fidelity/save evidence remain historical. In particular, the old per-file checker skipped model loading on rejected syntax and cannot certify this current native semantic route.

The measurement labels follow the distinction between project timing and profiling in the [Ruff guide](https://github.com/astral-sh/ruff/blob/main/CONTRIBUTING.md), explicit cache conditions in [uv's benchmarks](https://github.com/astral-sh/uv/blob/main/BENCHMARKS.md), and first diagnostics versus rechecks in [ty's benchmark guide](https://github.com/astral-sh/ruff/blob/main/scripts/ty_benchmark/README.md). Their infrastructure is unnecessary for this checkpoint.

## Expanded lint comparison: base-074

This checkpoint compares explicit selections at HEAD
`1cc9aa7b8669e619840037cedc4260b7c844f5d4` against the retained base-049 native
linter at `86995b2f2a7a9e98761e438561d8ca14ac821c50`. The historical `all`
preset contained 16 IDs; current `all` contains 26. The comparison therefore
uses the same ordered explicit historical16 list in both revisions, then
measures new10 and joint26 separately. `target/base074/baseline-performance-pin.json`
retains all three ordered lists. Current catalogue resolution produced manifests;
the actual timed native binary and its selection are recorded independently.
Historical raw reports, four retained binaries and 312 controls were preserved.

The two Python drivers and two Rust developer examples accept existing selectors.
The batch driver's `--native-only` option omits companion coverage explicitly;
omitted coverage is not zero or completed. Its default behavior still collects
the companion. Short `sN` stream filenames support long explicit lists. Timing
uses the existing fresh-process launch-to-exit timer, child CPU and Darwin
`os.wait4` RSS in bytes. Full stdout/stderr, exit/deadline and first-use rows are
retained. Current release settings, hardware and cache labels are in every
report. Measurements, builds, tests and profiles ran sequentially. Host scheduling
and filesystem caches remained uncontrolled.

### Matched native controls

`target/base074/matched/` contains 12 reports and 24 native trials. All four
selections use identical bytes, positional paths, include/stdlib options and
per-case original hash maps. Two trials provide observations and spread, not p95
or statistical confidence. The bounded control contains seven roots including
data, rejected syntax, missing dependencies and invalid UTF-8; status 2 is its
expected retained input-error result. The shared control has 20 distinct roots
loading the same dependencies. The diagnostic control has 10,000 warnings in
170,015 source bytes.

| Selection / control | Wall seconds, trials 0 / 1 | Child CPU seconds, trials 0 / 1 | RSS MiB, trials 0 / 1 | Native statuses | Completed native roots per trial |
| --- | ---: | ---: | ---: | --- | --- |
| baseline16 / bounded-final | 7.21 / 8.10 | 6.95 / 7.17 | 30.83 / 30.23 | 2 / 2 | 7 / 7 |
| baseline16 / shared-roots-20 | 27.41 / 26.46 | 23.84 / 23.81 | 34.27 / 34.72 | 1 / 1 | 20 / 20 |
| baseline16 / diagnostics-10000 | 7.22 / 8.85 | 6.61 / 6.57 | 41.80 / 41.34 | 1 / 1 | 1 / 1 |
| current16 / bounded-final | 7.19 / 7.80 | 7.04 / 7.09 | 31.59 / 32.22 | 2 / 2 | 7 / 7 |
| current16 / shared-roots-20 | 23.77 / 23.82 | 23.50 / 23.40 | 34.70 / 33.72 | 1 / 1 | 20 / 20 |
| current16 / diagnostics-10000 | 16.34 / 16.91 | 16.14 / 16.25 | 56.73 / 56.22 | 1 / 1 | 1 / 1 |
| current10 / bounded-final | 6.33 / 6.47 | 6.30 / 6.32 | 29.75 / 30.89 | 2 / 2 | 7 / 7 |
| current10 / shared-roots-20 | 20.86 / 21.29 | 20.80 / 21.01 | 31.31 / 31.23 | 0 / 0 | 20 / 20 |
| current10 / diagnostics-10000 | 180.01 / 180.01 | 178.17 / 178.09 | 49.97 / 50.22 | -9 / -9 | unknown / unknown |
| current26 / bounded-final | 7.53 / 6.98 | 7.05 / 6.96 | 34.55 / 32.59 | 2 / 2 | 7 / 7 |
| current26 / shared-roots-20 | 23.52 / 23.44 | 23.31 / 23.29 | 34.77 / 35.61 | 1 / 1 | 20 / 20 |
| current26 / diagnostics-10000 | 180.00 / 180.01 | 177.68 / 178.14 | 56.72 / 55.16 | -9 / -9 | unknown / unknown |

Current16 bounded/shared child CPU stays close to the retained native control;
the longer baseline shared wall times are retained scheduling observations.
On the diagnostic control, current16 CPU rises from 6.61/6.57 to 16.14/16.25
seconds and RSS from 41.80/41.34 to 56.73/56.22 MiB. This is a reproducible
comparable-ID added cost. New10 and joint26 both reach the 180-second diagnostic
deadline. Those killed commands have no completed-root count or full-analysis
throughput. Status 0 on a completed native command also does not establish that
all selected semantic rules completed; separate observations retain limitations.

### Full current native attempts

Both selections use the same three positional directories and explicit installed
stdlib as base-049. Native discovery records 6,417 canonical roots and 641,084,859
bytes. All 7,439 original hashes match the historical full map. The unrelated
dangling non-source artifact still produces a separate discovery error. Neither
full current native selection completed within its 120-second deadline:

| Selection | Wall seconds, trials 0 / 1 | Child CPU seconds, trials 0 / 1 | RSS MiB, trials 0 / 1 | Statuses | Completed roots |
| --- | ---: | ---: | ---: | --- | --- |
| new10 | 120.01 / 120.01 | 114.66 / 112.12 | 163.11 / 174.31 | -9 / -9 | unknown / unknown |
| joint26 | 120.01 / 120.01 | 118.59 / 118.50 | 176.38 / 181.84 | -9 / -9 | unknown / unknown |

These reports collected native timing without companion coverage. Emitted
stderr is a prefix observation, not corpus-wide totals, processed-root counts or
semantic completion. Both reports recheck unchanged originals and native/probe
binaries. The separate per-file campaign and its Unobserved remainder are in
[lint-expansion-checkpoint.md](lint-expansion-checkpoint.md).

### Separate allocation and coverage observations

`target/base074/observations/` records bounded16, bounded10, bounded26, shared26
and diagnostics16 outside timed comparisons. Native and probe full stderr/status
match for each case. All 42 independent root scopes return exactly to their
recorded allocation baseline. This establishes release of retained per-root
work in these controls; native RSS remains a process high-water measurement.
Allocation traffic and phase peaks are neither summed live memory nor RSS.

For bounded26, analysis requests 217,112,958 allocation bytes, versus 217,043,402
for current16; both have a scoped peak delta of 12,973,982 bytes. Shared26 observes
20 complete/resolved roots, 20 warnings and 40 limitations, with zero roots whose
26 selected rules all completed. Diagnostics16 observes one complete/resolved
root, 10,000 warnings and two limitations, again zero all-selected completion.
Bounded10 observes three resolved roots with all ten completed, but the other
bounded roots retain their errors/inapplicability. Raw per-rule partitions and
phase counters remain in each report.

### Added-cost attribution and acceptance gaps

The first sandboxed `sample` attaches failed and remain under `profiles/`.
A permitted retry records two-second intervals under `profiles-retry/`:
current16 has 1,210 of 1,462 top stacks in `SourceLocation::new`, mainly through
integer bounds; current10 has 1,353 of 1,489 through instantiations, optional,
callable and definition facts. A later current10 interval at 30–32 seconds has
1,480 of 1,488 top stacks there through iteration walking. `SourceLocation::new`
scans the source prefix for each location. These intervals establish a dominant
observed cost at the existing base-078 seam. They do not attribute the entire
180-second run or exclude later bottlenecks. Profiled native times are separate
from the matched table.

Base-078 already owns source-location repair, diagnostic controls, shared roots
and full current replay, so this checkpoint proposes no duplicate follow-up.
Base-075 include-loading, base-076 callable semantics, base-077 dense-save,
base-079 matrix-width work and the final acceptance gates also remain necessary.
No optimization or budget relaxation was made here. The current native cutoffs,
limited semantic outcomes and prior save-budget misses prevent full performance
or semantic acceptance. The checkpoint supplies integration/accounting evidence
for the coordinator and subsequent repairs.

## Source-location repair — base-078

This checkpoint uses HEAD `3b4ab1e9a04a0029f5109dd931002da7e7caa589` plus the recorded source-location change. `target/benchmarks/base078/` contains immutable before/after release binaries, input and standard-library hashes, commands, complete streams and child usage. The installed MiniZinc version receipt reports 2.10.1; no compiler or solver analysis ran for this task. Builds, tests, native timings and allocation observations ran sequentially on the same macOS arm64 host. Filesystem caches and host scheduling were uncontrolled. The first default 100-warning process took 0.312 seconds before and 0.333 seconds after, with only 0.007/0.009 seconds of child CPU; these rows are retained, without a controlled-cold claim.

### Source ownership and compatibility

`ParsedFile::line_column` stores a lazy line-boundary vector in the parsed source. `ModelFile::location`, source-local analysis and loader diagnostics with a parsed source use it. Standalone `SourceLocation::new` and ownerless errors keep their existing behavior. A zero-byte query returns `(1,1)` after slice validation and does not initialize the vector; clean source-local preflight therefore avoids a needless full scan. Parsing and formatting do not build line storage. A lookup still counts Unicode scalar columns within the relevant line, so very long single lines can retain repeated within-line work. This change adds no cross-root cache or semantic interpretation.

The private public-API replay checks 510 repeated reverse lookups through externally constructed `ModelFile` values. Empty source, Unicode, CR, LF, CRLF midpoint, EOF and BOM-adjusted ranges agree exactly with standalone location construction. Both APIs retain invalid UTF-8-boundary/out-of-range panics. Existing public loader, lint and CLI checks pass; no tracked timing or layout tests were added.

### Matched native controls

The warning budgets apply to the original two IDs, `naming,missing-constraint-label`. The additional `all` runs select the current 26 IDs and produce twice as many warnings on these controls. The shared-root target uses current `all`. Each row below contains both fresh native repeats, in seconds. All completed rows exit 1; four before expanded-rule rows retain their 300-second deadline, status -9 and unknown completed-root count.

| Control | Before wall | After wall | Before child CPU | After child CPU |
| --- | --- | --- | --- | --- |
| Default, 100 warnings | 0.312 / 0.007 | 0.333 / 0.008 | 0.007 / 0.006 | 0.009 / 0.006 |
| Default, 1,000 warnings | 0.037 / 0.036 | 0.030 / 0.029 | 0.035 / 0.035 | 0.028 / 0.027 |
| Default, 10,000 warnings | 0.986 / 0.986 | 0.263 / 0.269 | 0.981 / 0.982 | 0.258 / 0.263 |
| Default, 25,000 warnings | 5.205 / 5.226 | 0.607 / 0.609 | 5.194 / 5.192 | 0.603 / 0.604 |
| All, 100 control declarations | 1.185 / 1.189 | 0.385 / 0.383 | 1.182 / 1.184 | 0.377 / 0.375 |
| All, 1,000 control declarations | 6.386 / 6.516 | 0.479 / 0.495 | 6.373 / 6.423 | 0.476 / 0.487 |
| All, 10,000 control declarations | timeout / timeout | 5.539 / 5.524 | 299.523 / 299.621 (censored) | 5.483 / 5.465 |
| All, 25,000 control declarations | timeout / timeout | 30.132 / 31.226 | 299.670 / 299.546 (censored) | 30.023 / 29.839 |
| All, 1 shared root | 1.183 / 1.180 | 0.363 / 0.362 | 1.178 / 1.176 | 0.360 / 0.359 |
| All, 3 shared roots | 3.527 / 3.538 | 1.109 / 1.677 | 3.521 / 3.531 | 1.084 / 1.113 |
| All, 10 shared roots | 11.738 / 11.769 | 3.757 / 3.691 | 11.722 / 11.740 | 3.581 / 3.582 |
| All, 20 shared roots | 23.495 / 23.294 | 7.149 / 7.224 | 23.466 / 23.272 | 7.129 / 7.163 |

Both repeats meet the 0.6-second target for 10,000 default warnings, 2.5 seconds for 25,000 and 12 seconds for 20 shared roots. Default growth from 10,000 to 25,000 is now about 2.3 times, versus 5.3 times before. Expanded `all` still grows substantially faster and retains other analysis costs; these results do not establish linear cost for every rule.

Native RSS is the exact child's Darwin `os.wait4` high-water value in bytes, converted to MiB. Default 10,000 changes from 14.750/14.750 to 14.859/14.828 MiB; default 25,000 changes from 29.344/29.313 to 29.391/29.406 MiB. Shared 20 changes from 33.109/31.953 to 33.250/36.391 MiB. Expanded 25,000 finishes at 121.844/124.109 MiB; its censored before peaks at 99.719/100.656 MiB cover less work and do not provide a matched completed-memory comparison. Raw small-case wall tails and all CPU/RSS rows remain in `before-matched/` and `after-matched/`.

`reconciliation.json` checks all 24 native pairs. Every complete before control has exact stdout, stderr and status after the change. The four censored before streams remain prefix observations. The bounded seven-root replay adds shared includes, data, rejected syntax, BOM, invalid UTF-8 and missing dependencies under default/thesis/all (2/14/26 IDs). Its 21 structured before/after root pairs have identical source/dependency data and complete rule outcomes, findings, errors and limitations. All six native before/after pairs match, and every native stderr/status matches its separate companion probe. These are matched controls, not full semantic acceptance.

### Separate allocation and drop observations

The unchanged `profile-phases batch` probe measures allocation traffic, live deltas and phase peaks separately from native timing. Default 10,000 analysis requests 12,240,887 bytes before and 12,503,007 after; retained analysis grows from 3,870,080 to 4,001,152 bytes. Default 25,000 requests 27,457,343 to 27,981,607 bytes and retains 9,675,080 to 9,937,224 bytes. These increases include the lazy line vectors (131,072 and 262,144 retained bytes), which drop with the owning source.

The shared-20 probe adds 6,304 bytes to maximum load retained/peak deltas and 87,584 bytes to maximum analysis retained/peak deltas (analysis peak 12,951,883 to 13,039,467 bytes). Total analysis traffic changes from 725,818,560 to 729,312,320 bytes. This bounds the local instrumented memory increase; it does not explain the separate native RSS dispersion.

All reported per-root scopes return exactly to their recorded live baseline, including parsed sources, model contexts, analyses and the new vectors. The 20-root probe retains 20 complete/resolved roots, 20 warnings, 40 limitations and zero roots with all 26 selected rules Completed. Expanded 25,000 observes 50,000 warnings and two limitations; analysis requests 27,445,446,466 bytes over 313,941,728 allocations, with a 72,303,320-byte scoped peak. Requested bytes are allocation traffic, neither retained memory nor RSS. Source-local default roots intentionally have no dependency assessment. Full rule partitions, phase counters and drop snapshots remain in `before-observations/` and `after-observations/`.

### Full current replay and remaining gates

The current native and separate structured commands use the original three positional roots, installed standard library and 600-second deadlines for each process. Discovery finds 6,417 canonical roots (641,084,859 bytes). The unrelated dangling `partridge-packing/.#solve-log.txt` remains a discovery error. No root was excluded to obtain a completed subset.

| Selection | Native wall / child CPU (s) | Native RSS (MiB) | Status and native completed roots | Structured observed / missing roots |
| --- | --- | --- | --- | --- |
| Default, 2 IDs | 90.256 / 41.374 | 7,543.250 | 2; 6,417 attempted through completion | 6,417 / 0 |
| Thesis, 14 IDs | 600.120 / 473.241 | 5,196.250 | -9, deadline; unknown | 518 / 5,899 |
| All, 26 IDs | 600.237 / 430.759 | 2,943.406 | -9, deadline; unknown | 516 / 5,901 |

Default attempted-input throughput is 6.774 MiB/s. Its 7,543.25 MiB native high-water mark (about 7.37 GiB) remains a substantial full-batch cost. These rows have no fresh matched full-before run, so they do not attribute a full-corpus speed or RSS change to this repair. Completed source-local default analysis also does not establish resolved semantic coverage.

The default probe reports 235,909 warnings and 202 errors. Both default rule partitions contain 6,396 Completed, 16 NotRun and five Unobserved invalid-UTF8 roots. All root records are present; the five per-rule Unobserved results remain explicit. Thesis observes 13,108 warnings, zero errors in its observed prefix and 3,756 limitations; all observes 3,481 warnings, zero prefix errors and 5,256 limitations. Their missing root records become Unobserved in every selected rule partition, each with the full 6,417 denominator. Zero observed prefix errors does not establish an error-free corpus. Thesis has 83 complete/resolved roots and all has 81; neither has a root with every selected rule Completed. The native completed counts remain unknown for both killed processes.

Every reported root drop returns to its exact live baseline. This does not cover the active scope killed at a deadline or explain OS high-water memory. `full-audit.json` reconciles all records, partitions and drops. The full pre-campaign root/stdlib map and the two supplemental `.mzn.model` inputs rehash unchanged: 7,441 paths, with no drift. Default native/probe complete stderr and status match exactly; semantic deadline streams remain separate prefix observations.

The independently established base-075 standard-include repair and base-076 default-overload repair remain documented in `semantic-corpus-checkpoint.md`. This location change adds no loading or callable support. The bounded shared replay still retains installed optional-operator limitations and search-coverage advice. Current full syntax, dependency, semantic and performance gaps continue to block final acceptance; these three local performance targets do not complete the corpus gate.

### Fresh complete-closure replay

The initial bounded hash map covered 1,053 roots and standard-library files but omitted two imported files: `base047/shared-includes/shared.mzn` and `base049/controls/shared-roots/common.mzn`. Their matching diagnostics are not a substitute for pre-campaign hashes. A separate fresh replay captures the complete 1,055-path closure before and after, leaving all original measurements intact. Every hash remains unchanged.

This replay repeats shared 1/3/10/20 and the seven-root default/thesis/all controls with the immutable before/after executables. All 14 native pairs have exact complete stdout, stderr and status; all 55 structured root pairs have identical sources, dependency identities and full rule results, with exact reported scope drops. Fresh shared-20 wall times are 23.277/23.321 seconds before and 7.130/7.091 after, again below 12 seconds. Their RSS values are 30.859/32.141 MiB before and 35.563/30.391 after; the dispersion remains visible. `closure-replay-index.json`, `closure-audit.json` and `closure-pins-{before,after}.json` retain the commands, outcomes and hash timing.

### Format-on-save recheck and validation

The existing save driver records 50 fresh `--stdin-filepath` samples for ordinary-changed and the 966,669-byte dense array under default and nested EditorConfig settings. All 200 before/after sample pairs preserve output hash, byte count, status and stderr. Dense p95 changes from 90.303/90.791 to 90.577/90.791 ms; after median child CPU is 79.655/79.806 ms. Five interleaved native before/after dense saves per setting retain exact full output bytes; every after RSS value lies between 63.9375 and 63.984375 MiB. Both 100 ms/64 MiB budgets remain met, with the same narrow memory margin. Ordinary medians change from 4.244/4.321 to 4.500/4.533 ms (CPU 2.715/2.755 to 2.856/2.937 ms); all raw tails remain visible. The new lazy vector is not initialized by formatting. The save driver's denied `time -l` capability rows remain retained; separate per-child `wait4` captures supply native RSS.

`commands.json` records exact commands, merged logs, exit status and reaped receipts. `before-binaries.json` and `after-binaries.json` pin formatter, linter, probe and library copies; `controls.json`, `environment.json`, `original-inputs.json` and the closure/full maps pin their inputs and environment. Native measurements use the existing `scripts/bench-save.py batch` driver, while `profile-phases batch` runs separately for attribution. `location-check.rs` exercises the public location APIs; `reconciliation.json`, `closure-audit.json`, `full-audit.json` and `save-audit.json` retain the comparisons. The original duplicate-`--lib` build rejection, setup notes, capability failures and all deadline rows remain identifiable. No solver, corpus acceptance replacement, semantic optimization or new tracked test was added.

Final `cargo fmt --all -- --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings` and `cargo test --offline --workspace` pass. Read-only `zdev check base --format json` and `git diff --check` also pass. The final ownership manifest retains the unchanged HEAD/main/empty index and six foreign files; all owned command handles are terminal and reaped.


## Matrix width repair — base-079

Matrix layout now classifies each row once in its own layout scope. The local
`(row, indexed)` vector supplies the existing index-width calculation, cell
rendering and per-column width closure. The closure no longer scans each row's
children for a colon for every column. Row order, classification, rendered cells,
width decisions, shared breaks and output bytes stay unchanged. The vector drops
when that layout calculation returns; no public API, parser, CST, layout engine,
cache, probe or driver changes.

The baseline is HEAD `918f327216a1a122f54e8e66effcc74361a6d4bd` on
2026-10-02, macOS 26.6.2 arm64, Apple M1 Max with 64 GiB, Rust 1.98.1 and
Python 3.14.6. Immutable before/after release executables, exact input hashes,
source pins and full command receipts live in `target/benchmarks/base079`.
Cargo's release settings are unchanged. Builds, native checks, allocation/drop
probes, save samples, corpus campaigns and tests run sequentially. Host load and
ordinary warm filesystem caches remain uncontrolled; first uses are separate.

### Matched native checks and output fidelity

Each version has two fresh file-backed `zincite-fmt --check` processes per case,
with a 120-second deadline, complete stdout/stderr, child CPU and exact-child
Darwin `wait4` RSS. Every check completes with status 1, empty stdout and empty
stderr. A separate full-source formatter run per version/case retains output;
all six before/after outputs are byte-identical. Two observations are not p95.

| Case | Input bytes | Before wall s | After wall s | Before child CPU s | After child CPU s | Before RSS MiB | After RSS MiB |
| --- | ---: | --- | --- | --- | --- | --- | --- |
| public-wide | 885,733 | 9.515204 / 9.578185 | 0.367222 / 0.364887 | 9.486472 / 9.552512 | 0.364142 / 0.361733 | 104.968750 / 105.562500 | 106.656250 / 105.078125 |
| small-table | 619,533 | 0.080956 / 0.081027 | 0.082091 / 0.080935 | 0.078232 / 0.078349 | 0.078857 / 0.078205 | 49.921875 / 49.984375 | 50.000000 / 50.000000 |
| large-table | 27,775,980 | 3.827149 / 3.720487 | 3.780931 / 3.692232 | 3.820246 / 3.714110 | 3.763152 / 3.678816 | 2221.984375 / 2221.953125 | 2222.015625 / 2222.000000 |
| wide-5000 | 30,014 | 0.278210 / 0.273328 | 0.019601 / 0.018390 | 0.275570 / 0.271389 | 0.017380 / 0.016757 | 6.265625 / 6.265625 | 6.343750 / 6.328125 |
| wide-15000 | 90,014 | 2.429597 / 2.386802 | 0.047523 / 0.046790 | 2.425523 / 2.384120 | 0.045539 / 0.045134 | 14.640625 / 15.625000 | 14.625000 / 14.593750 |
| wide-30000 | 180,014 | 10.851250 / 10.755378 | 0.090804 / 0.090281 | 10.841687 / 10.750073 | 0.088500 / 0.087745 | 32.953125 / 27.343750 | 27.156250 / 27.125000 |

Both 30,000-column checks pass the 0.5-second target; both public-wide checks
pass one second. Fixed-row child CPU rises from about 0.017 seconds at 5,000
columns to 0.045 at 15,000 and 0.088 at 30,000. This is roughly proportional
rather than the retained baseline's quadratic growth. The small and large public
table controls show no material native regression. This local result does not
identify every cost in the full formatter batch.

The existing developer checker runs separately with an adequate 120-second
limit on the public wide and 30,000-column synthetic inputs. Before and after
both pass exact token/tree coverage, original spelling/structure/protected-byte
preservation, clean parse/reparse and second-pass stability. Three independent
parse/format/lint scopes per input return exactly to their recorded live baseline.
The probe checks the caller's `ParsedFile` source and lints it after formatting,
so formatting still leaves the borrowed parse usable.

### Separate allocation attribution

The unchanged phase probe retains the CST during formatting. These are format
phase counters and instrumented times, separate from native process wall/RSS.
Requested bytes describe allocation traffic; retained/peak deltas describe tracked
heap bytes, not the operating system's high-water value.

| Case | Format ms before → after | Allocation calls before → after | Requested bytes before → after | Retained bytes before → after | Peak delta bytes before → after |
| --- | --- | --- | --- | --- | --- |
| wide-5000 | 268.754 → 12.709 | 255,149 → 255,151 | 9,013,982 → 9,014,110 | 65,536 → 65,536 | 1,757,223 → 1,757,223 |
| wide-15000 | 2418.091 → 38.419 | 765,163 → 765,165 | 20,557,123 → 20,557,251 | 262,144 → 262,144 | 3,690,423 → 3,690,423 |
| wide-30000 | 10737.274 → 80.490 | 1,530,176 → 1,530,178 | 41,115,868 → 41,115,996 | 524,288 → 524,288 | 7,380,751 → 7,380,751 |
| public-wide | 9409.815 → 322.422 | 6,250,076 → 6,250,148 | 196,600,862 → 198,862,238 | 2,469,836 → 2,469,836 | 15,857,904 → 15,857,904 |

The local vectors add allocation traffic while eliminating the repeated scans.
Public-wide format traffic increases by 2,261,376 bytes and 72 calls, with the
same retained/peak counters. Its formatting phase still makes about 6.25 million
allocation calls and requests about 199 MB. Scope-drop checks prove release of
tracked live allocations; they do not establish a native allocator plateau or
attribute full-batch high-water memory. No further profiler was needed to confirm
that this change removes the specifically measured fixed-row CPU growth.

### Unchanged formal save budgets

The existing save driver now includes the public wide matrix alongside all 13
established named cases. Every case/settings entry has 50 fresh stdin saves,
complete stdout capture, separate decoded UTF-8 diagnostic capture, status and
child CPU. Default/nested EditorConfig both resolve to the established four-space,
LF, width-120 settings. Separate native stdin saves retain raw complete streams
and exact-child RSS with 30-second watchdogs. All 1,400 sample statuses and
outputs match those native controls. Known-case hashes remain identical to prior
current outputs; invalid syntax still exits 2 with no stdout.

| Save shape | Settings | Input bytes | p50 ms | p95 ms | min–max ms | Native stdin RSS MiB | 100 ms / 64 MiB gate |
| --- | --- | ---: | ---: | ---: | --- | ---: | --- |
| dense-1000000 | D | 966,669 | 88.609 | 90.949 | 87.618–92.103 | 63.953125 | pass |
| dense-1000000 | N | 966,669 | 88.918 | 90.624 | 87.485–91.616 | 63.968750 | pass |
| public-wide | D | 885,733 | 376.136 | 380.463 | 370.734–382.619 | 106.296875 | FAIL latency and RSS |
| public-wide | N | 885,733 | 375.980 | 382.080 | 369.993–416.748 | 105.125000 | FAIL latency and RSS |

The public-wide input is 885,733 bytes, so both misses belong to the unchanged
100 ms / 64 MiB formal gate. Passing its separate one-second optimization target
does not pass format-on-save acceptance. All established representative cases
within 1 MiB still pass their own latency/RSS gates; the supported 1.93 MB dense
case remains outside those interactive size budgets. No hard case or tail was
removed. Denied `time -l` capability attempts remain in the raw save report;
separate `wait4` measurements provide the available RSS.

Campaign first use is 5.383 ms wall / 3.131 ms child CPU on an executable already
used by the matched controls. The first `--help` on each immutable executable
copy is retained separately in the matched reports. These are neither controlled
cold-cache measurements nor evidence that the material fresh-copy launch delay
documented in base-048 has disappeared.

A bounded follow-up must address the public-wide save shape using the retained
885,733-byte input, complete identical output and both configuration variants.
It should attribute the remaining cell-rendering allocation/CPU work separately
from retained parser memory, then keep only a measured local improvement and
recheck 100 ms / 64 MiB with the established dense/small controls. The present
repair does not broaden into that optimization or relax its gate.

### Two full native formatter repeats

The same three positional directories are rediscovered for the fresh campaign:
the Challenge checkout, `~/minizinc`, and the 2026 supplement. The manifest has
6,417 canonical roots and 641,084,859 bytes. All original source hashes and the
formatter's existing EditorConfig sidecars are pinned before execution and
rehashed unchanged afterward. Formatting does not load includes or semantic
sidecars. Each fresh `zincite-fmt --check` has a finite 900-second deadline.

| Repeat | Native wall s | Child CPU s | Native RSS MiB | Status | Attempted roots through completion |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 109.376963 | 108.456146 | 8083.250000 | 2 | 6,417 |
| 1 | 110.226764 | 108.209802 | 8715.890625 | 2 | 6,417 |

Both commands finish, with exact matching complete stdout/stderr and status 2.
The existing dangling non-source file remains a discovery error; input/syntax
errors also remain in stderr. Completed attempts do not establish that every
input is valid or formats successfully. Native RSS reaches about 7.89/8.51 GiB,
a substantial remaining batch cost. No completed-root count was inferred from a
stderr prefix, and no timed-out root was excluded. The historical base-049
300-second cutoffs stay retained. This fresh completion follows the current
source state; it does not attribute all improvement from that older revision to
this single row-classification change.

### Fresh syntax/format corpus and validation

The fresh existing corpus runner uses the current pinned checker without
`--rules`, ten-second per-file deadlines and a finite 1,800-second campaign
limit. It attempts all 6,417 canonical inputs, rehashes every original unchanged
and reaches no campaign cutoff. The checker campaign takes 268.562 seconds.
There are 6,390 clean completions and 27 retained gaps: six timeouts, 12 syntax
assessments, four model-constructs-in-data inputs and five invalid UTF-8 files.
No row is Unobserved. All jointly reported raw results are identical to base-077
(6,394 rows); the remaining 17 former timeouts now complete cleanly. Input paths,
sizes and hashes match that campaign exactly. Rejected/timeout rows and all raw
streams remain in `target/corpus/base079-final`; this is not complete language
or semantic acceptance.

The public wide and synthetic 30,000-column inputs have the separate adequate
120-second before/after fidelity controls above. Current small/27.8 MB table
supplements also pass exact token/tree coverage, spelling/structure/protected
bytes, clean parse/reparse and idempotence within 120-second limits. Their
checker wall times are 0.191 and 8.811 seconds, with native RSS 136.703125 and
4,726.343750 MiB. Separate three-scope drop probes complete in 0.249/11.312
seconds and return exactly to their tracked live baselines. All native streams
and original input pins remain retained; these drop observations do not erase
the full native high-water or the remaining six corpus timeouts.

Required `cargo fmt --all -- --check`,
`cargo clippy --offline --workspace --all-targets -- -D warnings` and
`cargo test --offline --workspace` pass (189 existing tests).
`git diff --check` passes. No tracked test was added: existing public matrix,
comment/protected-range, width-break, BOM/settings and layout checks cover the
unchanged behavior. The initial private reconciliation helper incorrectly
expected the checker fields to say `match` instead of the established `ok`;
its script and setup note remain retained. Corrected reconciliation passes
without changing source behavior. Earlier denied capabilities, first-use tails,
base-049 cutoffs and base-077/078 evidence stay intact.

Repeat from the repository root with new report output paths:

```sh
cargo build --offline --release -p zincite-fmt --bin zincite-fmt \
  --example profile-phases --example check-corpus-file -p zincite-lint --bin zincite-lint
python3 scripts/bench-save.py batch \
  /private/tmp/portfolio-150-mzn-challenge-a844 /Users/zayenz/minizinc \
  /private/tmp/zincite-base023-mznc2026-probs \
  --selection format --native-only --repeat 2 --timeout 900 --probe-timeout 120 \
  --output target/benchmarks/NEW-FORMAT-REPORT
python3 scripts/bench-save.py --samples 50 \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/ATSP/atsp.mzn \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2021/java-routing/trip_7_4.mzn \
  --external /private/tmp/portfolio-150-mzn-challenge-a844/2019/groupsplitter/u7g2pref1.dzn \
  --output target/benchmarks/NEW-SAVE-REPORT.json
python3 scripts/check-corpus.py \
  --challenge /private/tmp/portfolio-150-mzn-challenge-a844 \
  --local /Users/zayenz/minizinc --supplement /private/tmp/zincite-base023-mznc2026-probs \
  --binary target/release/examples/check-corpus-file --timeout 10 --campaign-timeout 1800 \
  --output target/corpus/NEW-FORMAT-CORPUS
```

The ignored `target/benchmarks/base079` contains exact before/after binaries,
controls, source/configuration pins, commands and every native/probe stream.
`matched.py`, `save-campaign.py`, `save-native.py`, `full-format.py` and
`table-supplement.py` record supplementary recipes with exclusive output paths;
use a new report directory when replaying. `reconciliation.json` and
`corpus-audit.json` retain exact comparisons, while the compact validation and
ownership indices identify the frozen files and all terminal/reaped commands.
Only `crates/zincite-fmt/src/lib.rs` and these two performance reports change.
The six foreign files, external originals and index remain unchanged; task
records, lifecycle and commit belong to the coordinator. Current wide-save
latency/memory misses, high full-batch RSS, corpus gaps and existing incomplete
semantic acceptance continue to block the final gate.

## Base082: current batch resource attribution

This checkpoint measures HEAD `97c58b3` with the pending base081 formatter and
private boxed-node changes. Those changes remain unaccepted. Base082 changes
only the attribution example, benchmark reporting and this report; it does not
repair production lint or declare the performance gates passed.

The current corpus uses `/private/tmp/zincite-base080-challenge-a844`,
`/Users/zayenz/minizinc` and `/private/tmp/zincite-base023-mznc2026-probs`:
2,040 + 4,287 + 90 = 6,417 source files, 641,084,859 bytes. The recovered
Challenge tree is revision `a8448864fc56162583f24aaf9c25653d93f83765`. The
old archive paths in earlier reports are historical commands. They are not
current replay inputs.

Raw evidence is in `target/benchmarks/base082/`. `current-source.json` and
`current-binaries.json` pin the current build separately from the retained
base078/base079 and base081 snapshots. `current-originals.json` pins 7,759
original source, control, standard-library, configuration and `.model` import
files before measurement. `campaign-environment.json` records load averages
812.816/783.038/776.164. Wall time and child CPU remain separate observations;
these loads do not establish the cause or share of any delay.

### Completion reporting

The driver now records the actual native binary for each selection. A mixed
formatter/linter campaign has no single `timed_native_binary`; the per-selection
map and each run's binary hash identify its executable. Native commands do not
emit per-root completion counts, so their `completed_root_count` and completion-
based throughput remain unavailable. Earlier driver counts were inferred from
terminal status 0/1/2. A valid formatter `--check` reaching status 2 is consistent
with finishing its loop, but status alone is not a structured completion record.

The separate allocation probe retains explicit root, drop and complete records.
Its new flushed `begin` records identify that probe's last unfinished phase if
censored. They do not identify a native command's currently executing root.
Coverage remains specific to the companion process and its exact rule partition;
no native count is borrowed from it.

### Independent dense files

Each file has 966,669 bytes and a distinct canonical path. These are single fresh
native observations, not p95 measurements. The formatter used `--check`; lint
used `--rules all`. All commands finished without a timeout, with formatter
status 1 and lint status 0. The allocation companions completed all 1/3/10/20
roots and returned every scope to its measured baseline. In the 20-root
companion, the largest retained/peak read-parse delta was 47,266,978 bytes.
The native RSS still grew across the measured file counts; scope release does
not establish a native memory bound.

| Files | Format wall/CPU (s) | Format RSS (MiB) | All lint wall/CPU (s) | All lint RSS (MiB) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1.620/0.096 | 53.828125 | 1.074/0.064 | 51.468750 |
| 3 | 2.636/0.257 | 72.625000 | 0.593/0.160 | 66.859375 |
| 10 | 14.636/0.813 | 179.968750 | 5.845/0.502 | 177.484375 |
| 20 | 23.617/1.616 | 333.109375 | 14.583/0.994 | 330.578125 |

A separate owned probe parsed, formatted and ran default lint on the same
one-file input three times, dropping each scope. Its tracked live bytes returned
to 968,560 after each iteration; the original input remained held. During a
finite post-drop hold, `heap` observed 193 allocations/996,928 bytes. `vmmap`
reported a 52.7M physical footprint and `MALLOC_LARGE (empty)` regions with 43.2M
resident/dirty pages, plus resident empty small regions. This demonstrates
freed allocator-region residency in this process. It does not prove a whole-
batch plateau or attribute all of the older multi-GiB peaks to allocator
retention. The probe's wait4 high-water was 71.78125 MiB; footprint, current
resident regions, tracked allocations and wait4 high-water are different values.

### Expanded analysis

The retained diagnostic family contains 100/1,000/10,000 simple constraints.
Native default/thesis/all commands and separate allocation companions each
completed their single root without a timeout. Default emitted one warning per
constraint; all emitted two. The model companions retained two limitations,
including `unused-declaration` Limited; registration does not imply complete
analysis. `controls/*/report.json` retains every actual partition over the
2/14/26 selected rules.

| Constraints | Default wall/CPU (s) | Thesis wall/CPU (s) | All wall/CPU (s) | All RSS (MiB) |
| ---: | ---: | ---: | ---: | ---: |
| 100 | 0.182/0.007 | 7.465/0.401 | 8.593/0.405 | 25.953125 |
| 1000 | 0.132/0.039 | 6.065/0.439 | 5.397/0.525 | 28.343750 |
| 10000 | 1.772/0.320 | 30.902/1.787 | 91.856/5.826 | 63.046875 |

For all rules, analyze allocation calls grew from 342,020 to 876,970 to
50,776,266, and requested bytes from 37,708,010 to 87,486,421 to 4,483,837,806.
A separate 25,000-constraint allocation probe completed in 554.336590 wall
seconds/31.854014 child CPU seconds, status 1, 96.75 MiB high-water. Its analyze
phase requested 27,445,446,466 bytes in 313,941,728 calls, with a 72,303,320-byte
peak delta; the final root drop returned 2,396 live bytes to 2,396. All 26 rule
partitions reconcile to one root, with no Unobserved roots. The large allocation
traffic is not the same as retained live memory.

Two separate three-second samples of that exact owned analysis child locate
concrete work. The early interval captured 2,352 stacks under
`analyze_model -> resolve_integer_bounds -> domains::Bounds::walk`, with 2,300
collapsed top-of-stack observations in that walk. The source scans
`calls.expressions` for every syntax node. The later interval captured 2,366
stacks under `resolve_iteration_facts -> iteration::Producer::walk`, including
source-location/column lookup and allocator calls. Its expression search
constructs `self.location(file, node)` inside each comparison, repeatedly
allocating a path and calculating a column. These are interval observations,
not percentages of total CPU or an attribution of the entire batch.

The necessary local repair begins by hoisting the invariant location/range out
of the iteration search. Remaining repeated lookups warrant a bounded local
membership/index experiment that preserves full file/range/type and unknown
semantics. Public `CallableFacts.expressions` can be modified by consumers;
a repair must not silently assume externally supplied facts are sorted. Reuse
the existing fact/rule behavior checks and compare the same native diagnostic
and shared-include families, allocation/drop evidence and full selected-rule
partitions. Base082 makes no production repair.

### Current full attempts and remaining boundary

| Selection | Wall / child CPU (s) | Peak RSS (MiB) | Status | Timeout |
| --- | ---: | ---: | ---: | --- |
| format | 600.151558 / 50.074347 | 3051.531250 | -9 | yes |
| default | 460.302800 / 45.624998 | 3551.703125 | 2 | no |
| thesis | 600.034272 / 45.784223 | 111.734375 | -9 | yes |
| all | 600.324593 / 188.971768 | 1675.468750 | -9 | yes |

Each native attempt selected the full 6,417-file denominator and had a finite
600-second deadline. The formatter/thesis/all cutoffs leave native completion
unobserved. Default returned status 2 without a timeout; its completion count
also remains unobserved. Different progress and wall/CPU gaps prevent treating
these high-water values as equivalent amounts of completed work or as proof of
a new memory bound. `full/native/report.json` and `runs.jsonl` retain all four
commands, binaries, raw streams and exact wait4 resources. `full/commands.json`
retains the native-driver receipt separately from its native children.

The separate all-rules companion reached its 600-second deadline: 600.066809
wall seconds, 41.609889 child CPU seconds, 102.5625 MiB RSS, status -9, reaped.
It emitted 231 root/drop records: 199 standalone data roots and 32 complete
resolved model roots. All 231 emitted drops returned to baseline. Each of its
26 rule partitions sums to 6,417, including exactly 6,186 Unobserved roots;
`full/all-coverage.json` retains every Completed/Inapplicable/Limited count.
For example, both default rules completed on all 231 observed roots;
`unused-declaration`, `search-coverage` and `vacuous-constraint` were Limited on
all 32 observed models, and Inapplicable on the 199 data roots. There was no
complete record. The last flushed begin identifies only this companion's
`2010/bacp/bacp-27.mzn` analysis. The interrupted scope has no final allocation
or drop record. This cutoff is not a source rejection or whole-corpus coverage.

Repeat the native protocol with a new output directory and the same frozen
binaries and inputs:

```sh
base082_bin=target/benchmarks/base082/current
python3 scripts/bench-save.py batch \
  /private/tmp/zincite-base080-challenge-a844 \
  /Users/zayenz/minizinc /private/tmp/zincite-base023-mznc2026-probs \
  --formatter "$base082_bin/zincite-fmt" \
  --linter "$base082_bin/zincite-lint" --probe "$base082_bin/profile-phases" \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  --native-only --repeat 1 --timeout 600 --probe-timeout 120 \
  --output target/benchmarks/base082-replay
```

The retained private `base082-controls.py`, `base082-observe.py` and
`base082-full.py` command receipts describe the exact sequential controls,
post-drop observations and separate companion. Pin the source snapshot and
binaries again before comparing a different build. The controls wrapper's
outer tool identifier was not retained: its terminal marker and subsequent
PID-21740 absence are recorded, and all seven driver children have terminal
wait4 receipts. Its outer exit status and tool-reap observation are unavailable;
they are not replaced with a guessed success. The known build, retry,
observation, sampler and full-campaign handles are terminal and reaped.


The remaining memory repair must distinguish an active root's live CST/facts
from freed allocator pages in the native process. The independent controls show
native growth and a post-drop allocator observation; they do not explain every
full-corpus high-water page or prove bounded accumulation. Preserve the large
inputs and separate native/phase measurements when checking a necessary
syntax-storage or allocation-lifetime repair. No allocator replacement, cache,
parallel engine or blanket representation change is established by this work.
Expanded analysis retains the concrete lookup/location repair boundary above.
Neither resource gap is silently accepted, and base081's save wall-time misses
remain separate and unresolved.

The attribution example passed Cargo formatting, Clippy and all 189 workspace
tests. The initial test attempt failed at the existing CLI helper's BrokenPipe
race; its complete streams remain separate from the successful focused/full
retry and release build. No timing or reporting-only test was added.
