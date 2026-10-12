+++
schema_version = 1
id = "base-127"
key = "literal-data-mapping"
area = "base"
status = "open"
complexity = "advanced"
priority = "low"
blocked_by = ["base-125", "base-126"]
+++
# Create and replace structured data using bounded comprehensions

## Outcome

Shared comprehension syntax constructs new data and atomically replaces existing structured instance data through the library and both query commands.

## Context

Read the contract produced by literal-mapping-contract and obtain explicit approval for its grammar, operation subset and comment/key policy before editing. This attended gate resolves those choices; existing set_value source insertion remains separate. The user also requires creation of data and a double-instance example, not only element filtering or numeric scaling. Use the shared bounded comprehension contract for reports and data; do not create a separate data evaluator. Begin with structured.rs, language.rs, lib.rs, CLI edit handling and established query tests.

For human-facing documentation and website prose, apply unslop (/Users/zayenz/.agents/skills/unslop/SKILL.md) and zayenz-writing-style (/Users/zayenz/.agents/skills/zayenz-writing-style/SKILL.md). Read their applicable guidance; use the professional-informal/code-documentation register for explanatory docs. Preserve exact commands, code, schemas, quotations and machine-oriented reference text.

The user confirmed that doubling means duplicating entities and their dependent data with explicit index/reference mapping. Reuse the bounded comprehension implementation delivered by read-only-query-programs.

## Boundaries

- Implement only the approved finite literal-data subset. No arbitrary parameter/model evaluation, inferred types, callbacks or general MiniZinc interpreter.
- Retain atomic edit candidates, original source ranges, reparsing, preview/write distinction and refusal of incomplete writes.
- Dependent data is created or remapped only through explicit comprehensions in the transformation; do not guess how arbitrary model references should change.

## Done when

- [ ] The exact approved shared contract is recorded before implementation, including any remaining syntax or policy choice at this attended gate.
- [ ] Library and both CLI forms use comprehensions to create new assignments or a data document and replace existing array/matrix/record values, including coherent multi-assignment candidates.
- [ ] One public structured-instance example doubles its entity collection and explicitly regenerates its dependent indexed or nested data with consistent identities and shape; multiplying values alone does not satisfy this example.
- [ ] The implementation reuses shared report-comprehension semantics, preserves supported keys and rectangular shapes, and leaves unrelated original source bytes intact.
- [ ] Unsupported expressions, arithmetic failures, uncertain shape/key/reference identity, conflicting edit targets and unresolved comment placement cause located refusal with no partial write.
- [ ] Documentation demonstrates reports, new data, replacements and instance expansion separately; generated candidates are reparsed and incomplete candidates cannot be written.

## Validation

- Use the shared report/data comprehension controls plus one compact self-contained doubled structured instance. Check indexed/nested consistency, command/library parity, candidate reparsing, retained unrelated bytes and failed multi-assignment write atomicity. No corpus sweep or solver run.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace when Rust changes; always run git diff --check and zdev check base --format json for changed records.
- Review the final human-facing prose against both writing skills: concrete openings, proportionate claims, direct explanations and Zayenz voice; remove generic or promotional wording without flattening useful context.
