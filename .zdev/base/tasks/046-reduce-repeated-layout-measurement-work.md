+++
schema_version = 1
id = "base-046"
key = "format-fit-performance"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-022", "base-027"]
+++
# Reduce repeated layout measurement work

## Outcome

Formatting avoids measured redundant width/fit work and improves save latency without changing the agreed layout.

## Context

Read brief Performance and format-on-save and ../background/performance.md, including the Ruff, uv and ty references. Start with Formatter::preview, node_exceeds_width, prefix_exceeds_width, current_columns and matrix cell previews in zincite-fmt/src/lib.rs. They render temporary strings or rescan text. Use the baseline profile to identify the relevant cost; preserve the preceding stability fixes.

## Boundaries

- No broad printer/framework rewrite unless the measured bounded fix cannot be expressed simply; a shared document IR or memoization system is not a prerequisite.

## Done when

- [ ] Use the smallest justified change to width/fit measurement, repeated subtree visits or preview buffers in this bounded path, with before/after release evidence on the affected shapes.
- [ ] Preserve actual prefix/suffix widths, tabs/Unicode counting, mandatory and explicitly expanded layouts, comments and protected spans; do not replace one redundant walk with an unbounded cache.
- [ ] Report p50/p95, peak RSS and size-scaling results, and retain no measurable regressions on the unaffected case set. If profiling shows this path is not a bottleneck and the applicable budgets already pass, record that evidence instead of making speculative changes; other bottlenecks remain follow-ups.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run the small performance set and existing formatter/corpus preservation and idempotence checks. Add a minimized behavior regression only when the change fixes an actual defect; use benchmark comparisons for speed proof.
