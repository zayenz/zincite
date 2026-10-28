# Interactive FlatZinc checker

## Objective

Build a reusable Rust checker that takes a prepared FlatZinc model and supplied
variable values, derives supported auxiliary values, and reports constraint
violations. Run the same checking code natively and in WebAssembly so an
application can check manual changes locally using its optimisation model.

The application translates its graphical state into enough primary variable
values to determine one full candidate assignment. The checker completes that
assignment and evaluates every constraint, including when the candidate is
infeasible. Users may keep and edit infeasible states while the application
shows all violations. Insufficient input is also a useful report outcome:
return unresolved values and blocked checks alongside any established
violations. The application decides when and how to expose that information.

The starting point is *A FlatZinc Checker in Rust for WebAssembly*, the unbuilt
master's thesis proposal dated 5 January 2021 supplied in this conversation.
Its mandatory features are native/Wasm execution, parsing a checker-specific
FlatZinc dialect, Serde serialization, completion of sufficiently specified
assignments, and human- and machine-readable violations using annotations.
Incremental checking, minimal input sets and explanations are research extensions.
The author's clarifying email supplied in this conversation settles the usage
and reporting requirements above; use it to interpret the proposal's references
to partial assignments and satisfiability. The user's subsequent clarification
settles that insufficient input is a report, not an error.
Read [the source and repository notes](background/design-context.md) before
compiler integration, lowering, report design or extending predicate coverage.
Read [the compilation contract](background/compilation-contract.md) before
implementing the initial checker dialect, definition evaluation or compiler
library. It records the verified two-job recipe, emitted predicates, metadata,
domain changes and candidate calculations.

Observable success is a small labelled MiniZinc model compiled once, prepared
once, and checked repeatedly after editing its decisions. Native and browser
checks agree on full feasible and infeasible candidates, and report insufficient
input explicitly. A serialized model can be loaded and checked
in the browser without shipping the source parser or
MiniZinc compiler. Current support covers the mathematical FlatZinc operations
emitted by the two pinned library profiles, text and official JSON preparation,
and the numeric and compiler boundaries documented in the checker guide.
External-execution and search-history hooks receive located diagnostics.

## Boundaries and settled direction

- Use MiniZinc 2.10.1 as the initial compiler and language baseline. MiniZinc
  performs flattening outside the checker; checking never starts a compiler,
  solver or network request.
- Add `zincite-check` as a library crate with a thin binary and root
  `zincite check` dispatch, following the existing workspace pattern. Its
  concrete consumer is repeated checking, independent of formatting and linting.
- Reuse `zincite-syntax` for parsing and source ranges. Validate the accepted
  FlatZinc subset during lowering: accepted MiniZinc syntax is not automatically
  valid checker input. Extend shared syntax only for demonstrated FlatZinc gaps.
- Keep preparation and execution separate within this crate. Execution owns
  simple typed values, variables, domains, constraints and definition directions.
  It does not depend on linter facts, the query language or a general MiniZinc
  interpreter. Keep syntax preparation optional for a parser-free Wasm build.
- Support Boolean, checked i64, compact finite integer-set and finite binary64
  float values with scalar variables, arrays, aliases and domains. The full
  mathematical surface of both pinned library profiles is implemented. Numeric
  limits, source compiler failures and external hooks are explicit; unknown
  predicates and malformed input fail preparation.
- Use fixed-width checked integers consistently across native and Wasm targets.
  Specify overflow, division by zero and signed division/remainder behavior from
  the MiniZinc builtins; never inherit target-dependent `usize` arithmetic.
- Preserve labels and available source annotations, with a FlatZinc location
  and constraint ordinal as a fallback. Ordinals identify constraints within
  one prepared model, not across recompilation. Keep individual flattened
  violations even when several share the same source label.
- Make structured violations the library's primary output. Preserve
  developer-controlled constraint identifiers and payloads through compilation
  so an application can associate a violation with its own entities and choose
  its presentation. Text messages are a rendering of structured records.
- Search annotations can be ignored after recognizing them as search metadata.
  Unknown predicates, malformed definition annotations and unknown annotations
  that affect checking semantics require an explicit diagnostic.
- Keep supplied assignments immutable. Start each check with fresh values;
  reuse the prepared model, not values derived for a previous assignment.
- Follow the [base brief](../base/brief.md) for shared syntax and existing tool
  behavior. This area adds an executable checker; formatter, linter and query
  commands keep their existing contracts. Keep all work local. Publication,
  release and deployment are outside this plan.

## Checking contract

Preparation resolves names and aliases, checks types, arities, domains and array
shapes, selects supported builtin operations and definition directions, and
records report locations. It either produces an executable model or located
errors; unsupported operations cannot be silently dropped.

The conceptual public sequence is `prepare(source) -> PreparedModel`, followed
by `model.check(assignment) -> CheckReport`. Preparation and assignment errors
are explicit error results. Serde loading validates the serialized model
before exposing the same checking operation. Exact Rust signatures can follow
the first implementation rather than introducing an interface framework now.

The application developer owns the mapping between its graphical state and
the primary variables, for example a shift's start time and `start[37] = 55`.
The checker owns completion and constraint evaluation. It is a reusable component;
the schedule example demonstrates that boundary without making the runtime
schedule-specific or exposing FlatZinc concepts in the end-user interface.

Assignments name FlatZinc variables or declared arrays. Validate unknown names,
duplicate entries, types, array lengths and contradictory assignments through
aliases. A correctly typed value outside its declared domain is a reported
violation. A supplied value for a defined variable is checked against its
definition rather than overwritten. Constants and singleton domains may establish
unsupplied values. A functional result outside its domain is still available
for downstream checks; report its domain violation without clipping the value
or abandoning the rest of the candidate.

MiniZinc's `computed_domain` marks a domain computed for solver convenience,
which does not impose a problem constraint. Handle this known semantic annotation
explicitly: its marked domain supplies neither a violation nor a singleton
default. Logical domains on other aliases still apply, and fixed initializers
keep their existing meaning. Preserve this distinction through preparation and
validated loading; no general propagation or annotation interpreter is needed.

Derive a value only through a supported deterministic definition, initially
aliases and verified `defines_var` directions. For example, known `x` and `y`
can define `z` through `int_plus(x, y, z)`. A definition annotation is not
permission to invert any arbitrary relation. A linear definition needs a
supported unique output calculation, including exact divisibility where needed.
Reification defines its Boolean result; half reification does not establish
equivalence. Every defining relation is also a constraint to check.

Use a simple dependency worklist to evaluate ready definitions until no new
values become available. A missing input, a cycle without enough supplied
values, or an unsupported derivation direction leaves values unresolved. A cycle
whose values are supplied can still have all its relations checked. The initial
implementation uses functional determination. Propagation is an admissible
later completion technique if needed: it must establish unique values without
search or arbitrary choices, and must preserve reporting for infeasible
candidates. There is no requirement for a general propagation engine initially.

Check every constraint whose required values are available, and report all
violations in source order. Do not stop at the first failed check or use a
failed constraint to discard the candidate and skip other checks. Domain
failures and undefined concrete operations such as division by zero are also
visible. A blocked constraint
must be distinguishable from a false one. The report carries independent
completeness information so a known violation can coexist with unresolved values:

| Result | Meaning |
| --- | --- |
| Valid | Every model variable is resolved and every domain and constraint check passed. |
| Invalid | A violation was found; completeness states whether other values or checks are unresolved. |
| Incomplete | No violation was established, but some values or checks are unresolved. This is a normal report outcome. |

An incomplete report includes findings already established and identifies what
is unresolved. It is useful feedback without claiming that all checks
completed or that a satisfying completion exists. Insufficient input does not
make the check call fail; malformed input and execution failures are errors.
The component does not prove uniqueness among all possible solutions:
supplied primary values and supported derivations must
determine one candidate without choosing between alternatives.
Checking an optimisation model reports an available objective value, never
optimality. A complete valid result establishes feasibility of the supported
compiled model. Equivalence to the original editable model depends on the
verified compilation recipe below.

Each violation has a machine-readable kind, a constraint reference, the
application identifier/payload when supplied, and typed relevant arguments or
variable references/values. Keep labels and locations as additional context.
The application can highlight the affected shift or other entity, translate
messages, and control presentation without parsing checker-written prose.
Include unresolved-variable and blocked-check details as structured completeness
information, separate from violations. These requirements need a small concrete
record format, not an explanation engine or application-specific checks written
in a second model.

## Implementation sequence

The imported tasks in [TASKS.md](TASKS.md) track these increments. Their records
give current status and validation; the sequence below describes their scope.

1. **Establish the compilation contract.** Compile a tiny labelled model with
   arithmetic-derived values, reification and an editable decision array. Inspect
   FlatZinc from a checker-specific library/configuration, starting with `-O0`
   and path annotations enabled. Establish which original decisions survive,
   how callers address them, which predicates occur, and how application identifiers,
   payloads and labels survive decomposition. Check a valid candidate and an
   infeasible candidate that still derives all auxiliary values and exposes
   multiple violations. Check missing primary input as an incomplete report.
   Record the exact repeatable compiler invocation and first predicate list.
   Do not proceed on an assumption that arbitrary solver output is suitable.
2. **Deliver one complete native check.** Add the crate, library API, both CLI
   forms, FlatZinc lowering and literal assignment input. For the small admitted
   Boolean/integer dialect, check complete assignments, domains and multiple
   structured violations with application metadata. Reject unsupported constructs
   explicitly. Use `.dzn` assignment syntax with literal values, not arbitrary
   expression evaluation.
   Provide text and JSON reports; exit 0 for complete valid candidates, 1 for
   invalid or incomplete reports, and 2 for input, preparation or execution
   errors. JSON includes findings and completeness information; insufficient
   input alone does not produce an error diagnostic or error exit status.
3. **Complete auxiliary values.** Add deterministic definition evaluation and
   the dependency worklist. Cover aliases, supported arithmetic, linear
   definitions, Boolean operations, reified relations and element access as
   required by increment 1. Demonstrate one decision assignment deriving a
   complete candidate, an infeasible candidate whose domain failures do not
   suppress downstream violations, conflicting supplied derived values, and an
   unresolved cycle described in an incomplete report.
4. **Make compilation useful for applications.** Package the proven custom
   MiniZinc library and configuration. Add a small scheduling or allocation
   example whose graphical state determines the primary variables, including
   infeasible states. Preserve identifiers/payloads sufficient to connect each
   violation to the affected entities. Document the exact supported predicates
   and definition directions.
   Ensure compiler rewrites, domain tightening, elimination and optional
   symmetry breaking do not silently change the advertised editing contract.
   Check all emitted predicates against the runtime's coverage. Keep compiling
   an explicit offline step; a native compiler wrapper is unnecessary initially.
5. **Serialize prepared models.** Add Serde round trips and loading checks, with
   a small format version and explicit rejection of incompatible versions or
   invalid references/types. Carry values, executable operations and reporting
   metadata, not the CST. Use an exact integer representation at the JavaScript
   boundary, such as decimal strings, so values beyond 2^53 are not rounded.
   Provide a native preparation/export command and a load-and-check example.
6. **Run the same model in a browser.** Add thin Wasm bindings and a local
   example that loads the prepared artifact and lets users edit the graphical
   state, including infeasible states. Translate structured violations into
   entity-specific feedback. Demonstrate complete checking during ordinary edits
   and an incomplete report when primary input is omitted. The application
   controls how completeness information is shown.
   Keep the runtime free of filesystem and compiler requirements;
   demonstrate that the parser-free build excludes `zincite-syntax`. Use a
   simple page and current Rust/Wasm tooling, without an application framework.
7. **Expand the admitted dialect where useful.** Add finite integer sets and
   their required relations, and selected global predicates where keeping
   them intact improves reports or avoids a measured decomposition cost. Choose
   the first globals from the example's emitted constraints rather than attempting
   a catalogue. Keep the coverage document and checker library aligned. Float
   support, if wanted, first needs an explicit precision/tolerance contract.
8. **Evaluate interactive performance.** Measure preparation, serialized load,
   repeated checking and Wasm boundary/report cost separately on the example
   and a few growing inputs. Target p95 below 100 ms for a named ordinary
   interactive example with a prepared model loaded once on the user's M1 Max;
   record browser, build, input sizes, sample counts and payload/Wasm sizes.
   This is a checker target, separate from formatter save latency. Keep larger
   cases as scaling measurements. Profile a concrete missed target before
   adding incremental execution or changing evaluation order.

Each increment should leave a demonstrable public path. The first compiler
experiment determines the exact predicate scope for the next two increments;
it is not an open-ended research campaign or a prerequisite to rebuilding the
existing parser. Serialization follows a working runtime, and browser work
follows serialization. Set/global expansion can follow the native path without
blocking the first browser example.

## Interactive measurement result

The [2026-10-28 engineering report](../../scripts/checker-performance.md) records
fresh release native and real-Chrome measurements on the user's M1 Max. The
two-job workshop model used 60 individual checks after ten warmups over feasible,
infeasible, omitted, incomplete-invalid and recovered states. Native checking p95
was 0.008834 ms and browser assignment-to-parsed-report p95 was 0.10 ms, below the
100 ms ordinary-example target. Preparation, serialized load, normalized Wasm/JSON
components and memory scaling for 10/100 copies inside one model are separate.
The final named run is `m1-max-release-2026-10-28-full-acceptance-v2`, with four
completed records, no failures and no unfinished records. The report records
build and library identities, artifact hashes, scope and raw-result location.

The first run exposed invalid item order in the existing scaling generator;
the corrected generator groups parameters, variables and constraints. A small
pilot verified all sizes and unchanged completed records on resume. The failed
run is saved separately. No runtime latency target was missed, and no checker
optimization was needed. Actual Initial plan, invalid, recovered and omitted
interactions in the original graphical example also passed, including recovery
after omission. Those interactions are separate from timing, which excludes DOM
rendering.

## Full FlatZinc and library support

Current support covers candidate checking for text FlatZinc and FlatZinc-JSON,
standard constraints emitted from MiniZinc's standard library, and mathematical
predicates exposed by the pinned Gecode MiniZinc library. The final acceptance
audit pairs exact signatures and include closures with family execution tests,
compiled controls and public-path agreement. Native, serialized and parser-free
Wasm entry points share structured findings and independent completeness.

Read the [coverage inventory](../../docs/checker-coverage.md) before extending a
predicate family or either compiler profile. It records exact source signatures,
aliases, current support, task ownership and include gaps, with the command that
checks the table against both pinned source trees.

Use these fixed upstream sources for this expansion:

- MiniZinc **2.10.1**, tag commit
  `c3448c48e7f05f13805616cfd1b9e94972eef58c`:
  [standard library](https://github.com/MiniZinc/libminizinc/tree/c3448c48e7f05f13805616cfd1b9e94972eef58c/share/minizinc/std),
  especially `flatzinc_builtins.mzn`, versioned `redefinitions*.mzn`, global
  decompositions and `stdlib/stdlib_ann.mzn`.
  The [2.10.1 interface specification](https://docs.minizinc.org/en/2.10.1/fzn-spec.html)
  defines text/JSON forms and solver-library conventions.
- Gecode **latest main checked on 2026-10-26**, commit
  `186551447db0f74639d65cb8de5af190f15ac2c2`:
  [MiniZinc library](https://github.com/Gecode/gecode/tree/186551447db0f74639d65cb8de5af190f15ac2c2/gecode/flatzinc/mznlib),
  including `gecode.mzn`, `gecode_fzn.mzn`, overrides and versioned redefinitions;
  [FlatZinc registry](https://github.com/Gecode/gecode/blob/186551447db0f74639d65cb8de5af190f15ac2c2/gecode/flatzinc/registry.cpp)
  supplies registration aliases, relation codes and posting semantics. This
  revision includes explicit-end scheduling, optional rectangles, arithmetic,
  set and float extensions absent from the installed release. Do not substitute
  that release or silently advance the source revision during implementation.

The coverage table records exact predicate names/signatures, aliases, owning
task families, checking semantics and supported deterministic outputs. Include
standard builtins added through versioned redefinitions, deprecated spellings,
and Gecode registrations behind set,
float or MPFR build flags. Distinguish standard-library predicates that flatten
to these operations from external predicates that need a direct evaluator.
Check include/decomposition closure for both actual library profiles; a count
of registered names alone does not prove library support. Do not build a general
MiniZinc interpreter or copy Gecode's solver implementation into Zincite.

Each predicate-family task delivers preparation, type/shape checks, direct
evaluation, useful deterministic completion, typed reporting, validated loading,
and native/Wasm agreement together. Complete relation checking is mandatory;
solving a relation backwards or choosing among multiple outputs is not.
Preserve `defines_var` semantics and distinguish full reification from implication.
Recognize declarations by signature and argument roles rather than formal
parameter spelling. Cover malformed input at both source and serialized entry
points. Keep integers exact and checked at the existing i64 boundary; respect
explicit Gecode integer/set-universe constants where those are part of an
extension's semantics rather than replacing them with i64 limits.

Legacy four-argument `cumulatives` checks concrete candidate values. Its result
must not depend on declared start ranges or Gecode's posting and propagation
effects. Document its concrete scheduling relation and the difference from
upstream's domain-dependent behavior in the checker guide. This explicit
choice also applies to its registered aliases; it does not add propagation or
search to the checker.

Explicit-end scheduling checks supplied starts and ends independently of
Gecode's posting-time rewrites. Unary checks endpoint separation for present
tasks. Cumulative checks half-open interval loads and each present task's own
usage plus tasks strictly spanning its start; coincident zero-length tasks are
checked individually. Multi-machine checks signed half-open interval loads
only during active time. Preserve unary/cumulative sign requirements, including
duration requirements for absent tasks. Do not add end equalities or
multi-machine duration/machine guards: separate model constraints check those
caller obligations. Document this concrete convention on inconsistent tuples
and its difference from native propagation behavior. Scheduling itself selects
no output; separately marked end equations can provide completion.

Floats are in scope. Use finite binary64 values with configurable absolute and
relative comparison tolerances, including a zero-tolerance strict setting.
Document the comparison formula, defaults, strict inequalities, domain checks,
rounding conversions, nonfinite/undefined operations and transcendental behavior
before exposing them. The same explicit settings must govern native, CLI and
Wasm checking; never round integer payloads through floats. Tolerance must not
invent values for unresolved variables or conceal unsupported operations.

For float-core, default absolute and relative tolerances are both `1e-9`.
Both settings must be finite and nonnegative; zero/zero gives exact checking.
Equality is `abs(a-b) <= max(absolute, relative * max(abs(a),abs(b)))`.
Evaluate it with exact equality first, then a finite absolute-distance check,
then a scaled relative-distance check. Use `abs(a-b)/m` for finite distance,
where `m=max(abs(a),abs(b))`; if subtraction overflows, use
`abs(a*0.5-b*0.5)/(m*0.5)`. Skip the relative check when its setting is zero.
Boundary results follow these binary64 operations; never compare an infinite
distance with an overflowing threshold. Disequality is the complement.
Strict less means raw `a<b` and unequal; non-strict less means raw `a<b` or
equal. Mirror these rules for greater comparisons and use them for checked
float outputs, domains, reification and implication.

The prepared model owns its validated settings and exports both explicitly.
Old version-1 artifacts without settings use the documented defaults. Rust,
CLI and Wasm configuration changes the same model policy; a CLI option changes
only its named component. Float values use finite JSON numbers with correctly
rounded binary64 parsing. Normalize signed zero to positive zero. Alias,
singleton and default identity stays exact; tolerance supplies no missing value.
Arithmetic checks each required intermediate for finiteness, permits underflow,
and reports zero division and nonfinite results as located execution errors.
Integer payloads keep their decimal strings. Only explicit `int2float` converts
them, with nearest-even binary64 rounding. Ceil/floor/round produce exact integer
outputs; round ties go away from zero. Check the converted result against the
inclusive `-2^63` and exclusive `2^63` limits before casting.

Float interval emptiness uses exact bounds before tolerant membership checks.
Reversed declaration intervals are empty and supply no default. `float_dom`
uses complete consecutive endpoint pairs and ignores an odd trailing endpoint;
empty data denotes an empty union. Fixed float-set transport requires canonical
ordered, nonoverlapping ranges and rejects reversed entries. Keep search
precision metadata separate from checker tolerance. The checker guide documents
the public settings APIs, supported spellings and entry-point examples.

Both library profiles need reproducible offline compilation commands. The
standard profile uses MiniZinc's own decompositions plus a small Zincite library
declaring supported primitives. The Gecode profile uses the pinned upstream
library and all its mathematical extensions, not a checker library that happens
to override just one Gecode predicate. Preserve annotations through each path
where the compiler allows it and document domain tightening/elimination limits.
Keep package configuration usable with `--compile`; the checker is not a solver
and must not advertise a search/optimization implementation.

Search, propagation and output-selection annotations are accepted as metadata
when they do not change the checked relation. Unknown semantic annotations are
diagnosed. Experimental blackbox executable/DLL calls and restart-history hooks
depend on external execution or search state and conflict with the existing
pure native/Wasm checking contract. Inventory these explicitly and give located
diagnostics; do not silently treat them as satisfied constraints or claim support
for their execution. They are separate from full mathematical constraint
coverage. Output rendering, solver protocols and optimality proofs are also
outside candidate checking.

### Expansion validation

Keep tests focused on public behavior, with compact table-driven cases where
they test distinct predicate semantics. Exercise actual predicate names and
signatures, true/false and reified/implication distinctions, empty inputs,
offset/tie/optional semantics and relevant arithmetic boundaries. Combine these
with changed/removed assignments, supplied-output conflicts, incomplete reports
and multiple findings in a few small models. Do not duplicate the full native
suite in JavaScript or introduce a test/registration framework just for coverage.

Every family updates the coverage table and current guide. Use a few native
report round trips and native/Wasm comparisons for new operations and values,
plus the existing Rust checks. Compiler-profile tasks run compile-only checks
against both fixed sources and inspect emitted operations and metadata directly.
Solver comparisons are optional evidence for a concrete semantic uncertainty;
no broad solver matrix is required. The final acceptance task checks every
coverage row and the two include/decomposition closures, exercises both CLI
forms and JSON input, and repeats the ordinary browser workflow/performance
check after the wider runtime lands. No full-support claim is valid with
unassigned or unsupported mathematical predicates in either targeted profile.

The final audit admits 128 direct standard signatures, 316 direct Gecode
signatures and 954 registered mathematical name/signature pairs across 918 names.
All 959 admitted call names reach mathematical evaluation in the family tests;
the authored coverage notes map those loops, overload roles and fixture routes.
Both include profiles have zero missing edges. The full native/Wasm smoke passes
184 comparisons across 21 artifacts. Source, prepared and official compiler JSON
reports agree through the native APIs and both CLIs, including exact integers,
float settings, partial input and supplied-output conflicts.

Compiler limits supply no runtime credit: the standard selected tree has 32
unconditional abort bodies across 27 names; Gecode's selected fallbacks have 31
across 26. Standard integer-table reification compiles only at widths 1..5.
Fresh width-2..5 controls also emit duplicate `defines_var` hints for one
auxiliary, contrary to the pinned specification's single-definition rule;
preparation diagnoses that malformed metadata. Width 1 exports and width 6
fails the compiler assertion. Gecode native table reifiers export. Seven
malformed restart prototypes lack semicolons. Direct mathematical coverage is
established separately from these compiler failures.

## Deferred research and decisions

The full sequential check is the reference behavior. If measurements justify
incremental checks, build variable-to-dependent-operation edges, invalidate
transitive derived values after edits, and compare results with a fresh check,
including removed assignments. No persistent cache, generic graph framework,
parallel executor or solver is needed for the initial checker.

The proposal's minimal sufficient input sets, multiple channelled viewpoints,
dynamic evaluation ordering, change-based explanations and violation measures
are later research topics. Structured violations with application metadata
provide the initial user feedback; completeness information also supports
partially specified states and integration work. Do not promise minimal
explanations or a minimal set of violated source constraints. Global inverse
consistency is a literature lead mentioned
in the email, not a selected algorithm or a requirement to test arbitrary
partial configurations.

The first dialect and compiler recipe are established by the completed tasks.
A general mapping of enum/record/optional MiniZinc values
to a high-level editing API is a product choice for later scope. Initially
callers use documented FlatZinc names/array positions for one prepared model.
The original proposal's academic timetable, thesis report and publication
arrangements are historical context, not project deliverables.

## Testing

Focused public-behavior coverage. Reuse the workspace's integration-test style;
no new tests for planning, metadata or scaffolding alone. Add a few small models
that jointly exercise complete success, all violations in an infeasible
candidate, completion from primary decisions, insufficient-input reports,
supplied/derived conflicts and explicit unsupported input. Include a
candidate with multiple violations where a derived value is outside its domain;
verify that downstream checks still run and that application identifiers and
typed payloads survive preparation, reporting and text rendering.
Include one reified-versus-half-reified distinction and one checked arithmetic
boundary where those operations enter the supported dialect.

For the compiler recipe, inspect the actual generated FlatZinc and labels, then
check known assignments against the intended source constraints directly.
MiniZinc acceptance alone does not prove preservation of labels, editable
variables or model meaning. A tiny native solver comparison may supplement
the recipe experiment when needed; solving is never a runtime dependency or a
broad solver-matrix gate. Use synthetic redistributable examples.

Serialization tests compare observable reports before and after loading, plus
one invalid artifact. Wasm acceptance checks the same named cases as native
and includes an integer beyond JavaScript's exact-number range. Demonstrate
application feedback from structured fields rather than parsed messages, and
verify completeness information with and without known violations. Do not
duplicate the entire native test suite in JavaScript. Benchmark measurements stay outside
timing assertions in `cargo test`; use a small existing-style driver.

## Validation

After record changes, run `zdev check checker --format json` and the
repository-required `zdev check base --format json`.

For implementation, run `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. Add a parser-free Wasm build and the focused browser
acceptance when that increment exists; native workspace checks alone do not
establish Wasm support. Use the recorded MiniZinc compilation recipe when
changing the checker library or supported predicate set. Reuse existing syntax
and save-performance checks only if shared parsing/layout code changes.
