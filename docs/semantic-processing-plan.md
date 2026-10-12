# Bounded semantic processing

Zincite provides a standalone, lint-oriented semantic front end for MiniZinc
2.10.1. It retains complete target syntax and source-preserving processing
obligations while promising bounded semantic facts and detection families.
Valid syntax does not require unrestricted interpretation of the model.

The [brief](../.zdev/base/brief.md) is the product authority. The user-approved
base-114 revision adopts deliberate semantic opacity and the retained proof
contract. It supersedes the base-097 requirement to preserve every
successful source inspector. Base-083, base-093 through base-100 and base-050 are
completed historical work; this revision neither reopens them nor changes their
recorded outcomes. The new bundle changes future support expectations, not code
or historical execution counts.

## Research basis

Source research on 2026-10-11 supports separating reusable facts from individual
lint proofs. These comparisons guide design; they establish no Zincite runtime,
conformance, coverage or performance result.

| Reference | Observed boundary | Zincite decision |
| --- | --- | --- |
| Clippy | Its [driver](https://github.com/rust-lang/rust-clippy/blob/master/src/driver.rs) runs inside rustc; its [constant helper](https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/consts.rs) still folds only bounded expressions. These are dated moving-branch observations, without an exact Clippy revision. | Separate facts and lint policy. Do not invoke MiniZinc for ordinary linting or rebuild a compiler to obtain every fact. |
| Ruff | Its [semantic model](https://github.com/astral-sh/ruff/blob/a5aee680b11d2190fbb6a72008047ae281dd2d18/crates/ruff_python_semantic/src/model.rs) supplies scopes, bindings and references; its [small type helper](https://github.com/astral-sh/ruff/blob/a5aee680b11d2190fbb6a72008047ae281dd2d18/crates/ruff_python_semantic/src/analyze/type_inference.rs) admits Unknown for names and calls. Other rules have their own narrow helpers. | Reuse identity and type facts, with bounded analyses for named consumers. Ruff is the closest product comparison. |
| ty | Its [inference architecture](https://github.com/astral-sh/ruff/blob/a5aee680b11d2190fbb6a72008047ae281dd2d18/crates/ty_python_semantic/src/types/infer.rs) serves broad type checking and cross-file inference. | Borrow explicit fact/error boundaries. Do not adopt gradual Any as proof or Python flow semantics. See the conditional incremental-database decision below. |
| Shackle | [Signature and body queries](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/crates/shackle-hir/src/typecheck/mod.rs) precede separate [compiler transformations](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/crates/shackle-thir/src/transform/mod.rs). | Use structural types and the signature/body distinction as design references. Do not copy code, add its crates or import its include policy. |

The Astral and Shackle links pin the inspected development revisions. Shackle's
front end is useful evidence for these boundaries, not a verified MiniZinc
2.10.1 compatibility baseline.

## Supported core and separate proof owners

Keep the existing CST, `ModelContext`, `TypeInst`, `CallableFacts` and rule outcomes.
Use simple modules and direct traversal in `zincite-lint`; add no semantic crate,
parallel type hierarchy or general certainty framework.

The common core provides scoped declaration/reference identity, sequential local
and generator visibility, aliases, include/source ownership, common structural
types, nominal enum identity, `par`/`var`, optionality and supported first-order
overload/coercion/substitution facts. Structural types cover scalars, sets,
arrays with rank/index/element types, tuples and records. Promise their documented
common combinations, not every inference problem expressible with those types.
An index type does not establish an index set or extent. A nominal enum type
does not establish membership of a computed value.

Calls retain the uniquely selected declaration and instantiated formal/result
types. Unknown candidates that could change selection veto definite selection;
ambiguity never becomes a guessed match. Independent instantiation can establish
decision dependence while another property remains unknown. A parameter value
can remain symbolic without data. Missing or incorrect support for an ordinary
promised projection, alias or selected-call fact is a gap, not a new exclusion.

Retain these families; the table separates source facts, local abstract facts,
optional callable proofs and consumer policy. All owners are existing modules in
`crates/zincite-lint/src`, except the named query consumer.

| Family and current owner | Named consumers | Retained guarantee | Intentional precision limit |
| --- | --- | --- | --- |
| Structural source facts: `bindings`, `callables`, `types`, `instantiations`, `model` | `decision_use`, `global_uses`, `unused_declarations`, all proof owners; query navigation | Same-context scoped identities, source ranges, structural types and unique first-order selections/substitutions; inspect written user references and required local/signature typing | No value, bound, presence, totality or output inferred from a signature; unknown candidates veto selection |
| Local domains/numeric facts: `domains` | `array_indices`, `effective_zero_one`, `suspicious_domain`, `guarded`, `iteration`, `unbounded_variable` | Compositional checked arithmetic, interval bounds, declared domains and symbolic identities | Overflow/error remains explicit; symbolic values and opaque calls supply no invented endpoint, member or extent |
| Local presence/partiality: `optional`, `guarded` | `hidden_optionality`, `partial_expression`, `compact_if`, `iteration`, fix eligibility | Presence separate from capacity; scoped truth and raw/effective definedness; MiniZinc relational partiality | Implication is not imperative short-circuit execution; opacity retains possible assertion/partiality barriers |
| Local indices/traversal: `array_indices`, `iteration` | `index_set_mismatch`, `expensive_comprehension`, `definitions`, `search` | Exact declaration/index identity, established membership and complete matching traversal | Equal cardinality is not equal indices; unknown extent/filter/optionality cannot certify whole-array coverage |
| Direct enforced definitions: `definitions`, `value_safety` | `constant_variable`, `unbounded_variable`, `search`, numeric producers | Safe initializers, enforced equalities, complete unfiltered matching universal definitions with dependency anchors/cycle checks | Conditional or one-element equality is not a whole definition; Unknown safety supplies no safe fix |
| Optional finite callable summaries: `callable_definitions` | `search`, combined definitions for `unbounded_variable`, nested callable summaries | Selected finite output dependencies, transparent enforcement/value wrappers, concrete substitutions and complete-owner publication | No arbitrary inversion, recursion execution or private-local output; a failed sibling cannot certify its owner |
| Search summaries/views: `search` | `search_coverage` | Actual searched seeds, complete direct/callable closure, standard searches/aliases/views and trusted complete enum2int projection (base-110) | Opaque relevant relations give Unknown coverage; filtered/partial views or user overrides cannot certify complete coverage |
| Lint/fix policy: `analysis`, rule modules, `fixes`; query predicates: `zincite-query/src/semantic.rs` | Library/CLI diagnostics, semantic queries and explicit edits | Independent findings/outcomes/errors, suppressions and source-preserving fix eligibility; Unknown vetoes dependent advice and matches | Failure to prove absence is not warning evidence; negation of Unknown cannot match; safe edits retain all stronger obligations |

Signature facts support navigation, decision-use advice and standard-call
recognition without proving what a body computes. They establish neither bounds,
presence, totality, output definitions nor search coverage. Constant-variable
advice and safe fixes keep their stronger safety obligations.

## Deliberate opacity

Opacity is a supported conservative operation after required source resolution
and typing. A uniquely selected signature may establish identity and types while
its body supplies no bounds, truth, definedness, outputs or search proof. Retain
argument facts and written reference/dependency edges, known relevant errors and
possible assertions/partiality. Inspect written user-body references and required
local/signature typing even when no body proof is requested. An unknown relevant
edge vetoes an unused conclusion. Bodyless/native calls are not universally total.

Precisely the former admission-only exclusions become supported opacity: skipping
arbitrary selected-body interpretation used solely to admit a typed expression;
skipping whole-body/initialized-source walks used solely to change Unsupported to
Unknown for symbolic bounds, generator membership/counts, local initializers,
relation definedness or search dependencies; and declining exhaustive library-body
execution, recursive execution or relation inversion when the consumer can retain
conservative Unknown without those operations. This permission removes the
admission prerequisite, not useful positive summaries or required source typing.
The regular/regexp/sliding-sum/GCC admission adapters and metadata/selected-source
adapters in the callable plan are examples, not mandatory replacement summaries.
Do not suppress already known source errors or erase possible abort/partiality
barriers; newly opaque bodies cannot acquire totality by being uninspected.

Remaining exclusions are explicit: unrestricted higher-order selection and
recursively changing type instantiations outside first-order support; general
computed enum-index dependency evaluation needed for complete data reduction;
and any requested exact evaluation/proof outside the retained bounded operations.
These remain Limited when indispensable to the requested consumer operation.
Missing ordinary projection/type/alias/call support is a promised-support defect,
not opacity. Missing source/dependencies and resource exhaustion are failures,
not permission to return Unknown.

## Detection families retained

All fourteen [thesis rules](../.zdev/base/background/thesis-rule-coverage.md)
retain working library/CLI paths, positive cases, locations and suppressions.
The examples below state supported paths, not new execution evidence.

| Rules | Useful supported path and limit |
| --- | --- |
| `array-index-start`, `effective-zero-one` | Proved numeric offsets and explicit 0..1 domains with checked arithmetic. Symbolic `1..N` and `1..K` need equality evidence. Enum indices are not numeric offsets. |
| `compact-if` | Decision Boolean condition and integer branches with one zero branch. Keep optionality and partiality restrictions. |
| `constant-variable`, `unbounded-variable` | Safe parameter initializers, enforced equalities and complete matching array definitions; explicit symbolic declared domains. Conditional/partial definitions do not count as full. |
| `element-predicate`, `reified-global`, `unmarked-symmetry-breaking` | Selected standard declarations, correct tuples and reification/marker context. User overloads do not match. Symmetry advice proves no model symmetry. |
| `global-variable-in-function` | A user-body reference to a top-level decision declaration, excluding shadowed formals/locals, callable names and field labels. |
| `search-coverage` | Standard searches, nested `seq_search`, supported aliases/views and finite output definitions. Opaque `p(x,y)` does not prove that searched `x` determines `y`. |
| `decision-variable-operator`, `decision-variable-generator`, `decision-variable-condition` | Known instantiation of relevant operands, domains/assignments or conditions/filters. Decisions only in a branch/body do not make its condition/domain a match. |
| `unused-declaration` | Static references rooted in constraints, solve/output and relevant annotations, including supported implicit output and unreachable cycles. Uncertain relevant edges veto unused conclusions; fragments cannot establish export unreachability. |

Keep the existing ten expanded families and their bounded numeric, guarded,
option, iteration and dependency consumers. Advice does not guarantee faster
solving. Findings do not upgrade fix safety: safe fixes still require solution,
objective, output, definedness, annotation, comment, index and source-preservation
obligations. Queries consume only the facts their selected stages require;
negating Unknown never makes a semantic predicate match.

## Named semantic exclusions

Do not extend interpretation merely to clear a corpus limitation. Deliberately
exclude arbitrary parameter/model evaluation, recursive function execution or
termination proofs, arbitrary relation inversion, general symbolic theorem
proving, exhaustive library-body interpretation and whole-program computed
value/dependency evaluation. Unrestricted higher-order callable interpretation
and recursively changing type instantiations are outside the promised
first-order support. Freeze new specialized relation-body inspectors.
Admission-only inspection may retire under the opacity contract; useful positive
proof cases require explicit review before removal or weakened expectations.

For numeric, totality, definition and search consumers these exclusions can block
bounds, definedness, complete outputs or coverage. For unused-declaration they
can block a conclusion only when the required reference/dependency edges cannot
be established; absent output proofs alone do not block static references. For
data reduction, general computed enum-index expression evaluation is excluded;
unresolved affected accesses keep the candidate incomplete. These exclusions do
not waive common structural types, source identity or first-order call selection.

Flattening, solving and whole-language validation remain outside the product.
Do not add instance expansion proportional to data, a full HIR/THIR, a fact
registry or a general fixed-point engine. Production persistent caching remains
outside this bundle; see the conditional Salsa decision below. Existing finite
output/search closures do not authorize a general evaluator. Unsupported
compiler extensions require explicit documentation, never opaque syntax
acceptance. Complete target syntax remains required.

## Outcomes and acceptance

Classify the operation required by each selected rule, not an entire fact bundle.
Keep `AnalysisResult` and `RuleOutcome`; use the following distinctions in plain
support documentation and located limitations.

| Situation | Handling and acceptance |
| --- | --- |
| Selected bounded analysis performed, including supported opacity, property unproved | Retain Unknown. Completed with no finding is valid when the promised bounded analysis admits this result. It proves no value or property. |
| Documented excluded operation blocks a selected rule | Retain a located limitation and Limited outcome, alongside independent findings. Acceptance may accept the named exclusion; it cannot count it as Completed. |
| Promised support missing or incorrect | Record a bounded gap/defect and retain Limited where analysis cannot finish. It blocks acceptance within promised scope. Difficulty does not create an exclusion. |
| Actual input/dependency failure | Preserve input errors and availability classifications, including missing/unreadable/cyclic includes. Missing parameter values alone are symbolic uncertainty. |
| Inapplicable context | Use Inapplicable for actual context restrictions, such as export reachability in a fragment or model analysis on standalone data. Difficult complete models are not inapplicable. |
| Resource cutoff or missing observation | Retain the resource failure/NotRun as appropriate or Unobserved report row. Never Unknown, Completed or a semantic exclusion. |

Keep exact selected IDs and every selected control root, with per-root/per-rule
Completed, Limited, Inapplicable and NotRun partitions and explicit invalid,
unavailable and Unobserved rows. Report accepted exclusions separately from
promised gaps without renaming or removing historical Limited rows. New runs
use the revised operation contract and identify approved expectation changes.
Acceptance requires no unresolved failures within promised support, complete valid-input parsing and
formatting, source preservation, complete accounting and the existing measured
performance obligations. Crashes, invalid target syntax, resource cutoffs and
missing observations cannot be waived to improve totals. Missing corpora or
dependencies prevent a full-coverage claim.

Preserve lint stderr limitations, source/suppression contracts and exits 0/1/2:
0 without warnings, 1 with unsuppressed warnings, 2 for actual errors with
precedence. A limitation alone does not change 0/1. Retain compiler pre-checks,
formatted-instance checks, source-preservation comparisons and final integration
under the brief; compiler validity alone does not prove a semantic fact.

## Historical work and retained boundaries

Base-098 classified retained source families, base-099 established simple-consumer
demand, and base-100 repaired query resource admission. Their results and the
base-093/base-094 callable map are historical evidence, not prerequisites to
reopen. Base-083 and base-050 acceptance records retain their actual revision,
root/rule selection, exclusions, source/dependency classifications and measured
cost. Policy adoption does not relabel any capture or rerun those tasks.

Retain the complete target syntax, parsing/formatting preservation, full rule
catalogue, finite query resource guards and independent query/data contracts in
the brief. General computed enum-index evaluation is still excluded from complete
data reduction: unresolved affected dependencies keep previews incomplete and
veto in-place writes. No corpus sweep or solver run is authorized by this revision.
The old initializer-query/whole-owner extraction drafts remain optional local
cleanup; a later semantic repair needs a concrete owner, witness and proof barrier.

## Warning and edit policy

Failure to prove a bound, definition or use absence alone cannot justify
`unbounded-variable`, `search-coverage` or `unused-declaration`. Keep scoped
explicit-domain advice for simple supported bare numeric declarations, as in
`tests/unbounded.rs`; this is advice about written domains, not proof of an
infinite feasible set. Uncertain relevant relations veto dependent advice, even
when opacity lets the selected analysis complete. A known uncovered independent
decision can still warn. References required for unused analysis remain inspected;
no execution proof is needed to establish a written use.

Unknown cannot enable a semantic query through negation or a safe fix. Preserve
solution, objective, output, definedness, annotation, comment, index and exact
source obligations. These rules govern the thesis coverage background's warning
and completion descriptions where a former required body-admission operation is
now supported opacity. They do not change the fourteen detection families.

## Demand and reuse before an incremental database

Ordinary per-analysis demand and reuse are the implementation target. `analysis`
selects source/signature facts for simple consumers and requests bounded body
proofs only for consumers that need them. Reuse each prepared fact result within
the same analysis/context; do not compute a full body result merely to discard it.
Inspect the existing prepared-definition path and mixed simple/deep selections
before changing scheduling. Keep current public fact APIs and direct functions;
no new IR, registry, general fixed point or cache framework is required.

Salsa is a conditional later choice, replacing the blanket rejection. Production
adoption is outside this bundle. It needs measured repeated-edit/recomputation
cost after ordinary demand/reuse, a concrete invalidation/identity design and a
separately scoped decision that accounts for integration cost. No speedup claim
follows from policy or fewer inspected nodes alone.

## Small baseline for this bundle

Current documentation baseline: `6b497f9caa46276fd9c62016371350a909a5b08e`
(2026-10-12). The following existing public controls were inspected here, not
rerun. Paths below are under `crates/zincite-lint/tests/`. Reuse them; add no test
for a documentation or scheduling-only change.

| Existing controls | Baseline obligation |
| --- | --- |
| `callables.rs::source_call_facts_skip_selected_standard_bodies_and_retain_written_user_calls`; `lattice_minima_unknown_candidates_and_rule_local_limits_do_not_guess` | Retain selected types/identities, written unresolved user calls and ambiguity vetoes across demand changes |
| `numeric_facts.rs::chains_intermediates_and_independent_requirements_support_only_conservative_proofs`; `domains.rs::named_domains_and_symbolic_bounds_retain_identity_and_precise_advice` | Retain compositional intervals/arithmetic and useful located array-index advice; distinct symbolic identities, BOM/CRLF and unchanged source |
| `guarded.rs::local_guards_preserve_obligations_identity_and_boolean_ranges`; `optional.rs::capacity_and_presence_follow_generator_kind_and_keep_parameter_identity` | Retain scoped positive definedness, raw uncertainty/relational partiality and capacity/presence distinction |
| `definitions.rs::whole_definitions_retain_identity_ranges_dependencies_and_item_suppression`; `conditional_partial_symbolic_and_cyclic_definitions_do_not_prove_whole_values`; `unbounded.rs::numeric_aliases_and_complete_anchored_definitions_keep_policy_separate` | Retain positive constant-variable findings/Completed, wrappers and whole definitions, suppression/ranges/source bytes, and vetoes for incomplete definitions |
| `search.rs::callable_outputs_keep_selected_direction_defaults_captures_and_array_extent`; `complete_enum2int_search_projection_keeps_shared_include_roots_distinct`; `enum2int_search_projection_refuses_user_partial_filtered_and_unsupported_values` | Retain finite output summaries, wrappers, complete projection coverage and per-root isolation; Unknown coverage/no warning for uncertain relevant projection cases |
| `unused_declarations.rs::anonymous_enum_initializer_is_a_call_not_an_unused_constructor`; `uncertainty_stays_local_and_fragments_do_not_claim_unused_exports`; `cli.rs::unused_declaration_selection_respects_complete_roots_limits_and_status_precedence` | Retain reachable uses, root applicability and outcomes/exits, with uncertainty vetoes |
| `fixes.rs::opaque_copy_edits_validate_boundaries_staleness_and_candidate_errors`; `crates/zincite-query/tests/queries.rs::model_selection_exposes_original_nodes_tokens_and_attached_comment_bytes` and `semantic_unknowns_survive_boolean_filters_projections_and_exact_root_checks` (repository path) | Retain exact source/comment bytes and refusal of stale/invalid edits; queries retain Unknown under negation |

The latest [acceptance checkpoint](../scripts/semantic-corpus-checkpoint.md),
“Bounded acceptance and the three reviewed repairs”, names the semantic originals
`neighbours-rect`, `mondoku-gcc-model-balance`, and `globals_all_equal_int`. Its
behavior capture used repaired working-tree source based on `1b37220`, pinned to
code committed through `11dc52a`: thesis limitations 11→9, 76→25 and 13→13,
respectively. Mondoku unused-declaration completed; ambiguous all_equal routes
remained Limited. Both presets kept exact 14/28 IDs, three resolved root rows,
no timeout/input errors, identical native/companion diagnostics and unchanged
originals/tools. The checkpoint reports fmt/clippy and 381 workspace tests at
that repair revision. These are historical witnesses, not a run at this baseline.
Use those three named originals for later bounded comparisons when available;
old `target/benchmarks` and `/tmp` captures are optional evidence, not hidden
prerequisites. No private source is published and no corpus replay is required.

For scheduling-only changes, compare complete facts, ordered diagnostics,
limitations, per-root/per-rule outcomes and source preservation unchanged.
For semantic simplification, separately record exactly which admission-only
Limited cases become Completed with Unknown, retaining warning/query/fix vetoes.
Do not rewrite old rows. Any lost useful positive proof/finding needs explicit
review; Unknown-only body admission shapes need not remain shape-specific tests.
This baseline is a small bundle control set, not a substitute for the existing
full catalogue and parsing/formatting acceptance evidence.
