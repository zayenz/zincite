+++
schema_version = 1
id = "base-049"
key = "batch-performance"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-022", "base-029", "base-045"]
+++
# Measure batch and semantic-lint speed and memory

## Outcome

A full-batch performance report identifies measured throughput and memory problems without hiding incomplete analysis.

## Context

Read brief Performance and format-on-save and ../background/performance.md, including the Ruff, uv and ty references. Use the finished recursive commands and selected-analysis results. Ruff motivates fast uncached whole-project work; uv distinguishes cache conditions; ty motivates sharing only the facts needed by selected rules. The formatter save path stays independent of semantic analysis.

## Boundaries

- This is a measurement and profiling checkpoint. Unproven optimizations and an unbounded semantic-engine tuning campaign are outside scope.

## Done when

- [x] Extend the small driver to measure total wall time/throughput and peak RSS for formatter checks and default/thesis/all lint runs over known roots, including shared include closures and many-warning cases. Report input bytes/counts and analysis coverage alongside speed.
- [x] Check memory across increasing counts of independent roots and shared dependencies, distinguish useful within-invocation reuse from retained unused work, and profile material superlinear growth or repeated configuration/diagnostic work.
- [x] Produce bounded optimization follow-ups for confirmed costs, with before/after proof and a target metric, as blockers of final acceptance where they violate the brief. Do not silently add parallelism, persistent caches or incremental infrastructure.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run a bounded root subset before the full batch, verify warning/error/limited-analysis outcomes remain visible, and compare repeated release runs. No broad new test suite is expected for reporting.

## Result

Completed the approved batch measurement/profiling checkpoint with current native coverage and finite cutoffs preserved; two confirmed CPU costs need bounded repairs before final acceptance. Production APIs/CLI and existing075/076/077 scope remain unchanged.

Validation:

- Release build and cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace pass; commands, statuses, hashes and logs are indexed in /private/tmp/zincite-base049-validation.json.
- Bounded native/probe default/thesis/all streams and statuses match for shared/missing includes, data, rejected syntax, BOM/UTF8 and a public many-warning root; canonical repeat/symlink dedup and distinct-file scaling are checked. Evidence: target/benchmarks/base049/bounded-reconciliation.json and growth-summary.json.
- Two fresh native full runs per selection over all 6417 discovered roots / 641084859 bytes are retained. Default finishes in 146.083/162.258 seconds at status2; formatter/thesis/all both reach finite 300-second deadlines with unknown completed counts/throughput. Six failures are preserved, not replaced by a passing subset. Evidence: target/benchmarks/base049/full/report.json and final-reconciliation.json.
- Current full default probe matches native stderr/status and observes all6417 roots; semantic companions each observe517 with5900 explicit Unobserved roots per rule, zero all-selected-completed models and visible errors/limitations. All reported root scopes drop to exact baseline.
- Native CPU/growth and separate sampled intervals confirm SourceLocation prefix scans and matrix width/row classification. Public wide and synthetic30000 fidelity pass; three public-wide parse/format/lint scopes release to baseline. Failed profiler attachments and censored extended profile are retained under target/benchmarks/base049/profiles/.
- All7439 inventoried originals and measured binaries remain unchanged. Two observed .mzn.model dependencies lack pre-full-campaign hashes; a subsequent two-parent native/probe replay and before/after hashes match without claiming historical repair. Six foreign files remain exact bytes/modes and unstaged; index is empty. Evidence: final-pin.json, bacp-source-hashes.json, ownership.json.
- The dedicated report is scripts/batch-performance.md; final validation index is /private/tmp/zincite-base049-validation.json. Exactly three attributable paths are frozen for independent whole-task verification. Existing075/076/077 and proposed cost repairs remain base050 acceptance blockers; no semantic acceptance or save-budget relaxation is claimed.
