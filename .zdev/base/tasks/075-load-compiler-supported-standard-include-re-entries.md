+++
schema_version = 1
id = "base-075"
key = "semantic-include-closures"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Load compiler-supported standard include re-entries

## Outcome

Zincite loads the compiler-supported standard include closures seen in the semantic checkpoint without false include-cycle errors.

## Context

Read brief Corpus and thesis expansion and scripts/semantic-corpus-checkpoint.md. MiniZinc 2.10.1 accepts the small all_different include control, but load_model rejects repeated edges through all_different/global_cardinality. Raw accepted compiler and loader evidence is retained under target/corpus/base045/probes. Start from ModelContext canonical identities and retained include edges. The pinned compiler rejects the separate self-include smoke; keep that genuine user-cycle diagnostic. See zincite-base045-root-self-include-compiler.json.

## Boundaries

- Repair the observed standard-library closure re-entry family, not the semantic policies or a general dependency platform. Preserve genuine user-cycle errors, file identities, include search order, source ranges and unavailable/malformed dependency diagnostics. Do not edit installed libraries or external models.

## Done when

- [ ] Resolve the compiler-supported all_different and globals closure reductions without false include-cycle errors, preserving canonical file deduplication and the correct explicit/implicit source classification.
- [ ] Preserve the compiler-rejected self-include control and bounded user-cycle errors while accepting the demonstrated compiler-supported standard closure re-entries. Do not remove cycle detection globally or suppress errors by filename spelling.
- [ ] Reconcile selected thesis/all loading outcomes on the original public controls and an affected corpus root; retain any independent semantic limitation and leave corpus acceptance blocked by unresolved families.

## Validation

- Run required workspace fmt/clippy/tests and focused public loader checks.
- Run pinned MiniZinc 2.10.1 compiler-only controls and selected-analysis replays with unchanged original hashes; no solver or full-corpus campaign is needed for this repair.
