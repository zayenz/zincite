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
selects `crates/zincite-check/minizinc/`, packaged with the checker crate. Its
only file declares the two application annotations. No builtin override or
solver configuration file is needed for this example. No candidate file is passed to the compiler: compile once, then
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
leaves an output unresolved. Half reification, division/remainder predicates,
globals and partial expressions are outside this dialect.
The runtime admits exactly the three predicates above; `int_lin_le` has no
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

## Validation and sources

The packaged recipe succeeded and emitted only the three admitted predicates,
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
