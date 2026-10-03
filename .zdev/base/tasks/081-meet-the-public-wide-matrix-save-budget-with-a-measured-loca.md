+++
schema_version = 1
id = "base-081"
key = "wide-save-budget"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-080"]
+++
# Meet the public wide-matrix save budget with a measured local repair

## Outcome

The retained 885733-byte public wide matrix meets the unchanged 100 ms p95 and 64 MiB fresh-save budgets while preserving exact source and matrix layout behavior.

## Context

Read brief Performance and format-on-save, scripts/save-performance.md and the base-079 matrix checkpoint in scripts/batch-performance.md. Row classification now meets its local native targets, but 50 fresh stdin saves give default/nested p95 380.463250/382.080125 ms and native child RSS 106.296875/105.125 MiB for 2019/groupsplitter/u7g2pref1.dzn. Separate phase evidence retains about 6.25 million format allocation calls, 198862238 requested bytes and 15857904 peak bytes above the retained CST. These counters are not native RSS or proof of its whole cause. Start with existing matrix cell rendering/layout, measure CPU and allocation attribution separately, then keep only demonstrated local improvements.

## Boundaries

- Keep the parser/CST APIs, protected bytes, indexed/unindexed rows, column indices, nested cells, comments, spelling, alignment, indentation, shared width breaks and stability unchanged.
- No new crate, unsafe packing, arena, layout engine, persistent cache, daemon, parallelism or LSP. Do not broaden into unrelated formatter tuning. Additional source ownership, especially syntax-owned siblings, requires coordinator review.
- Initial task-owned path allocation: ["crates/zincite-fmt/src/lib.rs","crates/zincite-fmt/src/layout.rs","scripts/save-performance.md","docs/format-on-save.md"]. Coordination may extend it only after checking retained parent edits and sibling assignments.

## Done when

- [ ] Matched before/after release measurements retain byte-identical complete output and status on the public wide shape and existing matrix/table controls.
- [ ] At least 50 fresh saves per default/nested configuration meet 100 ms p95 and 64 MiB on the public wide input; existing small/dense representative cases retain their per-case budgets and hard cases are not omitted.
- [ ] Separate profiler/allocation/native-RSS evidence identifies the local change and its limits; material first-use observations remain separate with measured attribution or explicit external limitation.
- [ ] Public wide and established small/large table fidelity, protected-byte and exact scope-drop controls pass; formatting leaves ParsedFile usable. Any remaining actual miss stays an acceptance blocker with a bounded follow-up.

## Validation

- Reuse the existing 50-sample stdin command protocol with complete stdout/stderr/status, unchanged input/configuration pins and fresh processes; native own-child RSS runs separately.
- Run matched native checks and adequate finite clean parse/reparse, token/tree coverage, spelling/structure/protected-byte and idempotence controls. Recheck the full no-rules corpus after layout changes.
- Run required workspace fmt/clippy/tests for Rust changes; use existing behavior checks and no timing/layout-size tests. Keep every failed/noisy/censored attempt and external original unchanged.
