# Checker compilation contract

MiniZinc 2.10.1, build 33348285743, compiles the two-job example into a
bounded Boolean/integer FlatZinc model. Both editable starts survive, arithmetic
definitions have explicit forward directions, and each constraint carries an
application identifier and integer payload. Job 1 cuts panels in three hours;
job 2 assembles them in two. The application supplies their start times. The
native checker prepares this artifact and checks complete and incomplete
candidates. The packaged Rust example translates its typed findings into job
feedback without parsing messages.

## Reproduce the specimen

From the repository root, with the named MiniZinc version on `PATH`:

```sh
minizinc --version
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --globals-dir crates/zincite-check/minizinc --no-output-ozn -D mzn_ignore_symmetry_breaking_constraints=true --output-paths-to-file /tmp/zincite-checker-compilation.paths --output-fzn-to-file examples/checker-compilation/model.fzn examples/checker-compilation/model.mzn
```

The driver requires an explicit solver selection for `--compile` in this
installation. That flag compiles without starting Gecode. `--globals-dir`
selects `crates/zincite-check/minizinc/`, packaged with the checker crate. The annotation file declares the two application annotations. The library also
contains the strict disjunctive override described below; the original two-job
example needs no builtin override or solver configuration file. No candidate file is passed to the compiler: compile once, then
inspect candidates against that same artifact. The `.paths` sidecar goes to
`/tmp`; the inspected contract uses the annotations already in `model.fzn`.

The documented command can put both output files under `/tmp` when the caller
does not need to refresh the archived specimen. In another checkout, the
library comment and absolute `mzn_path` prefixes also change. Generated names and locations belong to one compilation.

## Values, addressing and domains

The source indexes both arrays by `Job = 1..2`. FlatZinc uses positions 1 and 2,
and `output_array([1..2])` records those source indexes. The array declarations
are aliases to these scalars:

| Source/FlatZinc array entry | FlatZinc scalar | Emitted domain |
| --- | --- | --- |
| `start[1]` | `X_INTRODUCED_17_` | `0..8` |
| `start[2]` | `X_INTRODUCED_18_` | `0..8` |
| `finish[1]` | `X_INTRODUCED_19_` | `3..9` |
| `finish[2]` | `X_INTRODUCED_20_` | `2..9` |
| `starts_early` | `starts_early` | Boolean |

A full primary assignment can name `start = [1, 4]`. A partial one names the
available scalar, such as `X_INTRODUCED_17_ = 1`; it does not insert a placeholder
for the other array entry. Those generated names are valid only for this
prepared model. Preparation resolves array/scalar aliases before accepting
assignments and detects contradictory values supplied through either name.

The compiler folds durations `[3,2]` into constants and shares coefficient
arrays `[1,-1]` and `[1,1]`. There is no element-access predicate. The original
finish domain is `0..9`; flattening infers lower bounds 3 and 2 from the starts
and durations. Both upper bounds stay 9. Neither `computed_domain` nor
`domain_change_constraint` occurs. An inspected `-g` variant produced the same
domains and constraints as `-O0`, so `-g` is not a domain-preservation guarantee.
Check the actual emitted domains; do not discard unmarked bounds. The inferred
lower bounds follow from the source equations when the starts are in domain.
A supplied inconsistent auxiliary may fail an inferred bound as well as its
definition; reports need not be a minimal set of source failures.

Explicit finish equations and `defines_var(finish[job])` avoid a demonstrated
initializer rewrite that substituted starts into precedence and dropped the
named finish array. A Boolean initializer with no other use disappeared in a
control compilation. The explicit labelled equivalence in this model produces
an observable full reified definition. `-O0` alone prevents neither rewrite.

## Direct checks and completion

These are all emitted predicate names. Scalar arguments accept integer or
Boolean literals and resolved variable references; coefficient arrays are
integer constants. Arrays have matching lengths.

| Direct check | Signature and meaning |
| --- | --- |
| `int_lin_eq` | `(array[int] of int: a, array[int] of var int: x, int: c)`; `sum(a[i]*x[i]) = c` |
| `int_lin_le` | Same argument types; `sum(a[i]*x[i]) <= c` |
| `int_le_reif` | `(var int: x, var int: y, var bool: b)`; `b <-> (x <= y)` |

Completion has only these directions in the specimen:

- `int_lin_eq([1,-1], [finish[1],start[1]], 3)` defines `finish[1]`;
  the second equality uses the second entries and constant 2. Given the start,
  calculate the finish. In general, a supported unique linear output is
  `(c - sum(other terms)) / output_coefficient`, with a nonzero coefficient
  and exact divisibility. Both actual output coefficients here are 1.
- `int_le_reif(start[1], 2, starts_early)` defines its Boolean result from the
  two integer arguments. It is equivalence, not half reification. A false
  result is valid when `start[1] > 2`.

The output declarations have `is_defined_var`, and each defining relation has
the matching `defines_var`. Every definition is also checked as a relation,
including when a caller supplies its result. Do not invert inequalities or
choose missing starts from finish bounds. A derived finish of 11 is available
to precedence even though it fails its domain. The runtime uses checked `i64`
products, sums, subtraction and exact division for these definitions. Overflow is a located execution error; nonexact division
leaves an output unresolved. Half reification, division/remainder predicates and partial expressions are
outside this dialect. The shared-bench extension below admits one global.
This original specimen emits exactly the three predicates above; `int_lin_le` has no
completion direction. The crate README documents declaration, annotation,
assignment and arithmetic boundaries.

## Application metadata and locations

`checker_constraint(string: id, array[int] of int: entities)` survives on all
five flattened constraints. `checker_value` uses the same argument types on
arrays and scalars. Array value annotations are copied onto their elements;
the array alias position identifies the affected job. String labels become
`mzn_constraint_name`, and `--keep-paths` supplies source ranges and generator
contexts in `mzn_path`.

| FlatZinc ordinal | Application identifier/payload | Label | Relation |
| --- | --- | --- | --- |
| 1 | `finish`, `[1]` | Finish follows duration | `finish[1] - start[1] = 3` |
| 2 | `finish`, `[2]` | Finish follows duration | `finish[2] - start[2] = 2` |
| 3 | `precedence`, `[1,2]` | Job 1 precedes job 2 | `finish[1] - start[2] <= 0` |
| 4 | `start_budget`, `[1,2]` | Start budget | `start[1] + start[2] <= 10` |
| 5 | `early_start`, `[1]` | Early start indicator | `starts_early <-> start[1] <= 2` |

The `forall` splits into ordinals 1 and 2: the label is shared while the typed
payload distinguishes jobs. The compiler emits reification last despite its
earlier source position. Use FlatZinc locations/ordinals for the compiled
constraint order, with labels and source paths as context. A finish-domain
violation has the variable's `checker_value("finish",[1,2])`, its array
position, and its FlatZinc declaration location; its unique definition also
has the job-specific `finish` payload. Applications can identify the job
without parsing a label. Metadata on this bounded example is accounted for;
arbitrary custom annotations and other decompositions need separate evidence.

## Candidate walkthroughs

The files in `examples/checker-compilation/candidates/` contain only literal
assignments for checker input. The calculations below inspect the source
relations and the actual emitted coefficient vectors directly, without solving.

| Candidate | Supplied starts | Derived finishes | `starts_early` | Outcome |
| --- | --- | --- | --- | --- |
| `feasible.dzn` | `[1,4]` | `[4,6]` | `true` | Valid, complete |
| `infeasible.dzn` | `[8,5]` | `[11,7]` | `false` | Invalid, complete |
| `incomplete.dzn` | first scalar `1` | first finish `4` | `true` | Incomplete, no known violation |
| `incomplete-invalid.dzn` | first scalar `8` | first finish `11` | `false` | Invalid, incomplete |

For the complete candidates, all start domains pass. The two definition checks
give `4-1=3`, `6-4=2`, or `11-8=3`, `7-5=2`. Full reification passes in each
case. The other checks are:

| Check | Feasible | Infeasible |
| --- | --- | --- |
| Finish domains | `4 in 3..9`, `6 in 2..9`: pass | `11 not in 3..9`: fail; `7 in 2..9`: pass |
| Precedence, ordinal 3 | `4-4=0 <= 0`: pass | `11-5=6 <= 0`: fail |
| Start budget, ordinal 4 | `1+4=5 <= 10`: pass | `8+5=13 <= 10`: fail |

Thus the infeasible candidate exposes three violations, including precedence
after the finish-domain failure. Its domain finding identifies `finish[1]`
and job 1; the other two findings carry `precedence/[1,2]` and
`start_budget/[1,2]`. No failed check suppresses another check.

In both partial candidates, the second start and finish are unresolved. Their
domain checks and ordinals 2, 3 and 4 are blocked; ordinal 1 and reification
pass. The first start's domain passes. In `incomplete.dzn` the first finish
domain also passes. In `incomplete-invalid.dzn` it fails at 11, even while
precedence and budget are blocked. Insufficient input is a normal report with
unresolved values and blocked checks, plus any established violations. It is
neither an input error nor evidence that a satisfying completion exists.

## Application feedback example

Run `cargo run -p zincite-check --example schedule-feedback -- MODEL.fzn` with
this compiled artifact. The example prepares once and sends a complete
`AssignmentValue::Array` named `start` for each edit. App job IDs 1 and 2 match
array positions 1 and 2. The example uses `Assignment::default()` when the whole
start array is omitted.

The example checks `[1,4]`, `[8,8]` and omitted input. `[8,8]` derives finishes
`[11,10]` and a false early-start flag. Both finish domains fail, precedence
fails because `11-8=3 > 0`, and the budget fails because `8+8=16 > 10`. Its four
findings form a complete invalid report. The two finish findings share
`checker_value("finish",[1,2])`; their distinct array alias positions select
job IDs 1 and 2. The example uses constraint entity payloads directly for the
other two findings. It reads `CheckRef`, variable aliases/positions and
`ViolationKind` to choose application wording. It does not parse labels,
paths or checker-written messages, group by application ID, or check a second
copy of the scheduling rules.

Omitting all starts yields an incomplete report with no established violation,
nine unresolved variable references and fourteen blocked checks. These counts
include the runtime's scalar and array declaration references; they are not
counts of distinct application jobs. The example displays completeness beside
findings, independently of their presence.

## Compiler controls and editing limits

These compile-only controls used MiniZinc 2.10.1 build 33348285743 and the same
`-O0 --keep-paths --globals-dir crates/zincite-check/minizinc --no-output-ozn`
flags, with all model, FlatZinc and paths files under `/tmp`. No solver ran.
The observations apply to these sources; they do not establish equivalence for
arbitrary MiniZinc input.

### Initializer elimination

In a copy of the two-job model, replace the finish declaration and explicit
finish equations with this initializer, and replace the Boolean declaration
and explicit equivalence with the second initializer:

```minizinc
array[Job] of var 0 .. 9: finish :: checker_value("finish", [1, 2]) = [start[job] + duration[job] | job in Job];
var bool: starts_early :: checker_value("starts_early", [1]) = start[1] <= 2;
```

Leave starts, precedence, budget and `solve satisfy` as in the specimen.
The compiler drops the named `finish` array and unused `starts_early` scalar.
It substitutes starts into precedence, producing
`int_lin_le([1,-1], [start[1],start[2]], -3)` in scalar form. Two unnamed finish
variables with domains `3..9` and `2..9` still have arithmetic definitions.
They carry `var_is_introduced`; their defining constraints carry
`checker_value("finish",[1,2])` instead of the explicit equations' job-specific
`checker_constraint` annotations. Preparation rejects the first
`var_is_introduced` annotation, with exit status 2 and no candidate report.
`checker_value` on a constraint is also outside the documented contexts. Do not
remove these annotations or reconstruct missing application values to admit
this output. The explicit-equation specimen supplies the supported editing
contract.

### Domain restriction and lost constraint feedback

Compile this complete control with the packaged include:

```minizinc
include "globals.mzn";
include "checker_annotations.mzn";
var 0 .. 8: start :: checker_value("start", [1]);
% This source restriction can become a narrower declaration domain.
constraint :: "Latest start"
(start <= 2) :: checker_constraint("latest_start", [1]);
solve satisfy;
```

The output contains `var 0..2: start` with its `checker_value("start",[1])`,
`mzn_path` and `output_var` annotations, and no constraint. Neither the
`latest_start` identifier nor its label survives. The runtime accepts the
output; `start=8;` is invalid and complete with one domain finding, value 8 and
domain `0..2`. This agrees with the source restriction, but feedback can name
only the variable/application entity and its emitted domain. The initial
`0..8` domain is not a promise that every edit inside it passes. The two-job
specimen also shows inferred finish lower bounds without a domain marker.
Applications must inspect and check the actual compiled domains.

### `forall` decomposition

The specimen is the decomposition control: its one labelled `forall` emits two
`int_lin_eq` relations. The label is shared; `checker_constraint("finish",[1])`
and `checker_constraint("finish",[2])` identify different jobs. The metadata
and functional directions survive preparation. Supplied wrong finishes are
checked by those relations, and `[8,8]` demonstrates two distinct domain
findings sharing value metadata. No finding is dropped because another one
uses the same application ID.

### Optional symmetry breaking

Compile this complete control twice, once with
`-D mzn_ignore_symmetry_breaking_constraints=true` and once with the value
`false`:

```minizinc
include "globals.mzn";
include "checker_annotations.mzn";
array[1 .. 2] of var 0 .. 8: start :: checker_value("start", [1, 2]);
% The two otherwise interchangeable jobs can be ordered for solving.
constraint :: "Order interchangeable jobs"
symmetry_breaking_constraint(start[1] <= start[2]) :: checker_constraint("job_order", [1, 2]);
solve satisfy;
```

With `true`, both start scalars and their array alias survive; no constraint
is emitted. `start=[8,1];` is valid and complete. With `false`, the same
declarations accompany one `int_lin_le([1,-1], [start[1],start[2]], 0)` in scalar
form, labelled `Order interchangeable jobs` with `job_order/[1,2]` and a source
path. The runtime accepts it and reports one complete invalid finding for
`[8,1]`, with sum 7 and bound 0. Direct evaluation agrees with `8 <= 1` being
false. Use `true` when the application's edits allow either representative.
The main scheduling model contains no symmetry breaker; its documented command
sets this choice explicitly. This does not test other symmetry formulations.

## Finite sets and the shared bench

The separate `shared-bench.mzn` model describes two independent workshop jobs
that need exclusive access to one bench. Their durations are `[3,2]`; the
application supplies starts, tool selections and a lead tool. The original
scheduling/browser model and its candidates are unchanged.

The source calls standard `disjunctive_strict`. The packaged override keeps its
one resource relation intact so an application receives typed job arguments and
all conflicting position pairs. This is a concrete feedback benefit; no
performance improvement was measured or claimed. The override is exactly:

```minizinc
predicate fzn_disjunctive_strict(array[int] of var int: start, array[int] of var int: duration);
```

The checker validates this known bodyless declaration, including parameter
names, before accepting its calls. Predicate bodies, duplicate declarations,
mismatched signatures and other user declarations fail preparation. This is
one implemented global, without a generic predicate interpreter.

### Repeat the compile-only check

With MiniZinc 2.10.1 build 33348285743:

```sh
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --globals-dir crates/zincite-check/minizinc --no-output-ozn -D mzn_ignore_symmetry_breaking_constraints=true --output-paths-to-file /tmp/zincite-checker008-shared-bench.paths --output-fzn-to-file examples/checker-compilation/shared-bench.fzn examples/checker-compilation/shared-bench.mzn
cargo run -p zincite-check -- examples/checker-compilation/shared-bench.fzn examples/checker-compilation/candidates/shared-bench-feasible.dzn
cargo run -p zincite-check -- --format json examples/checker-compilation/shared-bench.fzn examples/checker-compilation/candidates/shared-bench-infeasible.dzn
```

The compilation succeeded without warnings and started no solver. Source,
packaged override and literal candidates pass Zincite's formatter check and
default linting with the installed standard library and packaged include path.
The archived FlatZinc is the exact compiler output, including its command-header
whitespace. The paths sidecar is temporary.

### Actual declarations and metadata

The compiled model declares starts `X_INTRODUCED_17_` and `X_INTRODUCED_18_`
with domains `0..8`. The `start` array aliases them in that order. Set scalars
`X_INTRODUCED_19_` and `X_INTRODUCED_20_` have upper domains `1..3` and `1..4`;
the `tools` array aliases them as `array[1..2] of var set of int`. Both arrays
carry output and `checker_value` annotations. Array positions and scalar aliases
let callers name supplied values and associate domain findings with jobs.

The source declares both tool upper bounds as `1..4`. Its labelled
`tools[1] subset {1,2,3}` restriction becomes the narrower first upper domain.
The `permitted_tools` constraint ID and label disappear. That failure is
reported through the `tools/[1,2]` variable metadata and alias position 1. The
checker checks the emitted domain and does not reconstruct the lost label.

`all_tools` and `shared_tools` have upper domain `1..4` and their matching
`is_defined_var`/`defines_var` annotations. `n_tools` has domain `0..4` and its
matching cardinality definition. The source uses explicit `set_union`,
`set_intersect` and `set_card` calls. High-level set equations can introduce
auxiliary sets without supported output annotations; the recipe depends on the
inspected explicit primitive form. The lead tool domain is `1..4`.

These are all five emitted constraints, in actual FlatZinc order. Each carries
its source label and `mzn_path`; entity payloads use exact integer strings when
exported.

| Ordinal | Predicate and direction | Application ID/entities | Label |
| --- | --- | --- | --- |
| 1 | `fzn_disjunctive_strict(start,[3,2])`; no completion | `shared_bench/[1,2]` | One job at a time at the shared bench |
| 2 | `set_union(tools[1],tools[2],all_tools)` defines `all_tools` | `all_tools/[1,2]` | Total tools |
| 3 | `set_intersect(tools[1],tools[2],shared_tools)` defines `shared_tools` | `shared_tools/[1,2]` | Shared tools |
| 4 | `set_card(all_tools,n_tools)` defines `n_tools` | `n_tools/[1,2]` | Tool count |
| 5 | `set_in(lead_tool,tools[1])`; no completion | `lead_tool/[1]` | Lead tool is available |

### Admitted set and global operations

The Boolean/integer table above still applies. These are the complete additions:

| Predicate | Direct relation | Deterministic `defines_var` output |
| --- | --- | --- |
| `set_in(x,s)` | `x` belongs to `s` | None |
| `set_eq(s,t)` | Equal sets | Either set argument if it occurs once |
| `set_subset(s,t)` | Every member of `s` belongs to `t` | None |
| `set_card(s,n)` | `card(s) = n` | Integer second argument |
| `set_union(s,t,r)` | `r = s union t` | Third argument if absent from both inputs |
| `set_intersect(s,t,r)` | `r = s intersect t` | Third argument if absent from both inputs |
| `fzn_disjunctive_strict(start,duration)` | Every pair satisfies `start[i]+duration[i] <= start[j]` or the reverse | None |

Integer sets use sorted, disjoint, nonadjacent inclusive i64 ranges, without
expanding interval members. Reversed literal intervals denote the empty set.
A set upper domain restricts allowed members; it requires none of them. Only
an empty upper bound supplies a unique default. Fixed set initializers supply
their exact set, and definitions withhold defaults as for integer/Boolean
outputs. Set arithmetic and comparisons borrow values; assignments, computed
outputs and findings own their values. Unsupported set operations fail
preparation explicitly. Cardinality above i64::MAX is a located execution
error, including while checking a supplied cardinality result.

The global requires equally long arrays and fixed nonnegative durations. Empty
arrays pass. Every start-plus-duration endpoint uses checked i64 addition, even
for a one-task array. A zero-duration task inside another task's occupied
interval conflicts; placement at either endpoint passes. Two zero-duration
tasks at the same start pass. A single finding carries typed `tasks` with
one-based `position`, `start` and `duration`, and all conflicting pairs as
`{first, second}` positions. The entity payload maps positions to application
jobs, independently of generated variable names.

Prepared JSON stays at format version 1 with additive `integer_set` values,
`integer_set` upper domains, six executable set operations and the strict global
operation. Existing Boolean/integer shapes are unchanged. A set value contains
`{"ranges":[{"lower":"1","upper":"3"}]}`; the empty set has an empty list.
All endpoints, cardinalities, start/duration values and application entity
integers use exact decimal strings. Loading rejects noncanonical ranges, wrong
operand types, invalid defaults, invalid duration parameters, unequal global
array lengths and mismatched report predicate names before rebuilding aliases
and definitions. Assignment JSON validates the same private set storage.

### Direct candidate calculations

For `shared-bench-feasible.dzn`, starts `[1,4]` give endpoints `[4,6]`. The
first job ends at the second job's start, so the strict pair passes. Tools
`[{1,2},{2,3}]` give union `{1,2,3}`, intersection `{2}` and count 3. Both
selected sets and derived sets fit their upper domains; lead tool 1 belongs to
the first selection. All five relations pass and every value is resolved.

For `shared-bench-infeasible.dzn`, starts `[1,2]` give endpoints `[4,4]`.
Neither `4 <= 2` nor `4 <= 1` holds, so positions 1 and 2 conflict. Tools
`[{1,5},{2,3,4}]` give union `{1,2,3,4,5}`, empty intersection and count 5.
The first selection exceeds upper domain `1..3`, the union exceeds `1..4`,
and count 5 exceeds `0..4`. Lead tool 4 is in its own integer domain but does
not belong to the first selection. These are five complete findings: three
domains, one resource conflict and one membership failure. All three definition
relations pass, so the domain failures do not block cardinality or membership.

Supplying only `start=[1,2];lead_tool=1;` establishes the same resource conflict
while both selections and all three derived values are unresolved. The report
is invalid and incomplete, with blocked set/domain checks. Omitting every input
produces an incomplete report with no established violation. Rechecking the
feasible assignment on the same prepared model passes; no prior edit supplies
values to a later check.

Native source and loaded checks agree on these candidates. The established
Node smoke loads this same prepared workshop artifact and compares its complete
and incomplete reports with native checks twice. Its separate exact-value
artifact compares integer and set endpoint 9007199254740993 against
9007199254740992, without JavaScript Number conversion. The parser-free Wasm
build excludes `zincite-syntax`. The native suite covers the wider set/global
evaluator cases, including strict zero-duration boundaries and checked
cardinality overflow. No solver ran and no performance benchmark was part of
this extension.

## Validation and sources

The original packaged specimen succeeded and emitted only its three predicates,
with five constraint IDs, labels, paths and the documented definition directions.
Native CLI runs of all four literal fixtures agreed with their source equations
and all five FlatZinc constraints. The application example reported the expected
feasible, four-finding infeasible and omitted-input results. Focused integration
coverage checks that both finish-domain findings map to distinct positions and
job IDs despite shared value metadata.

The initializer, domain, decomposition and symmetry controls above were inspected
directly and checked as described. The repository formatter's `--check` passed
for the model, packaged library and candidates. Default linting with the installed
standard library and `-I crates/zincite-check/minizinc` exited 0; it reported one
`index-set-mismatch` analysis limitation for the array access in `defines_var`.
This is not full lint analysis. Cargo formatting, Clippy and workspace tests
passed; Cargo's package file list includes the MiniZinc library and guarded Rust
example. No compiler or solver runs during checking, and no solver ran during
this validation.

Primary references are the
[MiniZinc 2.10.1 FlatZinc specification](https://docs.minizinc.org/en/2.10.1/fzn-spec.html),
the installed `std/flatzinc_builtins.mzn` declarations for the three predicates,
and `std/stdlib/stdlib_ann.mzn` for definition, domain, label and path annotations.
Both library files are under
`/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc/` in the inspected
installation. `minizinc --help` documents the compile-only flags. The saved
source, packaged annotation library and unedited emitted `model.fzn` are the
evidence for this recipe; compiler acceptance alone does not establish a
general checker dialect.
