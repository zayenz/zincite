# Current lint runtime: five model roots

2026-10-15, task base-138. The current six-rule default took 0.086–1.414 seconds
across five named models. Thesis/all took up to 8.753 seconds on generated
java-routing. Every observed process finished within its 60-second cap, but
several rule results remain Limited. These observations establish current costs
for these roots; they do not establish whole-corpus performance or acceptance.

## Build, inputs and prechecks

Source revision: `d2934e810a9ac2a3f98dc66bcff3bc764499e4e8`, release build,
Rust `1.98.1 (48a229cea 2026-09-01)`, arm64 macOS 26.6.2, Python 3.14.5.
The repository's previously recorded host configuration is Apple M1 Max with
64 GiB RAM; fresh CPU/RAM introspection was unavailable in the sandbox.
The installed MiniZinc is 2.10.1, build 33348285743.

```sh
CARGO_BUILD_JOBS=2 cargo build --release --locked \
  --target-dir /tmp/zincite-target-138 \
  -p zincite -p zincite-lint -p zincite-fmt --bins --example profile-phases
```

All five originals passed the following no-solve precheck, with exit 0 and empty
stdout/stderr, in 0.130–0.287 seconds each. No data instance was needed for model
checking or source linting. No input was unavailable or compiler-rejected.

```sh
minizinc --model-check-only --solver gecode \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  --time-limit 60000 --output-base /tmp/zincite138-20261015/CASE ROOT
```

The table paths are relative to `/Users/zayenz/minizinc/`. Only these five files
were root arguments; loaded dependencies were not additional analysis roots.
The private sources stay outside the repository.

| Case | Root | Bytes | Reason for selection |
| --- | --- | ---: | --- |
| assignment-pool | `projects/assignment-pool/older/base-model/base-model.mzn` | 2,655 | Ordinary edited model: records, labelled constraints, small comprehensions and a local function. |
| sudoku | `software/libminizinc/docs/en/examples/sudoku/sudoku.mzn` | 1,239 | Shared library and comprehensions; the public Sudoku/all_different callable shape had retained limitations in historical base-076. |
| atsp | `challenge-models/2021/problem-set/mznc2021_probs/ATSP/atsp.mzn` | 6,899 | Nested and filtered comprehensions over several parameter domains, retained from the save workload. |
| seating | `models/misc/seating/seating.mzn` | 10,606 | Shared globals/Gecode library, structured records, set comprehensions, local decision-dependent functions and search annotations. |
| java-routing | `challenge-models/2021/problem-set/mznc2021_probs/java-routing/trip_7_4.mzn` | 85,229 | Generated dense constraints, many diagnostics and explicit int_search; a larger callable/definition/search workload retained from the save set. |

## Measurement

Measurements ran sequentially in an agreed quiet window after other builds and
validation finished. Each row contains three fresh native processes, followed
by one separate instrumented `profile-phases batch` companion for exact rule
outcomes. Wall time surrounds process creation through completion; RSS is each
native child's OS high-water value from `wait4`, converted from bytes to MiB.
Instrumented companion time/RSS is retained separately in the raw report and
is not substituted for native timing. Caches were ordinarily warm. Sample 1 is
first invocation for that case/preset, with no controlled cold-cache claim.
Three samples provide a spread, not a p95.

The temporary controller imports the existing `scripts/bench-save.py::file_invoke`
and uses the existing unchanged base-122 native waitid bridge because this
CPython lacks `os.waitid`. The bridge reserves the PID with WNOWAIT until the
helper's sole wait4 reaper runs. Every child has a finite deadline and a reaped
receipt. The controller stops a root/preset after its first timeout; none timed
out. Its shared 1,080-second process budget includes prechecks, native processes,
companions and the initial incomplete library setup described below. Total
process wall time was **117.131 seconds**: five prechecks, 54 native processes
and 18 companions. There was no solving or instance product.

The native command for each root/preset was:

```sh
/tmp/zincite-target-138/release/zincite-lint \
  --isolated --show-limitations --rules SELECTION \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc ROOT
```

For seating, add `-I /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc/gecode`.
The companion uses `/tmp/zincite-target-138/release/examples/profile-phases batch
--rules SELECTION --stdlib-dir STDLIB [same -I] ROOT`. Explicit roots and
selections avoid directory discovery and any corpus sweep.

Selections are the current `default` (6), `thesis` (14) and `all` (28), whose
rule membership is documented in the [lint guide](../docs/linting.md).
The exact expansion order from this build is retained here:

- `default`: `naming`, `missing-constraint-label`, `prefer-array-comprehension`, `index-set-mismatch`, `suspicious-shadowing`, `partial-expression`.
- `thesis`: `array-index-start`, `compact-if`, `constant-variable`, `effective-zero-one`, `element-predicate`, `reified-global`, `global-variable-in-function`, `unbounded-variable`, `search-coverage`, `decision-variable-operator`, `unmarked-symmetry-breaking`, `unused-declaration`, `decision-variable-generator`, `decision-variable-condition`.
- `all`: `naming`, `missing-constraint-label`, `prefer-array-comprehension`, `array-index-start`, `compact-if`, `constant-variable`, `effective-zero-one`, `element-predicate`, `reified-global`, `global-variable-in-function`, `unbounded-variable`, `search-coverage`, `decision-variable-operator`, `unmarked-symmetry-breaking`, `unused-declaration`, `decision-variable-generator`, `decision-variable-condition`, `suspicious-shadowing`, `expensive-comprehension`, `index-set-mismatch`, `hidden-optionality`, `partial-expression`, `vacuous-constraint`, `unused-generator-binding`, `global-constraint-opportunity`, `missing-input-precondition`, `suspicious-domain`, `prefer-enum`.

## Native elapsed time and memory

Triples retain samples 1, 2 and 3 in order. RSS triples correspond to those same
processes, rather than separate memory-only invocations. All native stdout was
empty, and each root/preset's complete stderr and status matched across repeats.

| Root | Preset | Elapsed seconds, samples 1 / 2 / 3 | Peak child RSS MiB, samples 1 / 2 / 3 |
| --- | --- | --- | --- |
| assignment-pool | default | 0.0971 / 0.0955 / 0.0939 | 38.625 / 37.750 / 37.562 |
| assignment-pool | thesis | 0.1552 / 0.1587 / 0.1576 | 42.297 / 41.531 / 41.969 |
| assignment-pool | all | 0.1615 / 0.1631 / 0.1633 | 40.781 / 41.781 / 42.000 |
| sudoku | default | 0.0873 / 0.0857 / 0.0865 | 31.422 / 31.656 / 32.516 |
| sudoku | thesis | 0.2045 / 0.1995 / 0.1971 | 46.516 / 44.141 / 44.406 |
| sudoku | all | 0.2079 / 0.2041 / 0.2032 | 44.797 / 44.531 / 44.547 |
| atsp | default | 0.1605 / 0.1719 / 0.1719 | 41.484 / 41.266 / 42.953 |
| atsp | thesis | 0.2649 / 0.2626 / 0.2386 | 43.328 / 43.078 / 41.938 |
| atsp | all | 0.2727 / 0.2715 / 0.2705 | 45.250 / 45.641 / 45.922 |
| seating | default | 0.7354 / 0.7294 / 0.7286 | 108.734 / 109.891 / 108.016 |
| seating | thesis | 2.0006 / 1.9971 / 1.9969 | 141.031 / 139.625 / 142.891 |
| seating | all | 2.0470 / 2.0265 / 2.2023 | 140.844 / 142.234 / 140.359 |
| java-routing | default | 1.3715 / 1.4143 / 1.3780 | 113.812 / 113.594 / 114.828 |
| java-routing | thesis | 8.4826 / 8.7525 / 8.4125 | 134.328 / 137.344 / 141.000 |
| java-routing | all | 8.4233 / 8.4283 / 8.4403 | 138.312 / 138.531 / 138.656 |

## Diagnostics and exact rule outcomes

W/I/E/L means warning, informational finding, error and located limitation counts.
The existing probe's `warnings` field includes informational findings; the table
splits them using the full native diagnostic stream. Each companion agrees with
the native status and complete stderr after removing the native `[root PATH]`
prefix. C/L counts are selected rules Completed/Limited. No selected rule was
Inapplicable, NotRun or Unobserved. Every rule not listed as Limited below is
Completed; this specifies the exact partition for each selection. Findings can
coexist with Limited results.

| Root | Preset | Exit | W / I / E / L | C / L | Limited rule IDs |
| --- | --- | ---: | --- | --- | --- |
| assignment-pool | default | 0 | 0 / 0 / 0 / 1 | 5 / 1 | `partial-expression` |
| assignment-pool | thesis | 1 | 2 / 0 / 0 / 13 | 12 / 2 | `unbounded-variable`, `search-coverage` |
| assignment-pool | all | 1 | 2 / 3 / 0 / 20 | 24 / 4 | `unbounded-variable`, `search-coverage`, `partial-expression`, `vacuous-constraint` |
| sudoku | default | 1 | 6 / 0 / 0 / 1 | 5 / 1 | `partial-expression` |
| sudoku | thesis | 0 | 0 / 0 / 0 / 0 | 14 / 0 | none |
| sudoku | all | 1 | 6 / 4 / 0 / 1 | 27 / 1 | `partial-expression` |
| atsp | default | 1 | 23 / 0 / 0 / 0 | 6 / 0 | none |
| atsp | thesis | 1 | 14 / 0 / 0 / 0 | 14 / 0 | none |
| atsp | all | 1 | 37 / 8 / 0 / 6 | 26 / 2 | `vacuous-constraint`, `suspicious-domain` |
| seating | default | 1 | 12 / 0 / 0 / 8 | 4 / 2 | `index-set-mismatch`, `partial-expression` |
| seating | thesis | 1 | 10 / 0 / 0 / 86 | 9 / 5 | `compact-if`, `element-predicate`, `unbounded-variable`, `search-coverage`, `unused-declaration` |
| seating | all | 1 | 23 / 4 / 0 / 95 | 20 / 8 | `compact-if`, `element-predicate`, `unbounded-variable`, `search-coverage`, `unused-declaration`, `index-set-mismatch`, `partial-expression`, `vacuous-constraint` |
| java-routing | default | 1 | 684 / 0 / 0 / 0 | 6 / 0 | none |
| java-routing | thesis | 1 | 568 / 0 / 0 / 1 | 13 / 1 | `search-coverage` |
| java-routing | all | 1 | 1252 / 0 / 0 / 1 | 27 / 1 | `search-coverage` |

All five final roots have a solve item and resolved dependencies. Pool and ATSP
load 179 files each, Sudoku 204, java-routing 598 and seating 599. Library loading
and source analysis are included in native elapsed time. Library sharing here
means overlapping dependencies across independent roots; no cross-process cache
or multiple-root reuse benefit was measured.

## Initial seating setup failure

The initial seating run supplied the core stdlib without its solver-specific
include directory. All three native repetitions per preset returned 2 with one
`cannot resolve include 'gecode.mzn'` error at the root's bytes 724..736.
Every selected rule was Limited: 0/6, 0/14 and 0/28 Completed/Limited. Default,
thesis and all respectively reported W/I/E/L of 12/0/1/8, 9/0/1/14 and 22/4/1/23.
Their native elapsed triples were 0.7868/1.2820/0.9822,
1.3273/1.3186/1.3118 and 1.3274/1.3667/1.3820 seconds, with RSS triples
108.094/107.547/109.125, 148.250/149.047/149.000 and
151.172/150.859/150.594 MiB. Companions matched those errors and partitions.

The installed file exists under `STDLIB/gecode`. Repeating only seating with
that explicit include path matches the compiler's solver-library setup and gives
the final rows above. The first capture remains separate; it is a setup failure,
not evidence that MiniZinc rejected the original. No source or semantic repair
was made.

## Conclusions and retained captures

The six-rule default stays below 0.18 seconds for pool, Sudoku and ATSP, but
java-routing takes about 1.4 seconds even before the expanded rules. Thesis/all
rise to about 8.4–8.8 seconds there, with search-coverage still Limited at an
installed `element.mzn` output-dependency traversal. A bounded profile of that
single unchanged root, comparing default with thesis/all, is the concrete next
performance check. These timings alone do not identify a hot function or
justify caching, parallelism or a semantic rewrite.

Completeness also needs separate attention. Pool retains unsupported max/domain
and structured-projection facts; Sudoku retains a partial-expression limit;
ATSP's all selection retains vacuous-constraint and suspicious-domain limits.
Seating reaches the installed library with no load error but retains ambiguous
`relax_and_reconstruct` and unsupported library/body facts. Faster execution or
exit 0 cannot replace those per-rule outcomes. The current Sudoku thesis row
completes all fourteen rules, unlike the dated base-076 observation, but this is
not a matched before/after performance comparison.

The older [batch report](batch-performance.md) and
[semantic corpus checkpoint](semantic-corpus-checkpoint.md) retain their own
revisions, rule counts and workloads. Their full-corpus and hard-root observations
are historical evidence; this five-root check does not supersede those campaigns
or claim their remaining gaps are resolved.

Raw private captures remain untracked under `/tmp/zincite138-20261015/`:
`report.json`, `prechecks.json`, `measure.log`, complete named stdout/stderr files,
`seating-gecode/report.json`, `seating-measure.log` and `reconciliation.json`.
The temporary controllers are `/tmp/zincite138-measure.py` and
`/tmp/zincite138-seating.py`; they reuse the existing driver and add no tracked
benchmark framework. The reports retain commands, exact seconds/RSS bytes,
selected rule IDs/results, binary/source hashes and deadline/reap receipts.
The initial run rehashed 1,027 originals/library files; the seating repeat rehashed
1,023. Every hash and both binaries remained unchanged. No production Rust,
Python, fixture or test changed.
