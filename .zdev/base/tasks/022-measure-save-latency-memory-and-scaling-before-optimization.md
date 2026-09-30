+++
schema_version = 1
id = "base-022"
key = "performance-baseline"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "high"
blocked_by = []
+++
# Measure save latency, memory and scaling before optimization

## Outcome

A small repeatable release benchmark measures the real format-on-save command and establishes actionable CPU/memory evidence.

## Context

Read brief Performance and format-on-save and ../background/performance.md, including the Ruff, uv and ty references. The exploratory release probe found about 267 ms and 143 MiB RSS for a 966,669-byte dense array, while small real models were much faster. The first invocation was also unusually slow. No durable benchmark currently exists; use the public libraries and existing stdin CLI, with the installed benchmark-script skill guidance for a small driver.

## Boundaries

- Keep this a local benchmark, not a framework or campaign. No runtime optimization in this task; no mandatory profiling service, downloaded private data or performance tests that freeze type sizes.

## Done when

- [x] Create a self-contained benchmark driver and small explicit case set following the brief: representative private/external paths or synthetic inputs, changed/already-formatted/invalid buffers, dense data, matrices and nested expressions; retain sizes and first-use versus repeated-run labels.
- [x] Measure fresh-process save latency including configuration and output capture, at least 50 samples per small case for p50/p95, and separate peak child RSS with correct platform units. Compare default and realistic EditorConfig lookup; report failures/timeouts rather than treating fast rejection as success.
- [x] Record a reproducible release baseline and profile the cases that miss budgets, separating lexer/parser, layout/fit work, buffers/configuration and diagnostics where useful. Attribute memory with allocation evidence when choosing representation changes; add bounded follow-ups for bottlenecks outside the planned scopes.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Verify one known valid and one malformed input through the driver, cross-check a timing/RSS reading with the native tool, and repeat enough cases to distinguish noise from a proposed improvement. Timing thresholds do not belong in cargo test.

## Result

Added a self-contained fresh-process save benchmark and developer-only allocation probe, with tracked case sizes, release p50/p95/RSS tables, CPU attribution and first-use evidence. Dense 966,669-byte input measured about 340 ms p95 and 141–143 MiB RSS; the marginal initial 96,669-byte latency miss passed on repetition. Existing base-046 and base-047 cover fit work and memory; first-execution waiting remains explicitly unresolved for base-048.

Validation:

- Primary and supplemental release measurements used 50 samples per case/settings variant; all expected statuses were accepted, including malformed-input rejection and unchanged already-formatted output.
- Native time cross-check measured dense input at 0.26 seconds real and 149635072 peak resident bytes; native CPU sampling and allocation counters attributed lexer, parsing, formatting and live-buffer costs.
- Fresh release first launch measured 280.77 ms wall with 3.78 ms child CPU, followed by 4.71 ms wall and 2.96 ms CPU; exact OS waiting cause remains unestablished.
- Valid/malformed smoke and forced-timeout reporting checks passed; Python compilation, cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace (53 tests) and git diff --check passed.
