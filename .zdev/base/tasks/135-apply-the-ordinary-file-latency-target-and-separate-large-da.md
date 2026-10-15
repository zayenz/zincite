+++
schema_version = 1
id = "base-135"
key = "ordinary-save-budget"
area = "base"
status = "done"
complexity = "routine"
afk = true
priority = "normal"
blocked_by = []
+++
# Apply the ordinary-file latency target and separate large-data reporting

## Outcome

Current performance documentation and benchmark classification apply 100 ms only to ordinary editing workloads.

## Context

The user clarified on2026-10-15 that100ms targets ordinary files, not megabytes of data. The brief Performance section now owns this correction. scripts/save-performance-current.md still calls the966,669-byte dense111ms result an acceptance failure. Read scripts/bench-save.py, save reports and user documentation; preserve dated observations.

## Done when

- [x] Name a small ordinary-file case set and distinguish dense/generated data and synthetic stress cases without a new universal size threshold.
- [x] Current summaries apply100ms p95 to ordinary cases; large data reports throughput, RSS and growth separately. Historical measurements and former criteria remain accurately dated.
- [x] No large-data optimization task is required solely because the old100ms criterion was exceeded.

## Validation

- Check all current performance claims and case classifications against the revised brief. Existing results may be reused with their dates and scope; no fresh timings or new tests are required for documentation-only work.
- Run zdev check base --format json.

## Result

Applied the ordinary-file latency target and separated dense data and stress reporting while preserving dated observations.

Validation:

- Independent review passed; zdev check base and staged diff check passed. Documentation-only change reused retained measurements.
