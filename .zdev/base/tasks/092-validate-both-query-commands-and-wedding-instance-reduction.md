+++
schema_version = 1
id = "base-092"
key = "query-reduction-acceptance"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "low"
blocked_by = ["base-089", "base-091", "base-100", "base-101"]
+++
# Validate both query commands and wedding-instance reduction on representative data

## Outcome

The initial query subsystem has documented recipes and measured, source-preserving best-effort previews of the two known wedding instances, a separate complete structured-reduction/write-path control, and working normal-library semantic navigation through both public command forms.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), References and focused acceptance, for exact local model/data paths and manual feasibility controls. Reuse current scripts/README.md performance/corpus tooling selectively and the query command's public API/CLI. Manual 12-to-8 and 150-to-110 reductions established feasibility, not implementation coverage.

## Boundaries

- External originals stay read-only/private; temporary reduced files are not copied into committed fixtures. No solver runs, broad corpus/test matrix or benchmark platform.
- The existing base-050 corpus/thesis gate remains separate. Compiler acceptance alone cannot establish the requested reduction, and resource/unsupported gaps remain explicit. Under docs/semantic-processing-plan.md, accept the two named real reductions as incomplete best-effort previews while computed dependencies remain unresolved; retain status 1, located unresolved dependencies and refusal of in-place writes. General computed enum-index expression evaluation is excluded. The installed-library navigation resource failure remains a required base-100 repair; a cutoff is not a semantic exclusion. Policy adoption does not complete this task.

## Done when

- [ ] Documented recipes exercise item filtering, nested inspection, explicit assignment edits, structured filters, enum reduction and semantic queries through both standalone and umbrella forms.
- [ ] Actual reductions of test_data/small.dzn and the named 150-guest mixed instance retain selected enum/record coverage, remove supported set references/empty groups, preserve expected ordering/unrelated data and report remaining dependencies honestly.
- [ ] Originals and the two incomplete best-effort previews pass MiniZinc --instance-check-only with recorded version/settings, plus direct retained/removed structured-data checks; previews retain incomplete status, unresolved dependencies and write refusal. A separate small complete structured-reduction control passes compiler/direct data checks through both command forms and exercises the complete write path. Local originals remain byte-identical and real reduced outputs stay temporary.
- [ ] Normal installed-library declaration/use/type navigation succeeds through both command forms with finite model-work and result/expansion bounds after base-100; resource failures remain explicit and cannot be waived as semantic exclusions.
- [ ] A few representative/growing data measurements report command latency/memory and scaling; confirmed unexplained growth is resolved or recorded as an acceptance gap. Recheck save behavior only if shared syntax/layout changed.
- [ ] The guide and acceptance summary state implemented stages, source-preservation behavior and best-effort limits without claiming full MiniZinc evaluation or complete semantic coverage.

## Validation

- Reuse the focused public tests introduced by preceding tasks; add an end-to-end regression only for a demonstrated integration failure.
- Run both command forms on the exact local model/data pairs, directly check retained/removed structured data and run MiniZinc instance checks on originals/candidates without solving. Record missing evidence rather than counting it as passed.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Checkpoint

Paired command recipes, actual private-instance reductions, direct source/data checks, compiler acceptance and fresh-process latency/RSS measurements are recorded in [the acceptance report](../../../scripts/query-acceptance.md). Independent whole-task review and required Cargo checks passed for that evidence at snapshot Wa27405e5c51666ef.

The task remains open: both actual reductions return status 1 with four computed enum-indexed accesses unresolved in the read-only model. MiniZinc accepts the originals and candidates, but those candidates remain incomplete. The adopted contract accepts them as best-effort previews while preserving unresolved reporting and write refusal; the separate complete structured-reduction/write-path control and normal-library navigation still require acceptance evidence. Full installed-library semantic inspection also reaches the documented collection limit. No dependency-analysis repair or corpus sweep was launched. Private originals remain byte-identical, and candidates/raw evidence remain temporary under /tmp/zincite092/.
