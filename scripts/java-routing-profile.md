# Java-routing lint cost

2026-10-15, task base-139. The near-nine-second thesis/all cost is concentrated
in callable-definition analysis shared by `search-coverage` and
`unbounded-variable`. Removing search leaves that cost. Two native CPU samples
locate most observed stacks in `BodyInterpreter::literal_call_outputs`, including
repeated location construction during expression lookup. This supports a small
follow-up there; it does not establish a speedup or resolve the search limitation.

## Matched setup and budget

Source revision: `c6eebe53cdc22e612ab70bdb48c9059c9e7d4371`, Rust
`1.98.1 (48a229cea 2026-09-01)`, Python 3.14.5, arm64 macOS 26.6.2.
Previously recorded hardware is Apple M1 Max, 64 GiB RAM; hardware was not
re-inspected. The coordinator paused its other heavy work and all task-owned
builds and measurements ran serially. Other project chats were active, and host
contention was not controlled. Child CPU times below help expose that uncertainty.

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_RELEASE_DEBUG=1 \
  CARGO_TARGET_DIR=/tmp/zincite-target-139 cargo build --release --locked \
  -p zincite-lint -p zincite-fmt --bin zincite-lint --example profile-phases

ROOT=/Users/zayenz/minizinc/challenge-models/2021/problem-set/mznc2021_probs/java-routing/trip_7_4.mzn
STDLIB=/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc
/Applications/MiniZincIDE.app/Contents/Resources/minizinc \
  --model-check-only --solver gecode --stdlib-dir "$STDLIB" \
  --time-limit 60000 --output-base /tmp/zincite139-20261015/precheck "$ROOT"
```

The unchanged original is 85,229 bytes, SHA-256
`41fdec8b328f51033c6601b117f518f6a0e5fdf7197dc07dd7509fe3ee316e9a`.
It was the only root; there was no instance or solver run. The fresh precheck
returned 0 in 0.3883 s with empty streams. Installed MiniZinc is the same 2.10.1
installation used in [base-138](lint-performance-current.md). The closure has
598 loaded files and 855 include edges, implicit core present, no unresolved
include, parse error, load error or load limitation, and one user solve item.
No solver-specific `-I` was needed for this root.

The new release binaries include debug information; they were used consistently
for native timings and sampling. Their SHA-256 identities are:

- Native: `08343343c3fbebcb2a347447d10edfceceadbe6984f6eeb94dc582817c15f6fa`.
- Phase probe: `e98368ca37611f89e8d91c57cdf45f202646627f8b8063d083a8215f2fe9c616`.

One ledger records **18 target lint/probe launches**, each with a 60-second
deadline, and **125.856 aggregate process-seconds**, including the precheck.
There were no cutoffs; all 19 children were reaped. Two sampler attachments
requested five seconds each, below the three-attachment limit. No further target
launch was made after reaching 18. All 598 input hashes and all three probe/native
binary hashes matched before and after. Production source remained unchanged.

## Fresh native baselines and outcomes

Each preset ran three fresh native processes, then one separate allocation
counting companion. Caches were ordinarily warm; first samples are not controlled
cold-cache measurements. Three observations establish a spread, not a p95.
Elapsed time covers process creation through completion; CPU and high-water RSS
come from that child's `wait4` receipt, with macOS bytes converted to MiB.

```sh
/tmp/zincite-target-139/release/zincite-lint \
  --isolated --show-limitations --rules SELECTION --stdlib-dir "$STDLIB" "$ROOT"
/tmp/zincite-target-139/release/examples/profile-phases batch \
  --rules SELECTION --stdlib-dir "$STDLIB" "$ROOT"
```

| Selection | Native elapsed s, samples 1 / 2 / 3 | Child CPU s, same samples | Peak RSS MiB, same samples |
| --- | --- | --- | --- |
| default | 1.4369 / 1.3783 / 1.3649 | 1.3902 / 1.3685 / 1.3601 | 116.547 / 110.203 / 113.969 |
| thesis | 8.4642 / 8.6070 / 8.4694 | 8.4204 / 8.5176 / 8.4119 | 134.328 / 138.547 / 141.016 |
| all | 9.3733 / 9.0796 / 8.5702 | 8.6121 / 8.6865 / 8.5310 | 141.375 / 141.234 / 141.375 |

All native stdout was empty. Every repetition's status and complete stderr
matched, and each companion matched after removing the CLI's root prefix.
All runs exited 1 because of warnings. Preset membership/order is unchanged
from the exact lists in [base-138](lint-performance-current.md#measurement).

| Selection | Warnings / info / errors / located limitations | Completed / Limited |
| --- | --- | --- |
| default | 684 / 0 / 0 / 0 | 6 / 0 |
| thesis | 568 / 0 / 0 / 1 | 13 / 1 |
| all | 1,252 / 0 / 0 / 1 | 27 / 1 |

The only Limited rule is `search-coverage`; every other selected rule Completed.
There were no Inapplicable, NotRun or Unobserved results. Nonzero warning counts
by rule are `naming` 113 and `missing-constraint-label` 571 in default/all;
`element-predicate` 155, `decision-variable-operator` 399 and `search-coverage` 14
in thesis/all. Every other selected rule reported zero findings. The limitation
is still installed `std/element.mzn:14:72`, bytes 544..552: output dependency
array access has no exact traversal membership proof. Its presence supplies no
timing attribution by itself.

## Measured phase and rule comparisons

The existing `profile-phases batch` companion separates load, analyze and render.
It adds atomic allocation counters; its times are instrumented observations,
not replacements for the native baselines. Analyze includes preparing demanded
facts and fresh diagnostic policy, and drops its temporary `ModelFacts` owner.

| Selection | Process s | Load / analyze / render ms | Analyze allocation calls | Analyze requested bytes | Analyze peak delta bytes |
| --- | ---: | --- | ---: | ---: | ---: |
| default | 1.3633 | 55.966 / 1,276.303 / 16.411 | 1,779,177 | 248,347,533 | 66,558,470 |
| thesis | 9.0373 | 60.307 / 8,950.831 / 13.812 | 125,755,047 | 12,305,512,039 | 94,353,578 |
| all | 9.0261 | 61.126 / 8,918.885 / 30.863 | 125,807,269 | 12,318,957,742 | 95,241,346 |
| search-coverage only | 7.4053 | 57.781 / 7,331.666 / 0.616 | 124,895,111 | 12,140,852,068 | 62,542,035 |
| thesis without search-coverage | 8.8312 | 58.565 / 8,746.677 / 13.272 | 125,742,232 | 12,302,148,416 | 92,960,858 |
| unbounded-variable only | 7.4881 | 59.324 / 7,414.004 / 0.001 | 124,892,763 | 12,138,654,405 | 61,149,315 |

The last three commands use the same companion and include configuration.
“Thesis without search” is the explicit comma-separated thesis list with
`search-coverage` removed, not an unsupported exclusion selector. Exact commands
and ordered expansions are in the raw ledger/companion manifests.
Search-only retained 14 warnings and its one limitation (exit 1, 0/1 C/L).
Thesis without search retained 554 warnings, no limitation (exit 1, 13/0 C/L).
Unbounded-only produced no diagnostics (exit 0, 1/0 C/L). These comparisons remove
advice and its completeness result; they are not semantically equivalent presets.

All six load phases made 280,245 allocation calls, requested 40,412,584 bytes,
and retained 21,233,972 bytes. Loading alone therefore does not explain the
seven-second expansion. Rendering 1,252 warnings took 30.9 ms. About 12.3 GB
requested during thesis analysis is cumulative allocation traffic, not live
memory, RSS, or a leak. Its retained result delta was 465,854 bytes; native RSS
was 134–141 MiB. The thesis/all companions used 8.8230/8.8938 child CPU seconds,
versus native thesis 8.4119–8.5176 and all 8.5310–8.6865. Counter overhead and
host variation remain mixed; no fixed overhead correction is justified.

A temporary optimized public-API probe loaded one context and requested facts
in dependency order, timing each call while retaining earlier facts. It reported:

| Demand | Incremental ms |
| --- | ---: |
| load | 59.764 |
| bindings | 16.086 |
| source_callables (signature/source typing preparation) | 65.596 |
| instantiations | 5.906 |
| domains | 77.253 |
| definitions | 13.782 |
| callable_definitions | 6,903.732 |
| search | 13.477 |
| complete_definitions | 0.768 |
| thesis policy and remaining fact demand | 1,345.414 |
| render | 14.056 |

The process took 8.5549 s (8.5263 CPU s), and reproduced all fourteen thesis
rule outcomes and its diagnostic stream. The probe's own exit 0 is not a lint
success result. Its request order differs from normal analysis, so these are
incremental demands on one retained owner, not independent costs to sum across
rules. In particular, the remaining 1.345 s contains other demanded facts as
well as policy; it does not isolate each remaining rule. The shared
`ModelFacts::callable_definitions` demand accounts for most of the expansion;
search processing after that demand is small in this ordering.

## Native CPU stacks and next step

Two separate native children were sampled with these bounded commands:

```sh
/usr/bin/sample 13591 5 1 -file /tmp/zincite139-20261015/thesis.sample.txt
/usr/bin/sample 14118 5 1 -file /tmp/zincite139-20261015/unbounded.sample.txt
```

Both attachments succeeded with elevated sampler access. Sampling started after
startup and ended when the child exited: the captures contain 2,969 and 2,382
main-thread observations, not five full seconds of uniform whole-process coverage.
Thesis had 2,921/2,969 observed stacks in callable-definition model-root
interpretation and 2,568/2,969 (86.5%) on its main `literal_call_outputs` branch.
Unbounded-only had 2,361/2,382 in the same interpretation path and
2,093/2,382 (87.9%) on that branch. These are inclusive counts within each
sample window, not whole-run CPU percentages or additive function totals.

The common path is `analyze_model_with_facts` → `ModelFacts::callable_definitions`
→ `resolve_callable_definitions_view` → `interpret_model_roots` →
`BodyInterpreter::{interpret_root,interpret_forwarded,interpret_body}` →
`literal_call_outputs`. Thesis enters through `ModelFacts::search`;
unbounded-only through `ModelFacts::complete_definitions`. The prominent
expression-lookup closure descends into malloc/free and `ParsedFile::line_column`.
The latter alone has 655 and 529 observations on the principal closure branch.
This connects the allocation traffic to actual native execution rather than
inferring CPU cost from RSS or Limited outcomes.

Source inspection of
[`interpretation/outputs.rs`](../crates/zincite-lint/src/callable_definitions/interpretation/outputs.rs)
shows `literal_call_outputs::type_of` scanning expression rows with `.find`.
Its comparison constructs `file.location(node.range()).range` inside the row
predicate. That range is constant for one lookup, while location construction
also creates source-location data and computes line/column. The sampled
allocator and line/column stacks agree with repeated work in this predicate.
The samples do not separately measure scan length, allocation count at this
exact site, or the speedup from changing it.

Recommend one bounded follow-up: compute that requested physical range once
before the `.find` predicate, preserving exact file/range equality and the first
matching row. This affects the callable output/type lookup consumed by search
and unbounded-variable, and should preserve all supported, Unknown and Limited
outcomes, including the existing element limitation. Confidence is high in the
hot path and moderate in this particular improvement; its gain remains
unmeasured. Recheck the same root, diagnostics/partitions and native release
timings before considering a wider lookup change. No fix was initiated here.

Sampled thesis took 9.5611 wall / 8.5620 CPU s; sampled unbounded-only took
7.4566 / 7.1079 s. The former has more wall delay than unsampled thesis, while
the latter used less CPU than its allocation-counting companion. Profiler
perturbation, counter overhead and external contention cannot be separated by
these observations. The approximately 1.3-second default/remaining work is not
fully attributed by these late windows. This report explains the dominant
expansion without claiming a complete per-function accounting.

## Retained evidence

Complete private captures remain untracked under `/tmp/zincite139-20261015/`:
`report.json` is the single budget/deadline/reap and identity ledger;
`reconciliation.json`, all named `.stdout`/`.stderr`, and both `.sample.txt`
files retain outcomes and actual stacks. Temporary controllers are
`/tmp/zincite139-measure.py`, `/tmp/zincite139-sampled.py` and
`/tmp/zincite139-reconcile.py`; they reuse `scripts/bench-save.py::file_invoke`
and the existing `/tmp/zincite122-waitid.dylib` bridge. The bridge keeps PIDs
reserved with WNOWAIT until the sole `wait4` reaper. The sampled-child controller
uses the same finite deadline and reap arrangement.

The temporary demand source/binary are `/tmp/zincite139-demand.rs` and
`/tmp/zincite139-demand`, compiled with `rustc --edition=2024 -O -C debuginfo=1`
against the matched release `libzincite_lint-11ce169f5d12d88c.rlib` and dependency
directory. Its source and the exact commands remain available locally.
Only this report and its link were changed; no new tests, dependencies,
production instrumentation, source/data rewrite or semantic repair was added.
