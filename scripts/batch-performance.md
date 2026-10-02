# Batch performance checkpoint

The [base-074 comparison below](#expanded-lint-comparison-base-074) records the current 26-rule checkpoint. The original base-049 measurements below remain historical evidence.

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
