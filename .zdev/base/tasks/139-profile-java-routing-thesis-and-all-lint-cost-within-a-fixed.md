+++
schema_version = 1
id = "base-139"
key = "java-routing-profile"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-138"]
+++
# Profile java-routing thesis and all lint cost within a fixed budget

## Outcome

Explain the observed java-routing thesis/all cost with measured phase, rule and CPU-stack evidence, and recommend one bounded next step without changing semantics.

## Context

scripts/lint-performance-current.md at d2934e8 measured Challenge 2021 java-routing/trip_7_4.mzn (85,229 bytes): default 1.37–1.41 s, thesis/all 8.41–8.75 s, with search-coverage Limited in installed element.mzn. A limitation is not itself a cost attribution. Use exactly /Users/zayenz/minizinc/challenge-models/2021/problem-set/mznc2021_probs/java-routing/trip_7_4.mzn and its normal include closure, matching base-138's isolated settings and explicit installed stdlib. Begin with the current report, scripts/bench-save.py, crates/zincite-fmt/examples/profile-phases.rs, and selected analysis/search consumers. Read the brief's Performance and Testing sections and linked performance background.

## Boundaries

- Profile only this unchanged original root and its required includes. No corpus sweep, solver run, semantic repair, optimization, changed analysis expectations or source/data rewriting.
- Durable changes are a concise scripts/java-routing-profile.md and a link from scripts/lint-performance-current.md. Reuse existing drivers/probes; temporary local probes or symbol-enabled builds are allowed, but no production instrumentation, new dependency, permanent profiling framework or new tests.
- At most 18 target lint/probe launches, each capped at 60 seconds, and 600 seconds aggregate target-process wall time. At most three CPU sampling attachments of at most five seconds each, only to task-owned children. Stop repeats of a timed-out case; report cutoff, denied profiler access or exhausted budget honestly rather than expanding the task.

## Done when

- [ ] A fresh matched release baseline for default, thesis and all has up to three repetitions per selection, with exact revision/build, input/include setup, commands, elapsed times, diagnostics and selected-rule outcomes. Reuse the original MiniZinc precheck only if its inputs/configuration still match; otherwise perform one bounded model-only check.
- [ ] Bounded selected-rule comparisons and existing phase/allocation probes distinguish loading/signature preparation, demanded semantic analysis and diagnostic rendering where measurable. Test the search-coverage hypothesis against measured alternatives rather than assuming it.
- [ ] CPU samples or an explicitly documented bounded fallback identify the dominant functions/call path and explain the near-nine-second observation as far as the evidence supports. Separate profiler overhead, timing differences, allocation traffic, RSS and Limited outcomes; leave unresolved attribution explicit.
- [ ] The report gives a concrete next-step recommendation with its affected consumer/contract and confidence, or explains why the bounded evidence is insufficient. Originals, dependencies and production source remain unchanged; raw captures and temporary probes stay untracked.

## Validation

- Run serially in a quiet window; retain process deadlines/reap receipts and a single budget ledger. Validate reported quantities and comparisons against complete raw captures and actual profiler stacks.
- Check unchanged input/binary identity and the diagnostic/completeness effect of changing selected rules. No new tests or full Rust validation are required for a report-only deliverable; independently review the report and evidence.
- Run zdev check base --format json after managed record changes.
