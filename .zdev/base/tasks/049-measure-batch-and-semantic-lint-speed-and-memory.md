+++
schema_version = 1
id = "base-049"
key = "batch-performance"
area = "base"
status = "open"
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

- [ ] Extend the small driver to measure total wall time/throughput and peak RSS for formatter checks and default/thesis/all lint runs over known roots, including shared include closures and many-warning cases. Report input bytes/counts and analysis coverage alongside speed.
- [ ] Check memory across increasing counts of independent roots and shared dependencies, distinguish useful within-invocation reuse from retained unused work, and profile material superlinear growth or repeated configuration/diagnostic work.
- [ ] Produce bounded optimization follow-ups for confirmed costs, with before/after proof and a target metric, as blockers of final acceptance where they violate the brief. Do not silently add parallelism, persistent caches or incremental infrastructure.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run a bounded root subset before the full batch, verify warning/error/limited-analysis outcomes remain visible, and compare repeated release runs. No broad new test suite is expected for reporting.
