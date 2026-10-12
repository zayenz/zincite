+++
schema_version = 1
id = "base-114"
key = "semantic-opacity-contract"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Adopt deliberate semantic opacity and the retained proof contract

## Outcome

Zincite has one actionable semantic support contract that permits deliberate opacity, retains valuable facts and proofs, and authorizes the simplifications in this bundle.

## Context

The user approved the architectural review on 2026-10-12: keep structural types, identities, bounds and selected proofs; stop interpreting arbitrary bodies merely to turn Unsupported into Unknown; introduce demand-driven reuse before considering Salsa. Existing docs/semantic-processing-plan.md and docs/callable-analysis-plan.md preserve every successful inspector and distinguish ordinary uncertainty from exclusions. Those rules need an explicit revision before implementation. Base-083, base-093 through base-100, and base-050 are completed historical work, not tasks to reopen. Start with the brief's semantic and acceptance sections, both plans, background/thesis-rule-coverage.md, and the source entry points analysis.rs, callables.rs, domains.rs, guarded.rs, definitions.rs and search.rs in crates/zincite-lint/src.

## Boundaries

- Documentation and planning only. Preserve completed task results, historical captures, current task selection and unrelated implementation work. Do not change semantic code or relabel historical results.
- Use the existing brief as product authority and the semantic plan for detailed contracts; update or mark superseded passages in the callable plan instead of adding a competing architecture document.
- Retain complete target syntax, source preservation and the full rule catalogue. Do not make zero limitations, full inference, a new IR, a general fixed-point framework, or Salsa adoption completion requirements.

## Done when

- [ ] The brief and semantic plan distinguish structural source facts, local abstract facts, optional callable proofs, and lint/fix policy. For each retained family, identify its current owner, named consumers, meaningful guarantees and intentional precision limit in a compact table.
- [ ] Define deliberate opacity as a supported conservative operation: a selected signature may establish types and identity while its body contributes no bounds, truth, definedness, output or search proof. Preserve argument/reference facts, known relevant errors and possible assertion/partiality barriers. Written user-body references and required local/signature typing remain inspected.
- [ ] Specify outcomes: Completed means the selected bounded analysis ran, possibly with ordinary Unknown; promised support defects and unavailable prerequisites remain Limited/NotRun or errors as appropriate; resource cutoffs are never Unknown. Document exactly which former admission-only exclusions become supported opacity, leaving other exclusions explicit.
- [ ] State the warning contract: inability to prove a bound/definition/use absence cannot alone justify an unbounded/search/unused warning. Preserve scoped explicit-domain advice on simple supported cases; uncertain relevant relations veto dependent advice. Unknown cannot enable a semantic query match through negation or a safe fix.
- [ ] Retain compositional arithmetic/interval, optionality, relational partiality, index identity, direct enforced definition and finite summary guarantees. Define retained callable summaries by semantic operation, including existing useful wrappers and search views, rather than promising every historical source shape. Retiring admission-only inspection is authorized; losing a useful positive proof requires explicit review rather than silently changing its expectation.
- [ ] Replace the blanket incremental-database rejection with a conditional Salsa decision: ordinary per-analysis demand and reuse are the implementation target; production Salsa adoption is outside this bundle and needs measured justification and a separately scoped decision.
- [ ] Record a small baseline selection using existing public tests plus the named semantic originals from the latest acceptance record: useful positive findings, uncertainty vetoes, root/rule outcomes, and source preservation. Separate semantic expectation changes from scheduling-only changes. Record the current baseline revision; old capture paths are optional evidence, not hidden prerequisites.

## Validation

- Check every superseded statement about preserving inspectors, source admission, rule completion and caching in the brief and both plans; leave historical task results intact.
- No new tests. Inspect named existing public controls and current acceptance records to ensure the documented guarantees have concrete witnesses.
- Run zdev check base --format json and git diff --check.
