+++
schema_version = 1
id = "base-142"
key = "complete-wedding-reductions"
area = "base"
status = "open"
complexity = "standard"
afk = true
blocked_by = []
+++
# Prove nested guest-group membership in wedding reductions

## Outcome

The two named wedding instance reductions complete safely, including guest-indexed model accesses through nested literal guest-group iteration.

## Context

The user selected the wedding cases for a dependable reduction workflow on 2026-10-17. Read tasks/092-validate-both-query-commands-and-wedding-instance-reduction.md, scripts/query-acceptance.md, crates/zincite-query/src/reduction.rs and existing reduction tests. Both retained private instances under /Users/zayenz/minizinc/models/misc/seating return four unresolved accesses at seating.mzn lines136/141: table_of[g1] and table_of[g2] inside forall(group in same_table/different_tables)(forall(g1,g2 in group where g1<g2)(...)). The reducer already prunes literal group arrays but currently proves only generators directly over Guests. The earlier acceptance deliberately allowed these incomplete previews; this task authorizes a narrow additional proof, not general MiniZinc interpretation.

## Boundaries

- Preserve current refusal for arithmetic/computed/unproved selectors, unresolved dependencies, unrelated enum identity and incomplete writes. Recognize guest membership only when justified by the actual supported group sources and lexical binding identity; names alone are insufficient.
- Read private model/data only; all candidates go to temporary files. Do not commit private source or instance data. Public regression fixtures must be small and anonymized. No solving, general computed enum evaluation, automated delta debugging or new Zirql behavior.

## Done when

- [ ] Nested iteration over supported literal guest-group arrays supplies a sound membership proof for selectors in the selected model; ordinary unrelated/unproved computed selectors still leave incomplete outcomes and refuse writes.
- [ ] The small12-to8 and mixed150-to110 wedding reductions complete, retain intended enum/record keys and pruned groups, preserve unrelated data and original bytes outside edits, and permit writing only complete candidates to temporary copies.
- [ ] Focused public-behavior regression coverage demonstrates this nested proof and one nearby unproved/refused case. Public docs explain the supported case and retained limits without publishing private data.

## Validation

- Run focused query reduction tests and cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace.
- Exercise both zincite query and zincite-query on the two named private cases using the kept sets already in scripts/query-acceptance.md; compare direct retained/removed data and group references, run MiniZinc --instance-check-only on originals and candidates with the existing installed stdlib/Gecode include setup, and confirm original files unchanged. No solver run.
- Run git diff --check and zdev check base --format json.
