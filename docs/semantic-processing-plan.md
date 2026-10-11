# Bounded semantic processing

Zincite provides a standalone, lint-oriented semantic front end for MiniZinc
2.10.1. It retains complete target syntax and source-preserving processing
obligations while promising bounded semantic facts and detection families.
Valid syntax does not require unrestricted interpretation of the model.

The [brief](../.zdev/base/brief.md) is the product authority. This policy adopts
the base-097 support boundary and guides the approved base-098, base-099 and
base-100 follow-ups. It changes acceptance criteria, not existing implementation
or historical execution counts. None of those tasks or the acceptance tasks is
complete merely because the policy is adopted.

## Research basis

Source research on 2026-10-11 supports separating reusable facts from individual
lint proofs. These comparisons guide design; they establish no Zincite runtime,
conformance, coverage or performance result.

| Reference | Observed boundary | Zincite decision |
| --- | --- | --- |
| Clippy | Its [driver](https://github.com/rust-lang/rust-clippy/blob/master/src/driver.rs) runs inside rustc; its [constant helper](https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/consts.rs) still folds only bounded expressions. These are dated moving-branch observations, without an exact Clippy revision. | Separate facts and lint policy. Do not invoke MiniZinc for ordinary linting or rebuild a compiler to obtain every fact. |
| Ruff | Its [semantic model](https://github.com/astral-sh/ruff/blob/a5aee680b11d2190fbb6a72008047ae281dd2d18/crates/ruff_python_semantic/src/model.rs) supplies scopes, bindings and references; its [small type helper](https://github.com/astral-sh/ruff/blob/a5aee680b11d2190fbb6a72008047ae281dd2d18/crates/ruff_python_semantic/src/analyze/type_inference.rs) admits Unknown for names and calls. Other rules have their own narrow helpers. | Reuse identity and type facts, with bounded analyses for named consumers. Ruff is the closest product comparison. |
| ty | Its [inference architecture](https://github.com/astral-sh/ruff/blob/a5aee680b11d2190fbb6a72008047ae281dd2d18/crates/ty_python_semantic/src/types/infer.rs) serves broad type checking and cross-file inference. | Borrow explicit fact/error boundaries. Do not adopt gradual Any as proof, Python flow semantics or an incremental database. |
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

Preserve the successful bounded analyses and their owners:

| Owner | Supported fact and proof barrier |
| --- | --- |
| `domains` | Literal/simple symbolic domains, checked arithmetic and intervals. Unknown bounds supply no invented members or extent. |
| `guarded` and `optional` | Scoped truth, raw/effective definedness and presence. Preserve MiniZinc relational partiality; implication is not imperative short-circuit execution. |
| `iteration` | Membership, cardinality, dependencies and traversal coverage where established. Capacity differs from present optional elements; equal sizes do not prove equal indices. |
| `definitions` and `value_safety` | Direct initializers, enforced equalities and complete matching unfiltered universal definitions, with safety and dependency anchors. One element or conditional equality does not define a whole value. |
| `callable_definitions` | Existing finite selected-body output summaries and transparent wrappers, with concrete substitutions, lexical context and complete-owner publication. Preserve sibling failure and recursion barriers. |
| `search` and search coverage | Closure from actual searched decisions through supported complete definitions, standard search/view families and aliases. Cycles and opaque relations supply no output proof. |

Signature facts support navigation, decision-use advice and standard-call
recognition without proving what a body computes. They establish neither bounds,
presence, totality, output definitions nor search coverage. Constant-variable
advice and safe fixes keep their stronger safety obligations.

Inspect written user bodies and bodies needed to resolve source references and
local types. Unused-declaration needs relevant possible dependency edges, not
execution. Reachable uncertain edges can block an unused conclusion. Inspect a
selected body for a bound, definition, definedness, traversal or search property
only when that named consumer needs it. Known source/body errors stay visible
and veto dependent proofs. No call is trusted by spelling alone; bodyless calls
and native endpoints are not universally total.

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
first-order support. Freeze new specialized relation-body inspectors; preserve
existing successful inspectors and proof cases.

For numeric, totality, definition and search consumers these exclusions can block
bounds, definedness, complete outputs or coverage. For unused-declaration they
can block a conclusion only when the required reference/dependency edges cannot
be established; absent output proofs alone do not block static references. For
data reduction, general computed enum-index expression evaluation is excluded;
unresolved affected accesses keep the candidate incomplete. These exclusions do
not waive common structural types, source identity or first-order call selection.

Flattening, solving and whole-language validation remain outside the product.
Do not add instance expansion proportional to data, a full HIR/THIR, a fact
registry, persistent caches or a general fixed-point engine. Existing finite
output/search closures do not authorize a general evaluator. Unsupported
compiler extensions require explicit documentation, never opaque syntax
acceptance. Complete target syntax remains required.

## Outcomes and acceptance

Classify the operation required by each selected rule, not an entire fact bundle.
Keep `AnalysisResult` and `RuleOutcome`; use the following distinctions in plain
support documentation and located limitations.

| Situation | Handling and acceptance |
| --- | --- |
| Supported analysis performed, property unproved | Retain Unknown. Completed with no finding is valid when the promised bounded analysis admits this result. It proves no value or property. |
| Documented excluded operation blocks a selected rule | Retain a located limitation and Limited outcome, alongside independent findings. Acceptance may accept the named exclusion; it cannot count it as Completed. |
| Promised support missing or incorrect | Record a bounded gap/defect and retain Limited where analysis cannot finish. It blocks acceptance within promised scope. Difficulty does not create an exclusion. |
| Actual input/dependency failure | Preserve input errors and availability classifications, including missing/unreadable/cyclic includes. Missing parameter values alone are symbolic uncertainty. |
| Inapplicable context | Use Inapplicable for actual context restrictions, such as export reachability in a fragment or model analysis on standalone data. Difficult complete models are not inapplicable. |
| Resource cutoff or missing observation | Retain the resource failure or Unobserved report row. Neither is Completed or a semantic exclusion. |

Keep exact selected IDs and every original root, with per-root/per-rule
Completed, Limited, Inapplicable and NotRun partitions and explicit invalid,
unavailable and Unobserved rows. Report accepted exclusions separately from
promised gaps without renaming or removing Limited rows. Acceptance requires no
unresolved failures within promised support, complete valid-input parsing and
formatting, source preservation, complete accounting and the existing measured
performance obligations. Crashes, invalid target syntax, resource cutoffs and
missing observations cannot be waived to improve totals. Missing corpora or
dependencies prevent a full-coverage claim.

Preserve lint stderr limitations, source/suppression contracts and exits 0/1/2:
0 without warnings, 1 with unsuppressed warnings, 2 for actual errors with
precedence. A limitation alone does not change 0/1. Retain compiler pre-checks,
formatted-instance checks, source-preservation comparisons and final integration
under the brief; compiler validity alone does not prove a semantic fact.

## Approved follow-ups and stopping points

Base-098 classifies every retained Connect site by family, fact and consumer,
reconciling later controls and Workforce evidence. It uses the four support
categories above and proposes only witnessed promised-scope repairs. Historical
nineteen-site observations are neither current counts nor nineteen defects.
If evidence establishes only documented exclusions, propose no semantic repair.

Base-099 checks the source-signature/body demand boundary for decision operator,
generator and condition consumers. Use existing `CallableFacts` and ordinary
functions: bindings, source signatures/types/calls, instantiations, then simple
checks. Mixed/deep selections compute one appropriate body-aware result and keep
all current proof barriers. Retain written user-body inspection and any typing
needed by signatures. If current paths already meet the boundary, finish with
evidence instead of extracting a helper. Stop before broader type/body repairs;
claim no performance improvement without matched measurements.

Base-100 repairs normal-library semantic query admission independently. Bound
model scan/fact construction separately from result collection/expansion, retain
finite pre-allocation protection and actual stage-work accounting, and validate
small installed-library navigation plus finite refusals through both commands.
Do not merely raise a collection limit or add a budget subsystem.

Base-083 retains diagnostic scaling/grouping review, including the retained
generated Project Planning case, confirmed expanded-analysis cost with matched
outcomes and bounded growth/allocation/native evidence, and final complete
14/current-all accounting (now 28 IDs), preserving historical 26-ID evidence.
Base-050 retains full corpus inventory, compiler pre-checks,
source preservation and final release performance/integration requirements.
Neither classifications nor bounded controls substitute for final acceptance.
The initializer-adaptation query and whole-owner extraction remain deferred;
they are optional local cleanup when a witnessed repair benefits, not new
prerequisites. A later semantic repair needs its own concrete promised-scope
witness, owner, proof barrier and proportionate checks.

Base-092 accepts the wedding reductions of `test_data/small.dzn` and
`instances/mixed/instance-guests-150-topics-10-tables-mixed.dzn` under the brief's
local seating model directory as incomplete best-effort previews while computed
dependencies remain unresolved. Keep status 1, located
unresolved dependencies and refusal of in-place writes. Check each original and
preview with MiniZinc instance checks and direct retained/removed-data inspection.
A separate small complete structured-reduction control through both command
forms must exercise the complete write path. The real installed-library query
resource failure remains a repair obligation under base-100. Keep privacy,
ordering/source preservation and measured query performance requirements. No
general enum-expression evaluator or completion claim for the real previews is
required or authorized.
