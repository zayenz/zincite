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
recorded outcomes. Base-115 through base-121 implement this contract through
owner-produced uncertainty, possible-reference boundaries, selected proof demand,
shared `ModelFacts` and removal of admission-only inspectors. The
[2026-10-12 acceptance](../scripts/semantic-corpus-checkpoint.md#base-123-bounded-semantic-acceptance-2026-10-12)
checks the implemented state at `d7c33659050c03ea1460df13b1fffe42220e6fd7`.
Historical execution counts remain unchanged.

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
The historical initializer-query draft is superseded by owner-produced
uncertainty. Any retained whole-owner check needs a concrete proof or scoped
accounting consumer; a later repair needs an owner, witness and proof barrier.

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

Ordinary per-analysis demand and reuse are implemented. `analysis`
selects source/signature facts for simple consumers and requests bounded body
proofs only for consumers that need them. Reuse each prepared fact result within
the same analysis/context; do not compute a full body result merely to discard it.
Inspect the existing prepared-definition path and mixed simple/deep selections
when changing scheduling. Keep current public fact APIs and direct functions;
no new IR, registry, general fixed point or cache framework is required.

Base-118 routes decision-use, standard symmetry/global recognition, unused
reachability, element advice, comprehension advice and enum advice through
`resolve_source_callables`. Mixed selections keep that source view separate from
the optional proof view. Captures and shadowing use bindings directly. Source
facts retain signature/default dependencies, written user-body references and
local typing. `resolve_callables` retains its public body-aware behavior.

Query types, instantiations and reference targets also start with source facts.
When a query explicitly reaches a standard body occurrence, or unused reachability
needs a raw uncertain body, `resolve_source_callable_body` adds that body's
written reference/local typing through the same callable engine. It follows no
transitive implementation bodies and supplies no value or output proof. Unused
reachability still follows relevant possible edges, defaults, captures, implicit
output and annotations; unresolved relevant edges veto unused conclusions.
Data reduction uses source types for static model accesses and retains its
computed enum-access refusals and edit vetoes. Query scan and result limits
remain in force.

The retained proof route serves concrete properties:

| Producer | Property requiring stronger facts |
| --- | --- |
| `domains`, numeric producers | Checked bounds, endpoints and offsets for numeric and index advice |
| `optional`, `guarded`, `compact_if` | Presence, truth, raw/effective definedness and possible assertion barriers |
| `iteration`, comprehension and global-pattern producers | Index membership, complete traversal, cardinality, multiplicity and raw operand totality |
| `definitions`, `value_safety` | Safe initializers, enforced equalities and complete anchored definitions |
| `callable_definitions`, `search` | Finite output dependencies and complete search closure/views |
| `input_preconditions` | Located formal-input definedness obligations and proved default/empty-iteration capture |
| `element` rewrite producer | The concrete indexing-equality relation, raw operand definedness and index membership; selected standard identity alone still supports advice |

Each immutable `ModelContext` has a `ModelFacts` owner. Named requests reuse
bindings, callable preparation, source instantiations and domains before direct
definitions, numeric, optional, guarded and iteration facts. Normal lint and query
consumers start with the source view. They do not request a transitive selected-body
closure merely because a deep rule is enabled. Unbounded advice requests a separate
combined definition result with cycle marking, leaving direct definitions unchanged.

A selected-body request names bounds, definedness or outputs, the selected
declaration and its exact concrete formal types. `CallablePreparation` shares that
body's concrete typing within the model owner. The cache uses only canonical source
facts and the concrete formal tuple. Actual arguments, defaults mapped at an
invocation, lexical generators, guards, membership checks and output publication
remain with their existing proof owners. Those contextual conclusions are evaluated
for each invocation; declaration identity alone cannot reuse them.

Callable output discovery checks written output forms and forwarding declaration
identities before requesting concrete typing. It retains possible scalar and array
equalities, enforced Boolean formals, transparent forwarding, returning assertions
and their reachable summary dependencies. Admitted owners retain every sibling,
finite instance deduplication, concrete-instantiation recursion barriers and the
existing output-then-boundary fixed points. Whole-array extent and complete-owner
checks still govern publication. An excluded relation retains written references
mapped through actuals, defaults and captures as localized Unknown boundaries.

The opaque-source error adapter checks actuals, written user sources and bodies
already inspected by required work. It does not recursively expand an unrequested
library implementation for admission. Known argument/initializer errors, strict
empty extrema and errors discovered in demanded bodies still veto dependent proofs.
There is no promise to discover arbitrary errors inside an excluded uninspected body.
This retires admission-only library walks, without replacing retained useful bounds,
definedness, traversal, definition or search proofs with Unknown.

`resolve_callables` and `ModelFacts::proof_callables` remain explicit bulk compatibility
requests for all transitively selected standard bodies. Body typing for both bulk and
selected-property requests belongs to `callables::Engine::inspect` and `Engine::infer`.
The bulk traversal retains generic declaration types and accumulates selected-body
observations. `requested_body` uses `instantiated_body` to bind concrete formals and
retype local bindings before calling the same engine. These distinct contexts preserve
the bulk result's generic observations without merging concrete substitutions into
canonical source facts.

Raw library-body navigation retains written source typing and never changes canonical
source selections or starts the bulk closure. Public owned producers and borrowed model
views use the same proof algorithms. `analyze_model_with_facts` runs fresh diagnostic
policy through an existing owner. Queries can retain an `Rc<ModelFacts>` through
`evaluate_with_facts`; each evaluation charges its own fact scan and result work,
including a warm owner, and checks the exact root bytes and BOM. The last session or
result owner releases the cached facts.

Salsa is a conditional later choice, replacing the blanket rejection. Production
adoption is outside this bundle. It needs measured repeated-edit/recomputation
cost after ordinary demand/reuse, a concrete invalidation/identity design and a
separately scoped decision that accounts for integration cost. No speedup claim
follows from policy or fewer inspected nodes alone.

## Salsa decision: 2026-10-12

Defer Salsa. The current consumers can reuse facts through an immutable
`ModelFacts` owner, and the bounded replay confirms that this reuse works. There
is no approved consumer that retains a mutable model across edits, and no
measured cross-edit recomputation cost to justify revision tracking. This
decision leaves the semantic contract and production dependencies unchanged.

Three mechanisms need separate justification. Demand avoids work that a selected
consumer does not request. Memoization reuses a requested result while its inputs
stay unchanged. Incrementality validates dependencies and reuses unaffected
results after an input changes. Zincite implements the first two within a model
owner. [Salsa's overview](https://salsa-rs.github.io/salsa/overview.html) describes
tracked deterministic computations, memoized results and dependency checking
across revisions; its inputs must represent everything that can affect a result.

The [post-base-121 measurements](../scripts/batch-performance.md#base-122-demand-and-reuse-observations)
use the existing 84-byte model and 115-byte installed-library model at
`0dbc2781c7a309668e932308a8e11b76d0876dc8`. Fresh CLI medians were 70.274 and
159.977 ms, with maximum child RSS of 28.938 and 41.438 MiB. In three independent
API processes per case, ten requests through the retained owner had pooled
medians of 0.800 and 0.363 ms; ten fresh-owner controls against the same loaded
context had medians of 42.236 and 128.622 ms. Loading is measured separately.
Every complete first/warm/fresh-owner result agreed, including ordered findings,
errors, limitations, fixes and rule outcomes. Repeated public fact requests
returned the same borrowed objects. This is observed result reuse, not a count
of producer executions or cached invocation proofs.

The separate allocation observation retained 25,191,904 and 36,002,418 live
bytes above its pre-load baseline after first analysis. Ten warm results, each
dropped before the next request, added no retained bytes. Dropping the owner
released its facts; dropping the context returned live allocation bytes to the
baseline. OS RSS is a process high-water measure and does not establish that
resident pages fall after a drop. The replay covers two small cases, not a
corpus, mutable session or matched Salsa implementation.

### Fact boundaries and possible query keys

`ModelFacts` has ordinary `OnceCell` slots for one borrowed immutable context.
Its source and proof views remain distinct. A future database would still need
the following semantic boundaries, even if Salsa managed the memo tables.

| Facts or policy | Current reuse boundary | Required boundary in a future database |
| --- | --- | --- |
| Source/core: bindings, source callable selections, types and instantiations | One model's sources, include closure and availability | Stable file, declaration and root identity, with source bytes and resolution dependencies; context-independent parsing may be shared across roots |
| Local domains/numeric facts | Source facts, checked arithmetic, direct definitions and symbolic identities | Root-local dependencies and the relevant declaration/expression identity; no invented parameter values |
| Optional, guarded and iteration facts | Named per-model results retain scoped presence, definedness, membership and traversal | Scope, substitutions, generators and guards wherever they affect the requested result; equal array sizes do not establish equal indices |
| Direct and callable definitions | Direct definitions stay separate from finite callable outputs and combined cycle marking | Preserve enforcement, coverage, sibling safety and root dependency anchors |
| Concrete callable-body typing | `DeclarationId` plus the exact formal `TypeInst` tuple in `CallablePreparation` | Stable declaration identity plus that tuple and its tracked source/type dependencies; actuals, mapped defaults/captures, lexical guards, membership and output publication remain invocation work |
| Search | Per-root seeds, definitions, callable outputs and supported complete views | Root-specific search annotations and dependency closure; a shared library path cannot identify a model's coverage |
| Rule and fix policy | `analyze_model_with_facts` recreates diagnostics over reused facts | Keep selected rules, parameters, suppressions, applicability, ordering and safe-edit obligations explicit if policy results are ever memoized |

Body navigation adds written source typing to its returned observation. Bulk
`proof_callables` is an explicit compatibility request. Neither is interchangeable
with canonical source facts or an invocation's bound, definedness or output
proof. A declaration-only key would merge distinct substitutions and caller
contexts. Salsa cannot supply the missing semantic key.

### Identity, dependencies and lifetime

`FileId` and `DeclarationId` are indices in one loaded context; CST byte positions
can move after an edit. They are unsuitable as unchanged cross-revision keys.
`load_model` retains exact source bytes and BOM offsets, canonical file identity
with the first readable spelling for diagnostics, include edges, and standard/user
availability. Resolution uses the including directory, ordered include directories
and the configured standard library. These inputs also determine missing,
unreadable and cyclic-dependency errors. Each root loads independently.

An immutable owner needs no invalidation. Changed source or include configuration
requires a new context and owner. A retained database would need tracked source
bytes, resolution/configuration inputs and dependency failures, plus an explicit
identity design for roots, files and declarations. The two observations loaded
382 file instances but only 204 unique canonical files. That identifies repeated
library loading in these roots; it measures no benefit from sharing it and does
not justify sharing root binding, type or proof results.

Salsa would automate memoization and dependency verification once those inputs
and computations were represented. Its
[accumulators](https://salsa-rs.github.io/salsa/overview.html#accumulators) can
collect diagnostics separately from query values. Zincite would still own
MiniZinc semantics, conservative Unknown/Unsupported distinctions, source ranges,
diagnostic order, suppressions, rule execution outcomes and safe-fix obligations.
Queries using `evaluate_with_facts` still check exact root bytes/BOM and charge
each evaluation's scan and result budgets; a warm fact owner does not waive them.
Their retained `Rc<ModelFacts>` also keeps facts alive until the last session or
result owner drops. No additional query measurement was needed for this decision.

[Salsa's cycle handling](https://salsa-rs.github.io/salsa/cycles.html) defaults to
a panic for a query cycle. Fallbacks and fixed-point recovery require explicit
domain policy; iterative recovery requires deterministic monotone operations
over a finite-height domain. Zincite already owns finite output-then-boundary
stabilization, concrete-instantiation recursion barriers and definition cycle
marking. Keep those algorithms and their refusal/publication rules. Adopting a
query engine would not authorize generic recursive interpretation.

[Salsa's tuning guidance](https://salsa-rs.github.io/salsa/tuning.html) says memo
values are retained without an eviction limit by default. LRU can evict values,
but query keys and dependency metadata remain; input identities last until the
database drops. A retained database therefore needs a memory policy beyond a
memo-value capacity. The current owner limits retention by model lifetime;
fresh CLI processes retain no facts for the next invocation.

The five primary sources were checked on 2026-10-12. The moving
[ty inference source](https://github.com/astral-sh/ruff/blob/main/crates/ty_python_semantic/src/types/infer.rs)
uses selected scope/expression queries, explicit type contexts and domain-specific
cycle normalization. It illustrates query granularity and contextual key design.
The [rustc query guide](https://rustc-dev-guide.rust-lang.org/query.html) separately
describes demand, memoization, dependency discovery and incremental facilities.
These are architecture references, not Zincite performance comparisons.

### Reconsideration and cost

No Salsa prototype is warranted here. Local code and the public-API observations
settle unchanged-input reuse and owner lifetime; the primary documentation
explains revision tracking, cycle behavior and retention. There is no specific
unresolved adoption question that requires a disposable implementation. Without
a matched prototype, this decision makes no Salsa speedup or overhead claim.

Reconsider when an approved consumer needs a retained mutable session across
edits, or a bounded profile identifies material repeated source/fact work across
actual roots after ordinary demand and reuse. Define stable identity and
include/configuration dependencies first. Then compare identical ordinary-cache
and Salsa workloads, with affected-result correctness, unaffected-result reuse,
cycle compatibility and memory lifetime inside an agreed latency/memory budget.

Adoption would require input/identity design, conversion of the named fact
dependencies, contextual key design, diagnostic collection and ordering,
cycle-policy compatibility, public API/lifetime changes and retention limits.
That is a separately scoped migration with focused validation. The current
measurement provides no reason to pay that cost now.

## Small baseline for this bundle

Historical policy baseline: `6b497f9caa46276fd9c62016371350a909a5b08e`
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

Base-115 changes only admission-related outcomes. Typed opaque integer calls
(such as `custom(1)`), Boolean calls and filters, optional calls, and opaque
collection sources now retain Unknown. Selected user/native `index_set` bodies,
written optional-matrix constructors, literal regular native bodies, sliding-sum
primitive bodies, and changed GCC count/equality bodies no longer require their
former admission shapes. Their selected bounded analyses may complete without a
finding. Unknown supplies no value, presence, extent, coverage or fix proof;
opaque Boolean calls and evaluated generator bodies retain a possible abort
barrier. Default cannot capture that barrier. Ambiguity, unavailable required
typing, closed argument/source errors and known-empty strict extrema still veto
completion or dependent proofs. Numeric chains, scoped guards, capacity/presence,
and definition/search guarantees keep their existing controls. Known enclosing
index/division obligations remain reportable, while opaque unused-generator
bodies acquire no fix eligibility. Historical captures above remain unchanged.

Base-116 moves initializer and domain uncertainty to their semantic owners.
Typed nonoptional integer selections and array comprehensions now publish
DefinitionSafety::Unknown without an initialized-body admission certificate.
Union comprehension operands publish Domain::Unknown while preserving named
identity, both union operands and closed minima. Definition enforcement,
coverage, dependency anchors and cycles retain their separate meaning. Exact
traversal membership can still establish Supported safety.

Former local selection/comprehension and selected written primitive-body
admission refusals can therefore become Completed with Unknown. Analysis no
longer changes definitions by matching diagnostic text or initializer syntax;
constant-variable alone no longer requests callable analysis. Unknown cannot
satisfy constant advice, strict output dependencies or semantic fix eligibility.
Known arithmetic/source errors, empty strict extrema, out-of-axis selections,
unavailable typing, invalid selections and relevant failed siblings remain
Unsupported with their source locations.

Callable boundaries retain an explicit Unknown or Unsupported status and a
Source or Output operation. Source opacity can veto dependent unbounded advice;
a stronger output refusal does not erase advice for understood conditional,
partial or unanchored definitions. Existing resolved references and
actual/default/capture mapping delimit affected targets without inversion.
An unavailable attribution retains the necessary owner/root veto. Relevant
opaque aggregates, relations and enclosing lets now retain Unknown search
coverage instead of a missing-search warning. Independent known uncovered
decisions and simple unbounded declarations still warn. Explicit scalar and
complete per-element ranges and all retained Scalar/WholeArray proof outputs
remain unchanged.

Base-117 separates possible references from finite output dependencies in search.
A selected relation without an output summary retains its written references,
mapped through actuals, defaults and captures. Those references turn affected
uncovered or partial coverage into Unknown after positive closure; they never
seed coverage or establish a relation direction. Unavailable attribution withholds
wider negative claims. Safely localized uncertainty preserves independent known
coverage and located missing-search advice.

Search no longer requires inspected-local certificates or a separate model-value
fresh-local admission walk. Scoped decisions without retained definitions remain
Unknown. Base-121 removes the unused public `inspected_locals` field, its
observation collection and publication. Private-local identity checks still
prevent private outputs from escaping their scope.

Base-121 also removes the regular/regexp, optional-matrix, row/fresh-local and
selected union-body interpreters that only licensed Unknown. Sliding-sum and GCC
specialized dispatch was already absent. Collection, conditional/set and native
source routes now use selected typing, the existing safety/domain result and
closed-error observations. They do not recognize library recurrence shapes or
execute excluded implementations to establish an admission certificate.

The retained shared source families have concrete consumers:

| Family | Current consumers and guarantee | Existing public witnesses |
| --- | --- | --- |
| Direct/indexed and initialized safety | `definitions`, `unbounded_variable`, `array_indices`, `guarded`, `domains` and `search` retain strict source dependencies, scoped membership, complete definitions and checked search views. Supported is required before a positive proof. | `whole_definitions_retain_identity_ranges_dependencies_and_item_suppression`; `full_index_range_membership_under_let_and_conjunction_supplies_bounds`; `local_guards_preserve_obligations_identity_and_boolean_ranges` |
| Length, index extrema and native identity | `array_indices`, `domains` and callable traversal/output checks retain selected signatures and exact source identity. Conditional/set/extremum routes supply typed uncertainty and closed-error vetoes; they supply no count or bound by themselves. | `named_domains_and_symbolic_bounds_retain_identity_and_precise_advice`; `let_integer_equalities_inspect_symbolic_array_extremum_bounds`; `native_integer_domain_sources_preserve_unknown_and_errors` |
| Checked reshape/source views | `definitions` and `search` retain matching axes and source identity for complete traversal and coverage. | `typed_searches_require_present_elements_matching_indices_and_native_views`; `callable_outputs_keep_selected_direction_defaults_captures_and_array_extent` |
| Closed source, type and argument errors | `definitions`, `domains`, `guarded`, `optional`, `iteration` and callable invocation checks retain closed arithmetic failures, false assertions, strict empty extrema, invalid nominal types and out-of-axis selections. Formal/actual correspondence retains the caller's source error. | `assertions_inside_opaque_selections_retain_only_known_failures`; `typed_opaque_calls_keep_unknown_parents_hazards_and_abort_barriers`; `uncertain_private_integer_arrays_inspect_sources_without_output_proof` |
| Narrow standard metadata and primitives | Callable output checks retain selected metadata/signature checks for literal-result elements and other finite outputs. Strict dependency and complete-owner checks still reject failed siblings. | `full_weighted_bodies_require_all_sibling_sources_before_load_outputs`; `constructed_weighted_rows_require_same_source_and_complete_output_traversal`; `selected_output_requests_leave_unrelated_library_relations_opaque` |

An admission-only library body shape or opaque initializer may now become
Completed with Unknown. This includes changed product recurrences, written
fixedness/domain-reflection bodies, opaque Boolean/numeric aliases, optional
metadata operands and unrequested native set/union bodies. Unknown establishes
no value, totality, output, coverage or fix eligibility. A direct facts request
can observe an error in a requested library body while a bounded rule that never
requests that body retains opacity. Errors found by required source, typing or
proof work still veto dependent publication, including failed siblings and closed
actuals. Error diagnostics may identify the owning invocation when the cause lies
in its callee or actual; their located error remains available.

Callable analysis still runs its finite sequence: build lookup indexes, collect
model clauses, discover concrete output candidates, stabilize outputs, stabilize
typed boundaries, publish definitions and mark cycles. Selected identity and
formal tuples, defaults/captures, complete-owner checks and recursion barriers
remain prerequisites for the retained outputs. Closed-error observations supply
no safety, value, output or coverage on success.

Guarded default evaluation records an `AbortingDefault` limitation kind at the
owning branch. `partial_expression` reads that kind; the rendered default-abort
reason and location remain diagnostic payload. The historical message-bridge
draft in the callable plan is superseded, and its old evidence remains unchanged.

This baseline is a small bundle control set, not a substitute for the existing
full catalogue and parsing/formatting acceptance evidence.

The [base-123 acceptance](../scripts/semantic-corpus-checkpoint.md#base-123-bounded-semantic-acceptance-2026-10-12)
runs the retained workspace controls, three originals, public library/native
comparisons and reordered shared-owner requests. It reuses base-122 measurements
after checking the unchanged production paths and retained raw/input hashes.
The dated Salsa deferral above remains in force. Future extensions follow the
[brief's consumer and evidence rule](../.zdev/base/brief.md#interpretation-before-policy).
