# Performance and format-on-save

The user wants both speed and low memory use, with formatting on save feeling
unremarkable. This is part of acceptance, not a later optional cleanup. The brief
owns the performance budgets; this note records research and starting evidence.

## Lessons from Ruff, uv and ty

- **Ruff:** its formatter measures uncached operation on real projects and shares
  parser infrastructure with the linter. Its developer guide distinguishes
  single-file microbenchmarks, whole-project benchmarks and profiling. Apply that
  distinction to Zincite: measure the save path, corpus throughput and specific
  hot functions separately. Python benchmark ratios are not Zincite targets.
  Sources: [formatter launch](https://astral.sh/blog/the-ruff-formatter),
  [contributing guide](https://github.com/astral-sh/ruff/blob/main/CONTRIBUTING.md).
- **Ruff's printer:** its source separates document construction from printing and
  exposes source slices and retained verbatim ranges. That is useful inspiration
  when replacing costly preview strings with bounded fit calculations in Zincite.
  Keep Zincite's layout contract; a generic document IR is only justified if a
  measured local change needs it. Source:
  [formatter infrastructure](https://github.com/astral-sh/ruff/blob/main/crates/ruff_formatter/src/lib.rs).
- **uv:** its benchmark documentation separates cold and warm scenarios and states
  filesystem/workload caveats. Its cache documentation makes dependency-sensitive
  invalidation explicit. Apply this to measurement labels and configuration/include
  reuse: measure misses as well as hits, and keep invalidation correct. Download
  parallelism, hardlinks and a persistent package cache are not direct solutions
  to formatting one edited buffer. Sources:
  [benchmarks](https://github.com/astral-sh/uv/blob/main/BENCHMARKS.md),
  [cache semantics](https://docs.astral.sh/uv/concepts/cache/).
- **ty:** its language server recomputes affected definitions and can avoid
  irrelevant third-party work. Its benchmark suite distinguishes first diagnostics
  from rechecks after an edit. Apply the principle of doing only needed work:
  formatting needs syntax, not include resolution or semantic lints; selected
  rules should share necessary facts within an invocation. A fresh CLI process has
  no previous query state, so adopting Salsa or a persistent server is not implied.
  Sources: [incremental analysis](https://docs.astral.sh/ty/features/language-server/#fine-grained-incrementality),
  [benchmark guide](https://github.com/astral-sh/ruff/blob/main/scripts/ty_benchmark/README.md).

These are design references, not dependencies, code to copy, or instructions for
this repository. Start with measured work avoidance and storage improvements.

## Exploratory local probe

2026-09-30; source revision `c5aaf97`; Rust 1.98.1; release workspace build;
Apple M1 Max, arm64, 64 GiB RAM. Source code was unchanged. Fresh formatter
processes received source on stdin with `--stdin-filepath`, and stdout was fully
captured. The timer surrounded subprocess creation through completion, including
configuration lookup. Seven sequential samples per case were taken without
controlling machine load. Peak formatter RSS was measured in a separate invocation
using macOS `/usr/bin/time -l`, whose RSS is reported in bytes. These are exploratory
medians, not statistically established p95 or cold-filesystem measurements.

| Case | Input bytes | Median wall ms | Peak RSS MiB |
| --- | ---: | ---: | ---: |
| assignment-pool base model | 2,655 | 4.86 | 2.11 |
| Challenge ATSP | 6,899 | 5.44 | 2.53 |
| Challenge java-routing | 85,229 | 21.50 | 10.38 |
| Synthetic flat data array | 9,669 | 6.91 | 3.34 |
| Synthetic flat data array | 96,669 | 30.57 | 17.27 |
| Synthetic flat data array | 966,669 | 267.10 | 142.75 |

The first invocation of the newly built binary took 303.95 ms for the small model;
subsequent samples were much faster. Its cause was not established. A durable
benchmark must retain first-use observations separately from repeated save samples,
not silently discard them or label them cold-cache timings.

The synthetic data is `values=[0,1,...,99,0,...];`, generated with `n//3` entries
for n = 10,000, 100,000 and 1,000,000. It exercises token/CST density rather than
large strings. All measured invocations returned success, but this speed probe did
not establish preservation or idempotence. The earlier corpus checks remain the
correctness evidence. Personal `.mzn` files outside software/archive have a median
size of 2,655 bytes and a 95th-percentile sample near 85 KiB; the largest is about
673 KiB. Rebuild this small case set from the corpus inventory rather than relying
on private absolute paths in ordinary tests.

## Code paths to profile

- `zincite-fmt/src/lib.rs`: `preview`, `node_exceeds_width`,
  `prefix_exceeds_width`, `current_columns` and matrix layout render/copy text for
  measurements. These are candidates, not established dominant costs.
- `zincite-fmt/src/layout.rs`: the output is lexed again, then copied to another
  output buffer while source tokens and the CST remain live. Preserve protected
  literals/comments before considering a smaller working set or fewer passes.
- `zincite-syntax/src/lib.rs`: on this arm64 build, `SyntaxElement` and
  `SyntaxNode` are each 48 bytes and `Token` is 24 bytes. The recursive enum stores
  a node inline, so token-index leaves occupy the full element size. Profile
  retained allocations before choosing boxing, packed indices or another layout;
  no particular replacement representation is prescribed.
- CLI configuration lookup and lint diagnostic rendering are separate costs.
  Measure many-file and many-warning cases before adding caches or line indexes.
  Semantic analysis should reuse source and facts only for their needed lifetime;
  retaining every independent root to speed one later lookup can increase memory.

Use release builds, real model/data cases and a few synthetic size families.
Record p50/p95 latency, peak child RSS and full-batch wall time/throughput. Profile
CPU or allocation traffic only where it determines the next bounded change.
Keep timing checks out of normal unit tests and never trade source preservation
or deterministic results for a faster number.
