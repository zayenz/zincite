+++
schema_version = 1
id = "base-138"
key = "representative-lint-runtime"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-137"]
+++
# Measure current lint runtime on a small varied model set

## Outcome

Current runtime and incompleteness are known for ordinary and difficult models without replaying every instance.

## Context

The assessment measured78/176ms only on tiny two/ten-rule cases; older hard-model and full-corpus timings predate semantic simplification. Read scripts/batch-performance.md, semantic-corpus-checkpoint.md and existing profiling drivers. Use the new default plus thesis/all, with the current release and explicit library setup.

## Boundaries

- No corpus sweep, solving, semantic repair, instance Cartesian product or infrastructure optimization in this measurement task. Measurements must not overlap other performance runs.

## Done when

- [ ] Select and explain four to six distinct model roots covering ordinary, shared-library, comprehension-heavy and previously difficult callable/search structure; use one representative instance only where needed.
- [ ] Report revision, command, selected rules, repeated fresh-process elapsed times, peak child RSS, diagnostics and Completed/Limited/error outcomes; report cutoffs honestly.
- [ ] Pre-check originals with the installed MiniZinc model-check or compile-only Gecode path as appropriate; classify rejected or unavailable inputs separately.
- [ ] Record bounded conclusions and any concrete follow-up recommendation; distinguish old observations from current results.

## Validation

- Run the existing driver sequentially with three repetitions and a60-second per-process cap, at most18minutes total timed runtime across selected roots/presets; stop repeated timed-out cases after the first cutoff.
- Retain originals unchanged and raw private measurements untracked. No new benchmark framework or timing assertions in tests.
- Run zdev check base --format json.
