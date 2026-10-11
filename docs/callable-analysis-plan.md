# Callable analysis responsibilities and bounded follow-ups

Recommend keeping the stages introduced by base-093 and tightening the two
existing local-initializer consumer contracts before extending those routes.
Callable analysis should provide bounded output guarantees and checked source
prerequisites. It should not become a general value evaluator or a certificate
that every selected analysis completed.

This is the base-094 design result, inspected at
`fda190e27e104538e9cbec236435c985235a573d`. It proposes future work; it changes no
fact API, semantic behavior, brief policy or task graph. The
[brief's Semantic analysis and Testing contracts](../.zdev/base/brief.md) and
[thesis coverage contract](../.zdev/base/background/thesis-rule-coverage.md)
remain authoritative. In source references below, `src/` and `tests/` mean
`crates/zincite-lint/src/` and `crates/zincite-lint/tests/`.

## Current responsibilities

All collaborating facts must come from the same retained `ModelContext`.
Declaration IDs, selected declarations and concrete type tuples determine
meaning; a callable's spelling does not. Physical file/ranges, original syntax,
lexical headers, optionality and overload ambiguity remain prerequisites.

| Owner and API | Prerequisites and result | Current consumers and absent guarantees |
| --- | --- | --- |
| `bindings::resolve_bindings`, `callables::resolve_callables`, `types`, `instantiations::resolve_instantiations` | Retained include closure and scopes; resolved/unresolved/ambiguous bindings, explicit call outcomes, qualified types and independent `par`/`var`/Unknown facts. Selected bodies retain formal identities and concrete substitutions. | Callable analysis, domains, definitions, guarded/iteration and rules. Resolution and typing establish neither values nor totality. Unknown matching never selects a guessed overload. See `bindings.rs:76–105`, `callables.rs:40–104`, `instantiations.rs:13–32`. |
| `domains::resolve_domains`, `resolve_integer_bounds`, `resolve_numeric_facts` | Raw written domains first; later readers receive actual calls/instantiations/definitions as needed. Symbols retain declaration identity; checked arithmetic can produce bounded numeric facts, Unknown or Unsupported. | Definitions, callable source inspection, array-index advice, guarded/iteration and numeric rule consumers. Raw domain production cannot presume later initialized-source inspection; source admission supplies no bound or cardinality. See `domains.rs:11–91,2035,2222,2541`. |
| `definitions::resolve_definitions` and `value_safety` | Direct initializer/equality candidates, resolved references, value instantiation, enforcement, coverage, safety and cycles. Full definition requires Enforced, Scalar/WholeArray, Supported safety and appropriate dependency anchors. | Constant-variable, unbounded-variable, search and later semantic producers. A candidate or an element definition does not define the whole owner. Dependencies are direct references, without arbitrary body expansion. See `definitions.rs:15–96,643–763`. |
| Callable discovery and interpretation | Root clauses; child-first selected body discovery, concrete tuples, ancestry/recursion checks; bounded clause handlers and actual/formal mapping. Output eligibility is a separate step from source inspection. | Output/boundary fixed points and model publication. No inversion of arbitrary computed values, callable-private locals, partial selections or opaque/recursive calls. See the [base-093 module map](../crates/zincite-lint/src/callable_definitions.rs), lines 1–28. |
| `inference::infer_outputs`, `infer_boundaries` | Output least fixed point first; boundaries then propagate using a complete previous-round snapshot and full boundary equality. | Nested calls and model roots. A successful output can coexist with an unavailable sibling; the result is not a model-wide completion certificate. See `callable_definitions/inference.rs:4–86`. |
| `source::SourceInspector`, `BodyInterpreter`, adapters | Retained facts, exact selected families, original written bodies and initialized backing sources; actual lexical headers; closed source-error, annotation, optionality and cycle checks. | Definitions, search, domains, array-indices, guarded and iteration. Inspection supplies no value, nonemptiness, membership, extent, totality, output, definition or search proof. Body-aware Float/local inspection remains with the interpreter. |
| `dependencies` | Strict ordered declaration dependencies under the selected view, actual headers and proof-sensitive source eligibility. Returns dependencies or an explicit refusal. | Callable output interpretation. An inspection-only Unknown cannot satisfy this path. See `callable_definitions/dependencies/traversal.rs:622–640`. |
| `model_roots::interpret_model_roots` | Model facts in source-item order. Constraint-local inspection publishes only after all relevant siblings, item iterations and inspection-only Boolean siblings pass. | Callable model definitions and `inspected_locals`; no sibling may certify a failed owning item. See `callable_definitions/model_roots.rs:4–86`. |
| `search::resolve_search_coverage` | Complete-root state, resolved search annotations/views/aliases, direct and callable definitions, whole-array closure, source safety and dependency cycles. Fresh model-value locals have a separate owning-source prerequisite route. | `SearchFacts` feeds search-coverage advice. Inspection IDs exempt only the exact unsupported-local accounting case; they do not seed closure. See `search.rs:111–137,1640–1872`. |
| `guarded` and `iteration` | Guarded owns truth, raw/effective definedness and scoped obligations; iteration reads those facts plus numeric/optional/domain facts and owns membership, counts and traversal coverage. | Their respective rule consumers. Shared source admission does not merge their scopes, erase raw failures or prove a count. See `guarded.rs:23–30,196`, `iteration.rs:24–66,293`. |
| `analysis` and rule modules | Selected facts, root availability/applicability, source suppressions and each rule's required analysis. Findings, execution, limitations and errors stay distinct. | Library/CLI results. Constant-variable requires Supported safety before advice; unbounded-variable withholds advice for Unknown; search-coverage warns only on complete roots with Uncovered/PartialArray top-level decisions. See `analysis.rs:29–58`, `constant_variable.rs:24–76`, `unbounded_variable.rs:15–167`, `search_coverage.rs:8–47`. |

The controlling public call remains:

```rust
pub fn resolve_callable_definitions(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
) -> CallableDefinitionFacts
```

Its sequence is: build retained lookup indexes and a borrowing source inspector;
collect root clauses; discover instances; infer outputs; infer unavailable
boundaries; publish model roots; mark definition cycles
(`callable_definitions.rs:97–121`). These stages produce facts without choosing
diagnostics or mutating source.

## Public callable facts and their actual uses

| Field | Guarantee and permitted use | Consumer and remaining obligations |
| --- | --- | --- |
| `outputs: Vec<CallableOutput>` | One selected callable and concrete `parameters` tuple; a formal `target`, ordered `dependencies`, Scalar/WholeArray `coverage`, source `location`, and `enforced_boolean`. The last flag permits the written Boolean actual to supply an enforced equality; it does not permit arbitrary inversion. | Inference and nested/root interpretation map formals to actuals. No inspected rule directly reads this vector. The public re-export permits library use, with the same context, selection, enforcement, coverage and cycle obligations. No computed value or general relation totality follows. |
| `definitions: Vec<Definition>` | Supported enforced model-call/scoped-let definitions, carrying target, file/item, location/value, instantiation, dependencies, enforcement, coverage, safety and cycle status. Publication initially supplies Enforced/Supported records; cycle marking still applies. | Search consumes them alongside direct definitions. Analysis combines both sets and marks cycles again for unbounded advice (`analysis.rs:736–749`). Constant-variable consumes adjusted **direct** definitions, not this vector (`analysis.rs:755–757`). |
| `unavailable: Vec<UnavailableCallableDefinition>` | An exact source `location`, affected `targets` and refusal `reason`; no successful proof is implied for those targets. | Search first marks uncovered affected targets Unknown, then reports a limitation if closure still leaves a target uncovered, or if the boundary names no targets (`search.rs:111–137`). Independent existing coverage can discharge a target-specific barrier. Unbounded advice does not directly read this vector. |
| `inspected_locals: Vec<DeclarationId>` | Fully inspected constraint/model-let Boolean choices, integer initializers or comprehensions with unproved membership/shape, gated by the complete owning item. IDs retain exact source identity; they supply no output or coverage. | Search excludes these IDs from unsupported-local accounting while retaining Unknown. Unbounded advice uses only exact same-source scalar-int initializer cases. Analysis adjusts one exact local rank-one int comprehension refusal to Unknown; this is an indirect ConstantVariable dependency and why that rule currently triggers callable analysis (`analysis.rs:434–489`). |

`search_coverage` consumes `SearchFacts`, not `CallableDefinitionFacts`.
`definitions` and `array_indices` consume source adapters rather than callable
output summaries. `domains`, `guarded` and `iteration` likewise use bounded
adapters; their facts keep their own meaning.

## Source adapters: admission is narrower than proof

The adapter map below covers the current consumer entry points in
`callable_definitions/adapters.rs`. Unless stated otherwise, inspection needs
context/bindings/calls/instantiations/domains, the owning file/node and actual
preceding headers. `None` means this route has not admitted the family; the
caller keeps its fallback or refusal. It never means safe. `Some(Unsupported)`
preserves a recognized source failure. Even `Supported` from a direct checker
is expression-safety support, not the caller's enforcement or coverage proof.

| Adapter family (line) | Extra prerequisites and result | Current clients and proof boundary |
| --- | --- | --- |
| `direct_expression_safety` (4), `indexed_expression_safety` (1073) | Same body-free direct checker; Supported/Unknown/Unsupported. Indexed input borrows the original facts' lookup rows. | Definitions (`742`), search RHS (`1935`), domains length (`2842`), array-indices (`503`). Search still checks selected core conjunction/enforcement and whole-array coverage. Numeric length maps Supported to Unknown. |
| `initialized_expression_safety` (556) | Follows initialized sources. Its Boolean flag selects the existing eager closed-integer veto for newly admitted traversal syntax; it does not request a value. | Search annotation/view sources (`search.rs:419,708,850,1100,1161,1577`), guarded (`1288,1767`), array-indices (`448,492,612,924`), numeric bool2int (`domains.rs:3047`). Each caller owns eligibility and uncertainty. |
| `initialized_conditional_bound_safety` (641) | Present parameter-int conditional, complete branches and source-error checks; successful result is Unknown. | Array-indices (`337`) retains local/formal correspondence and declines an unproved minimum; no branch value or nonemptiness proof. |
| `initialized_integer_set_source_safety` (854), `selected_integer_set_source_safety` (740) | Known nonoptional parameter set-of-int source. Original calls own initialized declarations; the current view owns the expression. Selection additionally requires a scalar from a present rank-one top-level parameter array and actual preceding headers. | Definitions (`976,1088`), domains (`1822`), array-indices (`953`); selected sets feed guarded (`3279`) and iteration (`698`). Success can retain Domain::Unknown; it proves no selected member or extent. |
| `decision_prefix_source_safety` (717) | Exact decision-prefix family under actual preceding integer binders, including its selected written range body. | Guarded predecessor/domain (`3212,3270`), iteration (`689`). Neither prefix membership nor a bound follows. |
| `initialized_parameter_set_extremum_safety` (908), `parameter_index_extremum` (1162), `length_argument` (957) | Exact selected typed extremum/metadata families. Recognition returns safety, formal identity or the written argument, respectively. | Bounds/numeric (`domains.rs:1375–1407,2809,3079`) and array-indices (`240,893`). These results establish no nonemptiness, extremum or length. Strict output dependencies retain separate refusals. |
| `set_axis_integer_reshape_source` (1138), indexed wrapper (1117) | Exact checked standard reshape and value-preserving source; the wrapper only selects raw-definition inspection fallback. | Search views (`1553`) and definitions (`706`). Source/cardinality checks remain required; unknown reshape cardinality does not define a whole array (`callable_definitions/source.rs:866–887`). |
| `regular_four_source_safety` (27), `literal_regular_source_safety` (177), `sliding_sum_source_safety` (384), `gcc_source_safety` (94) | Exact selected standard declaration and tuple, written bodies/metadata, native endpoints and actual sources. Regular/sliding discovery supplies concrete views without output inference. Recognized success is Unknown; failures stay Unsupported. | Guarded relation handlers (`2061–2158`) retain unknown definedness, clear truth/numeric and retain possible assertion failure. Callable interpretation also owns its named relation inspections. No truth, totality, count, window membership or output follows. |
| `optional_parameter_matrix_initializer_safety` (452) | Whole owning lexical initializer of the admitted optional matrix family. | Guarded (`2027–2047`) keeps optional-value uncertainty, Unknown definedness and no truth/numeric result. |
| `native_operation_source_safe` (595), `closed_integer_type_source_error` (692) | Selected primitive/signature admission, or a written closed arithmetic failure. A Boolean/native success still requires initialized-source checks. | Array-indices (`975`); guarded/iteration type-error readers (`3947`/`473`). Neither API supplies a domain bound or certifies a result. |
| `indexed_model_value_fresh_locals` (1095) | Top-level User value owner, known nonoptional type, unique initializer; complete initialized value/domain/metadata/annotation and local scope checks. Returns `Option<Result<Vec<DeclarationId>, String>>`. | Search only (`1709–1726,1871`). Ephemeral prerequisite accounting is separate from callable facts; failure admits no sibling IDs, success supplies no searched seed, definition or output (`callable_definitions/source/locals.rs:746–817`). |

Lookup lifetime is part of these contracts. `LookupIndexes` and search's indexes
select the first original row, preserve physical ranges and borrow immutable
facts (`callable_definitions/source.rs:42–88`, `search.rs:1662–1691`). Substituted
callable views use the linear fallback; original-call indexes cannot stand in
for that view.
The existing Some/None lookup choices must survive cleanup. There is no measured
cost defect in this repeated construction and no reason here to add a cache.

## Checked examples and counterevidence

1. **Refusal text crosses ownership boundaries.** `value_safety.rs:127,176`
   emits the membership and control/iteration refusals. `analysis.rs:444–489`
   matches the latter's exact text plus local ID/type/enforcement/coverage and
   exact initializer ranges, then changes only the checked local comprehension
   to Unknown. `unbounded_variable.rs:122–140` independently matches the former
   plus scalar-int and exact same-source initializer gates. Changing refusal
   wording can therefore change completion/advice policy in distant consumers.
   This is observed coupling, not demonstrated unsoundness: both bridges withhold
   proof and advice; search rejects non-Supported closure safety (`search.rs:1765`).

2. **Two owning-source routes are intentional.** Constraint locals enter
   `inspected_locals` only after the complete constraint item passes
   (`callable_definitions/model_roots.rs:66–86`). Model-value locals enter search's
   separate ephemeral list after the entire initializer passes. The public test
   `model_value_fresh_integer_lets_complete_without_search_certificates`
   (`tests/search.rs:16101`) uses symbolic `limit`/`enabled`, local `p`/`q` and
   an array comprehension. It asserts Completed search analysis with Unknown
   coverage for both locals and `choices`, no searched seeds, outputs or local
   definitions, and no callable `inspected_locals` entries. Replacing a sibling
   constraint with `q >= (1 div 0)` retains Limited and exempts neither local.
   A shared helper must not erase the owner distinction or all-siblings gate.

3. **Generator admission repeats, but evaluation differs.** Guarded's
   `generator_domain` (`3257–3307`) and iteration's `source_set` (`607–742`)
   dispatch the same prefix/selected-set adapters and map success to
   Domain::Unknown. Guarded supplies its actual scope and evaluation; iteration
   reconstructs preceding headers, requires guarded raw/effective definedness
   and checks declaration math. The public decision-prefix control
   (`tests/iteration.rs:474`) retains written standard range/optional-sum bodies
   around `1..demand_end_job[d]`. Sharing their whole functions would discard
   prerequisites. A source error in an earlier header/body remains counterevidence
   to admission; success establishes neither members nor counts.

4. **Relation specialization reflects different obligations.** Regular/4,
   literal regexp, sliding-sum and bound-GCC repeat inspector setup and guarded
   Unknown propagation, but differ in selected tuples, optional transitions,
   nested bodies, regexp/native handling, window sources and count sources.
   Existing tests explicitly refuse totality/output certificates
   (`tests/guarded.rs:1169,1398,1610`, `tests/search.rs:15641`). Small result
   handling could be shared if it reduces a named caller's reasoning burden;
   the evidence does not support a universal standard-library interpreter.

5. **Metadata inspection must not become output proof.** The rank-three
   `length(matrix)` case inspects original backing/axes; numeric interpretation
   declines a value (`domains.rs:2792–2861`) and strict dependencies explicitly
   refuse the new route (`callable_definitions/dependencies/traversal.rs:622–640`).
   The public test (`tests/definitions.rs:3352`) retains Unknown definitions and refuses
   backing overflow, axis division-by-zero and changed primitive bodies.
   The [checkpoint's multidimensional-length section](../scripts/semantic-corpus-checkpoint.md)
   (`7523–7540`) records an earlier rejected candidate that leaked into output
   inference. That candidate is historical; the current explicit refusal is
   counterevidence to a current defect claim.

Search also rechecks direct RHS safety with its own whole-array traversal
fallback (`search.rs:1920–1977`). Its narrower enforcement/closure needs explain
this recomputation; no checked evidence establishes that it is redundant or a
performance defect. Whether wider sharing helps remains a hypothesis.

## Alternatives and recommendation

| Approach | Benefit to current consumers | Trade-off and cost |
| --- | --- | --- |
| Keep the staged modules and document adapters only | Preserves all current proof barriers and public APIs with no implementation cost. Enough when no relevant source family needs extension. | Exact refusal-text bridges and owning-source accounting remain difficult to change safely. Every future repair must retrace these seams. |
| Keep the stages; tighten named local-initializer and owning-source contracts | Centralizes the two checked adaptation gates in `definitions`; retains strict proof paths and caller-specific evaluation. Public facts and ordered results can remain unchanged. | Small private query/refactor work and focused before/after comparison. The query retains the existing safety strings in one classification owner. It does not itself repair missing support or improve measured cost. **Recommended before extending those routes.** |
| Extract a reusable source-inspection unit inside `zincite-lint` | Could reduce repeated setup across existing consumers if a concrete shared contract emerges. | Broader movement of scope/view/lifetime obligations; greater validation and compatibility cost. Current evidence does not justify this step or a new public API. Reconsider only for named consumers that cannot use the existing adapters cleanly. |

For the recommended approach, preserve all public fact types and strings. Prefer
a named private query in `definitions` that centralizes the existing exact safety
string, initializer, identity and owning-item checks. Private refusal tags would
need additional transport: `Definition` and `DefinitionFacts` are public and
constructible (`definitions.rs:40–59`); analysis reuses prepared definitions
(`analysis.rs:304–315,373–375`) and clones direct/callable records into a combined
set (`734–749`); definition recording can replace ordered records
(`definitions.rs:596–618`). Parallel metadata would have to survive these paths
and account for caller-constructed facts. The query reads the actual record and
requires no additional state. This meets the current consumer need with less work.

The proposed private result needs only two admitted kinds:
`RankOneIntegerComprehension` and `ScalarIntegerMembership`. A signature sketch is:

```rust
fn inspected_local_initializer(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    definition: &Definition,
    inspected_locals: &[DeclarationId],
) -> Option<InspectedLocalInitializer>
```

`Some` means the existing gate for that consumer's Unknown treatment passed;
`None` preserves its existing path. Both kinds require the same retained context,
an inspected Local ID, known nonoptional decision type, matching file/item and
exact local/initializer ranges. The comprehension kind additionally requires
the existing rank-one integer-array type, ArrayComprehension initializer,
Enforced/WholeArray record and exact control/iteration refusal. Membership requires
the existing scalar-int type, Enforced/Scalar record and exact membership refusal;
do not add a root ArrayAccess syntax requirement that the current bridge lacks.
Unbounded advice retains its prior cycle/dependency-anchor and limitation precedence
checks. The query supplies no additional source inspection or proof.

Analysis applies only its existing comprehension-to-Unknown adjustment; unbounded
advice applies only its existing membership-to-Unknown treatment. Neither caller
classifies by refusal text. The two exact strings remain compatibility details
inside `definitions`, matching their unchanged `value_safety` producers. Do not
normalize other Unsupported definitions or mutate raw public definition results
for other users. This is a **proposed private query**, not a contract implemented
today.

For owning-source accounting, use the existing distinction: complete constraint
item produces `inspected_locals`; complete model-value initializer produces
ephemeral search IDs. Keep the whole-owner gate named and adjacent to publication.
Constraint-item inspection currently depends on discovered bodies and stabilized
output/boundary snapshots. Any future extraction retains those inputs; merely
requesting ConstantVariable does not justify bypassing them or establish redundant
pipeline cost. No shared inspection result may be consumed as Supported definition safety,
strict dependencies, searched seeds, coverage, value or totality.

Leave raw domains/numeric evaluation, guard truth/definedness, iteration
membership/cardinality, search annotation interpretation and lint advice with
their present owners. Do not add a generic certainty framework, fact registry,
cache, evaluator, crate or exhaustive standard-library interpretation. Lookup
or relation scaffolding cleanup is optional and needs a concrete consumer benefit.

## Unknown, missing support and unavailable prerequisites

Classify completion for the applicable **rule**, not for an entire callable fact
bundle. A value remaining unproved is not itself an implementation gap.

| Situation | Required handling |
| --- | --- |
| Recognized source, all required inspection performed, symbolic value/membership/extent remains unproved | Ordinary Unknown; the consumer can complete and decline an unprovable candidate. The fresh model-value local test is a checked witness. No stronger fact follows. |
| Required inspection/interpretation is missing, an opaque unfamiliar construct blocks applicable required analysis, or a necessary body/source remains unsupported | Explicit Unsupported/limitation and Limited execution. Keep base-083 acceptance open; identify a bounded required-support gap before repair. Do not relabel this Unknown to remove limitations. |
| Strict output/dependency proof is refused after inspection succeeded | Preserve the refusal. Determine separately whether the rule completed a supported negative analysis or lacks required implementation support. Rank-three length demonstrates the barrier, not blanket permission to remove output limitations. |
| Missing/unreadable/cyclic external include or absent model context | Preserve dependency errors/availability and affected rule limitations. Missing parameter data alone is symbolic uncertainty, not missing source. |
| Compiler-invalid source, parse errors, standalone data or include-only fragment | Keep input/error/applicability classifications separate. In particular, unused-declaration is inapplicable to a fragment without a complete root, not to an unresolved valid complete model. |

Compiler acceptance establishes validity of the checked source/instance. It does
not establish every value, totality or output, nor require the linter to prove
them all. Conversely, it cannot excuse missing brief-required support. Each
consumer must retain declaration identity, ambiguity veto, option/partiality
guards, enforcement, whole-array coverage, cycles, file/ranges and suppressions.
Known symbolic types/parameters are usable without data; unfamiliar syntax and
unavailable prerequisites must retain their actual reason.

The latest [checkpoint](../scripts/semantic-corpus-checkpoint.md), lines
`7490–7505`, records complete two-original captures through `7482f51`: Workforce
completed 14/26 analyses; Connect retained nineteen model-local prerequisite
sites (sixteen Constraint, three model-value). Later bounded conditional-array,
length and minimum checks retain Unknown and pending original/full-corpus
acceptance (`7506–7562`). These are retained observations, not current corpus
counts or proof that every retained site is a defect. This task ran no corpus
sweep, compiler control or solver.

## Proposed follow-up drafts

These are drafts without task IDs or imported records. Each needs normal review
and approval. They do not broaden base-083 or change its blockers.

### Name the two inspected-local initializer adaptations

**Outcome:** analysis and unbounded advice no longer select these two behaviors
by rendered refusal text; the current public facts, ordered diagnostics,
limitations and rule outcomes remain identical.

**Ownership:** `definitions` owns the proposed `inspected_local_initializer`
query and its two private admitted kinds described above. Move the current exact
string and initializer/identity/owning-item gates there; keep `value_safety`,
`Definition`, `DefinitionFacts` and public `resolve_definitions` unchanged.
`analysis` consumes only the comprehension kind and `unbounded_variable` only the
membership kind, retaining its existing cycle/anchor and limitation precedence
checks. Compute the query from the supplied record and current facts; add no
parallel metadata, storage or transport. Return no Supported safety or proof.

**Dependencies/boundaries:** approved contract plan; no support extension,
numeric/guarded/search rewrite, broad refusal taxonomy or public API/layout change.
Recommended before a repair touches these adaptations; optional for classification
or unchanged routes. If preserving behavior would require broader API work,
stop this draft and present that decision separately.

**Done when:** neither consumer classifies these adaptations by refusal text or
repeats their moved source identity gate; the query preserves both exact current
admission cases and refuses all other cases. Raw public facts, record order and
selected rule outcomes match before/after, including prepared-definition reuse
and the cloned combined set.

**Validation:** existing scalar membership control (`tests/search.rs:1038`) and
the local-comprehension/failed-owner cases (`tests/search.rs:3149–3217`);
matched full direct/callable facts and ordered diagnostics/limitations
before/after for these cases and one retained bounded capture exercising these
paths. Include selected-analysis comparisons with `array-index-start` plus
`constant-variable`/`unbounded-variable` to exercise prepared reuse and the merge;
do not assert that this makes their proof prerequisites interchangeable. Reuse
existing cases; no new test solely for query extraction. Rust edits require
workspace fmt/clippy/tests.

### Make whole-owner local inspection explicit at publication

**Outcome:** constraint-item publication and model-value search accounting expose
their complete-owner prerequisite through named private calls, with identical
inspection IDs and public results on success and failure.

**Ownership:** keep constraint gating in `callable_definitions/model_roots` and
body/source checks in the existing interpreter; keep model-value checking in
`source/locals` and its ephemeral list in `search::close`. A private
`inspect_constraint_item_locals` may extract the existing sibling/iteration
gate, but must return no IDs on failure. The existing model-value adapter's
Option/Result contract remains separate. Preserve the discovered instances and
output/boundary snapshots required by constraint publication; no reduced-demand
callable pipeline is part of this draft. Consolidate only a shared scan that
these two current owners actually need; do not relocate whole guarded/iteration
functions or export another public fact vector.

**Dependencies/boundaries:** approved plan; independent of the initializer-query draft
unless their concrete edits overlap. Recommended before extending either local
inspection route; optional if classification finds no such required extension.
No new syntax admission, proof/coverage changes, cache or lookup optimization.

**Validation:** reuse `tests/search.rs:16101`, the constraint-local array cases
(`2417–2518`) and local-comprehension/failed-owner cases (`3149–3217`);
compare full facts, exact IDs/order, diagnostics,
limitations and outcomes. Confirm model-value IDs still do not enter callable
facts and neither route supplies outputs/search seeds. Rust edits require the
workspace gates; no new test for extraction alone.

### Classify the retained Connect local-prerequisite families

**Outcome:** a bounded addition to the existing checkpoint accounts for the
nineteen retained Connect sites by owning item/family and affected rule. Each
row names the required fact, retained validity/availability evidence, current
producer/consumer path, classification and any separately bounded repair needed.
Reconcile relevant later conditional/minimum inspection evidence; do not assume
those increments cleared an original site.

**Ownership:** checkpoint documentation plus read-only inspection of each named
local-prerequisite path. Read private originals/captures without publishing their
sources. Use one small private or public control only where the retained evidence
cannot decide a specific classification.

**Dependencies/boundaries:** approved plan and retained captures; this is a
necessary prerequisite to selecting further local semantic repairs. It performs
classification only. No Rust repairs, task import, brief change, diagnostic
grouping, cost optimization or full-corpus replay. Distinct missing-support
families need separately reviewed repair drafts with exact owners, witness,
proof barrier and validation; do not create one catch-all semantic repair task.

**Validation:** account for every retained site, including supported uncertainty,
required missing support and actual source/external failures. Check each proposed
gap against the unchanged fourteen-rule contract and a concrete counterexample.
Preserve Limited for unresolved required support. Documentation needs diff/reference
checks; a necessary bounded control needs complete public facts/rule outcomes,
and compiler no-solve checks only when validity remains undecided.

## Base-083 resumption and decisions

No brief or product-scope change is proposed. The user decision is whether to
approve this bounded plan and which proposed contract refactors to adopt. Missing
required coverage must be repaired, not waived by adopting the plan. The
classification draft precedes selection of any remaining local-support repair;
the two refactor drafts are conditional prerequisites only for extensions of
their respective routes.

Resume base-083 only after:

1. Base-093 and base-094 independently complete; the fresh advanced-design
   challenge has tested this recommendation and consumer contracts. Resolve
   supported objections or retain material choices explicitly before dependent work.
2. The user approves the bounded resumption plan. Review/publish necessary
   follow-up drafts through the normal coordinator-owned workflow and satisfy
   their prerequisites. Any new scope/completion/brief decision needs explicit
   approval before application. Base-094 completion alone does not authorize
   semantic repairs or corpus sweeps, even if the graph becomes ready.
3. Fresh work-context admits base-083, retaining `base-080`/`base-082` blockers,
   completed increments and unmerged private evidence. Leave base-081, base-084
   and query/data work independent. Do not treat private rejected candidates as
   integrated repairs.

Resumed base-083 still owns its diagnostic-scaling/grouping decision, including
the retained generated Project Planning example; confirmed expanded-analysis
cost with matched outcomes and bounded growth/allocation/native evidence; and
fresh complete thesis/all accounting after final changes. Preserve all original
roots, exact 14/26 selected IDs, source/dependency/availability classifications,
per-rule partitions and invalid/unavailable/Unobserved rows. An approved plan,
bounded controls or two-original replay is not final acceptance. No solver run
or universal value proof becomes a prerequisite.

## Advanced-design review

The first independent advanced challenge returned rework: the proposed private
refusal tags lacked a transport contract through public constructible facts,
prepared-definition reuse, cloned merges and record replacement. The coordinator
checked those paths against the unchanged draft. This revision resolves the
objection by choosing the named query over tag transport; classification by the
existing exact strings stays in one owner, and public fact layouts stay unchanged.

The revised document still requires a fresh focused advanced challenge, then a
fresh whole-task verifier. This review note claims neither a whole-task pass nor
implemented contracts or final corpus acceptance.
