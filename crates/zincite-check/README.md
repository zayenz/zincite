# zincite-check

A Rust and WebAssembly checker for a small FlatZinc dialect targeting MiniZinc 2.10.1.
Prepare a model once, then check candidate assignments repeatedly. Checking
performs no I/O and starts no compiler or solver.

The [full-library coverage inventory](../../docs/checker-coverage.md) records the
pinned standard/Gecode signatures, aliases, task owners and include paths for the
planned expansion. Working support is exactly the subset documented below.

This checkout introduces the checker. Install it locally with
`cargo install --locked --path crates/zincite-check`, or install the root command
with `cargo install --locked --path .`. It is not part of the published 0.3.0
packages.

## Try the compiled example

From the repository root:

```sh
cargo run -p zincite-check -- examples/checker-compilation/model.fzn examples/checker-compilation/candidates/feasible.dzn
cargo run -p zincite -- check --format json examples/checker-compilation/model.fzn examples/checker-compilation/candidates/feasible.dzn
```

Both commands derive `finish = [4,6]` and `starts_early = true` from
`start = [1,4]`, then report a valid, complete candidate. Changing the assignment
to `start = [8,5];` reports three violations:
the first finish domain, precedence and the start budget. Each finding includes
its location and application metadata. Missing values produce an incomplete
report, alongside any known violations.

The four saved candidates in `examples/checker-compilation/candidates/` supply
only primary decisions and cover feasible, infeasible and incomplete reports.

## Export once and load for repeated checks

```sh
cargo run -p zincite-check -- --export /tmp/zincite-prepared.json examples/checker-compilation/model.fzn
cargo run -p zincite-check -- --load --format json /tmp/zincite-prepared.json examples/checker-compilation/candidates/infeasible.dzn
cargo run -p zincite -- check --load /tmp/zincite-prepared.json examples/checker-compilation/candidates/incomplete-invalid.dzn
```

`--export OUTPUT.json MODEL.fzn` prepares the source and creates a new file.
It refuses an existing output path, including the source path, and writes
nothing to stdout. Choose a fresh path for another export. `--load` validates
the prepared JSON and checks assignments without reading or parsing the
FlatZinc again. Both entry points support these options; the source-checking
invocation above also works.

The artifact has `format_version: 1` and an executable `model`: typed slots
and seed defaults, name/array bindings, declaration domains, the supported
operations, an optional objective, and report references with application
metadata. It carries no source text or CST. All mathematical integers are
decimal strings, including domain bounds, coefficients, defaults and entity
payloads. Integer sets use `{ "kind": "integer_set", "value": { "ranges":
[{ "lower": "1", "upper": "3" }] } }`; an empty set has `ranges: []`. Loading
rejects reversed, unordered, overlapping or adjacent ranges. The format stays
at version 1 with additive float values/settings/operations, set value/domain, set operation and strict global
records and integer relation variants. The original `Linear` executable with its
`equality` Boolean and `ReifiedLessEqual` executable keep their existing shapes. Loading rejects
incompatible versions, bad references, wrong types and invalid definition
annotations before exposing a checkable model. It rebuilds aliases, definition directions and dependencies from validated
operations rather than accepting serialized execution caches. A slot's seed
must come from a fixed or singleton declaration; other alias domains can still
report violations against that value.

## Compile the application model offline

The package includes `minizinc/checker_annotations.mzn`. It declares
`checker_constraint(string: id, array[int] of int: entities)` for constraints and
`checker_value` with the same arguments for variables. Application developers
choose these IDs and integer entity payloads. The original scheduling example
needs no builtin override or separate solver configuration file. The shared-bench
example below uses the packaged `fzn_disjunctive_strict.mzn` override.

From the repository root, using MiniZinc 2.10.1:

```sh
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --globals-dir crates/zincite-check/minizinc --no-output-ozn -D mzn_ignore_symmetry_breaking_constraints=true --output-paths-to-file /tmp/zincite-checker-compilation.paths --output-fzn-to-file /tmp/zincite-checker-compilation.fzn examples/checker-compilation/model.mzn
cargo run -p zincite-check --example schedule-feedback -- /tmp/zincite-checker-compilation.fzn
```

`--solver` selects the installed compiler backend; `--compile` starts no solver.
For a source package outside the checkout, point `--globals-dir` at that
package's `minizinc/` directory and supply your own model path. Compilation is
an explicit offline step. Prepare its output once for repeated checks.

Job 1 cuts panels in three hours; job 2 assembles them in two. Starts are hours
from opening, between 0 and 8, and both jobs must finish by hour 9. Assembly
starts after cutting finishes. A separate budget limits the sum of start times
to 10. `starts_early` records whether cutting starts by hour 2 and imposes no
required truth value.

The [Rust example](examples/schedule-feedback.rs) sends the application's
complete start array as `AssignmentValue::Array` named `start`. Position 1 maps
to application job ID 1 and position 2 to job ID 2. The checker derives finishes
and the early-start flag from those primary decisions. Omitting the whole array
uses `Assignment::default()`; no placeholder values are needed.

| Application edit | Checker outcome | Application feedback |
| --- | --- | --- |
| `[1,4]` | Valid, complete | No findings; finishes are `[4,6]` |
| `[8,8]` | Invalid, complete | Finish 11 for job 1, finish 10 for job 2, precedence and start budget |
| Omitted `start` | Incomplete | Unresolved values and blocked checks, no established violation |

For a constraint finding, the example maps `CheckRef::Constraint` through its
`Application.entities`. For a domain finding, it finds the array position in
`VariableRef` or its aliases and selects that entry from `checker_value`'s
entity payload. Both finish findings carry `finish/[1,2]`; their positions
identify different jobs. The example iterates every violation without grouping
by ID, chooses wording from `ViolationKind` and typed values, and prints
completeness independently. It never parses labels, paths or checker-written
messages to identify jobs.

`-O0` does not preserve every editable value or source constraint. The tested
controls show these limits:

| Source form | Observed MiniZinc output and checking behavior |
| --- | --- |
| Explicit finish equations with `defines_var` | Named arrays survive; the runtime derives both finishes and checks every relation |
| Finish and unused Boolean initializers | Named finish array and Boolean disappear; precedence uses starts directly, so callers cannot edit the removed values |
| Explicit `int_pow_fixed(x,3,cube)` with a marked result | The standard decomposition can emit `defines_var(cube)` without `is_defined_var` on `cube`; preparation diagnoses the missing marker |
| Labelled `start <= 2` for a `var 0..8` scalar | Domain becomes `0..2`; the source constraint ID and label disappear; `start=8` gives a domain finding with variable metadata |
| The model's `forall` finish equations | Two `int_lin_eq` constraints share a label and carry distinct `[1]` and `[2]` entity payloads |
| Optional order symmetry breaker | With the recipe's ignore flag, `[8,1]` passes the control; with the flag false, its emitted `int_lin_le` fails with `job_order/[1,2]` |

Use the ignore flag when an application allows either representative of a
symmetry. The main model has no symmetry breaker. Inspect each compiled model's
predicates, declarations and metadata before admitting it; the controls establish
these source forms, not equivalence for arbitrary MiniZinc models. The detailed
control sources and observations are in the repository's
[compilation contract](../../.zdev/checker/background/compilation-contract.md).

Compile-only Boolean controls with this recipe preserve explicit `bool_or`,
`bool_not`, `bool_lin_eq` and `bool_le_reif` calls, their definition annotations,
application IDs and labels. An inspected high-level Boolean equivalence instead
lowers to clauses and copies variable annotations such as `checker_value` and
`output_var` onto constraints. Current preparation diagnoses those misplaced
annotations. Explicit primitive controls establish their own compilation contract;
compiler-profile work in checker-030/031 must address the broader source form.

## Shared-bench workshop example

Two independent jobs need exclusive access to one bench. The application edits
their starts and tool selections. Standard `disjunctive_strict(start, duration)`
compiles to one resource check with the packaged declaration:

```minizinc
predicate fzn_disjunctive_strict(array[int] of var int: start, array[int] of var int: duration);
```

The checker validates that declaration's name, argument types, variable roles
and array shapes before accepting calls. Formal parameter names can vary;
bodies, annotations and duplicate declarations diagnose. Both arrays must have equal lengths, including
zero. Durations are fixed nonnegative integer parameters. For every `i < j`, the
check evaluates the pairwise relation in the coverage table with checked endpoint
additions. A zero-duration job inside another job's occupied interval conflicts;
at either boundary it passes. The checker does not derive starts or durations.

```sh
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --globals-dir crates/zincite-check/minizinc --no-output-ozn -D mzn_ignore_symmetry_breaking_constraints=true --output-paths-to-file /tmp/zincite-shared-bench.paths --output-fzn-to-file /tmp/zincite-shared-bench.fzn examples/checker-compilation/shared-bench.mzn
cargo run -p zincite-check -- /tmp/zincite-shared-bench.fzn examples/checker-compilation/candidates/shared-bench-feasible.dzn
cargo run -p zincite-check -- --format json /tmp/zincite-shared-bench.fzn examples/checker-compilation/candidates/shared-bench-infeasible.dzn
```

The saved `shared-bench.fzn` is the inspected compiler output. The feasible edit
uses starts `[1,4]`, tools `[{1,2},{2,3}]` and lead tool 1. It derives union
`{1,2,3}`, intersection `{2}` and count 3. The jobs touch at hour 4 and pass.
The infeasible edit uses starts `[1,2]`, tools `[{1,5},{2,3,4}]` and lead tool 4.
It derives union `{1,2,3,4,5}`, empty intersection and count 5. It reports five
findings: the first tool domain, the union domain, the count domain, the bench
conflict and the missing lead tool. The three definitions pass despite those
domain failures. Supplying only starts `[1,2]` and lead tool 1 gives an incomplete
invalid report with the established bench conflict and blocked set checks.

`DisjunctiveStrict` findings carry typed `tasks` with one-based `position`,
`start` and `duration`, and every conflicting pair as `{first, second}` positions.
The constraint's `shared_bench/[1,2]` entity payload maps those positions to jobs.
Keeping the global intact gives the application one resource finding with
explicit conflicting jobs. No performance improvement is claimed.

This compiler output uses the global and `set_union`, `set_intersect`, `set_card`
and `set_in`. Explicit primitive calls preserve the three set definition
annotations. The source restriction `tools[1] subset {1,2,3}` narrows its emitted
upper domain to `1..3` and loses its constraint ID and label; feedback uses that
variable's `tools/[1,2]` metadata and alias position. High-level set expressions
and arbitrary compiler rewrites need their own inspected compilation contract.

## Library use

```rust
use zincite_check::{prepare, parse_assignment, PreparedModel, Status};

let model = prepare("small.fzn", "var 0..3: x; constraint int_lin_le([1],[x],2); solve satisfy;").unwrap();
let assignment = parse_assignment("candidate.dzn", "x=3;").unwrap();
let exported = model.to_json().unwrap();
let loaded = PreparedModel::from_json(&exported).unwrap();
let report = loaded.check(&assignment).unwrap();
assert_eq!(report.status, Status::Invalid);
assert!(report.completeness.complete);
assert_eq!(report.violations.len(), 1);
```

Native callers can construct `Assignment`, `AssignmentEntry`, `AssignmentValue`
and `Value` directly. Supplied assignments are immutable; each check starts
with fresh values. `prepare` and `parse_assignment` return located diagnostics.
`check` returns `CheckError` for malformed assignments or checked arithmetic
failures. Insufficient input returns `Ok(CheckReport)`.

The default `prepare` feature uses `zincite-syntax` for CST traversal and source
locations, and includes the native CLI. Disabling default features excludes the
parser and CLI. `PreparedModel::from_json` and `check` also work with
a path dependency on this checkout:

```toml
[dependencies]
zincite-check = { path = "../zincite/crates/zincite-check", default-features = false }
```

Adjust the path relative to the consuming project's `Cargo.toml`. This checker
has not been published to the registry. Supply `Assignment` values directly in
that build. `from_json`
returns a `Diagnostic` on malformed input; `PreparedModel` does not implement
`Deserialize`, so deserialization cannot bypass admission checks.

The optional `wasm` feature exposes a JavaScript `PreparedModel` constructor and
`check` method through wasm-bindgen. Build it with `--no-default-features` to
exclude parsing and the native CLI. Typed assignment JSON and report JSON use
the shared Rust records and exact decimal integer strings. The
[Wasm example](../../examples/checker-wasm/README.md) gives repeatable builds for
Node.js and browser ES modules, the assignment format, and an executable
native/Wasm report comparison. It loads exported artifacts without source files
or a MiniZinc installation.

## Accepted model and assignment forms

- Scalar Boolean, signed 64-bit integer, finite binary64 float and finite integer
  set parameters and variables. Fixed float sets serve membership arguments. Integers use decimal, `0x` hexadecimal or `0o` octal spelling, with
  an optional minus sign. Integer domains use literal inclusive intervals or
  enumerations. A set domain is an upper bound: `var set of 1..3` allows every
  subset, including the empty set. `var set of int` admits finite i64 sets.
- Parameters require literal initialization and have no annotations. Variables
  accept fixed literals, earlier scalar references, literal arrays indexed by
  `1..n` (including `1..0`), and earlier same-shape array aliases. Literal array
  access such as `values[2]` selects a scalar; bounds and types are checked.
- Items follow predicate, parameter, variable, constraint, solve order. There
  is exactly one final solve item: `satisfy`, or `minimize`/`maximize` of a scalar
  integer or float literal/reference. An available objective value appears in reports;
  the checker makes no optimality claim.
- Exactly the predicates and aliases in the coverage table below. Integer and Boolean
  linear coefficients are fixed integers; integer-linear bounds are fixed integers
  and Boolean-linear bounds accept integer operands. Float linear coefficients and
  bounds are fixed finite floats. Coefficient and operand arrays have equal lengths.
  Bodyless declarations for these names validate types, fixed/variable roles and
  scalar/array shapes independently of formal spelling. A declared fixed length
  or restricted domain also constrains calls. `fzn_disjunctive_strict` requires
  its declaration; other supported calls can omit declarations. Unknown, duplicate,
  mismatched, annotated and body-bearing declarations diagnose.
- Application annotations `checker_value(string, literal_integer_array)` on
  variables and `checker_constraint(string, literal_integer_array)` on constraints
  preserve IDs and exact integer entity payloads. Equal repeated annotations are
  accepted; conflicting application metadata diagnoses. Strings support C escapes,
  including octal and hexadecimal bytes, and decode to UTF-8. `mzn_path(string)`
  and `mzn_path(int)` supply paths; `mzn_constraint_name(string)`,
  `constraint_name(string)` and direct string annotations supply constraint labels.
  Expression names and documentation strings are accepted as metadata.
- `is_defined_var` and `defines_var` must match a scalar variable in a unique
  defining relation. Supported completion directions appear below; other
  well-formed directions can be checked with supplied values.
- Variable output markers `output_var` and `output_array` are metadata.
  `output_array` accepts integer-set dimensions, including enumerations,
  non-1-based dimensions, multiple dimensions and empty dimensions. Their
  cardinality product must equal the flat array length. Assignment positions
  still use `1..n`; no output renderer is provided. MiniZinc output-section
  annotations belong to output items, which are outside FlatZinc preparation.
- Standard/Gecode search metadata includes `int_search`, `bool_search`,
  `set_search`, `float_search`, nested `seq_search`, `warm_start`/`warm_start_array`, `int_assign`,
  integer/Boolean/set/float default searches, restart strategies and
  `relax_and_reconstruct`. Referenced arrays, fixed values, lengths and strategy
  roles are validated recursively. `restart_geometric` accepts a finite float
  literal base greater than one and a positive integer scale. Float search precision
  is a finite fixed float metadata argument, separate from checker tolerance.
- Propagation metadata on constraints includes `domain`, `bounds`,
  `domain_propagation`, `bounds_propagation`, `value_propagation`, and Gecode's
  basic, advanced, full and decomposed propagation choices. Introduced-variable,
  checking and compiler context flags are recognized in their contexts. Search,
  propagation and output selection do not change candidate feasibility.

Unquoted variable and parameter names may start with a letter or underscore;
predicate and annotation names start with a letter. `.dzn` assignments accept
the literal values above and flat arrays of them. Set enumerations may repeat
or reorder members; preparation normalizes them. A reversed interval is empty.
No missing-value placeholders, expressions, indexed assignment targets or solver
solution streams are accepted. Native CLI input uses `.dzn`; the Wasm API uses
typed assignment JSON. A partial candidate may name available scalars; an array
assignment must supply every element.

Unknown names, duplicate entries, contradictory explicit assignments through
aliases, wrong types and wrong array lengths are input errors. Assignments
cannot change parameters. A correctly typed value outside a logical domain is a
violation. Fixed values, singleton integer domains and empty set upper bounds
establish defaults. A singleton set upper bound allows two subsets and supplies
no default. A definition's output slot, including aliases, does not fall back
to a domain default when inputs are missing. Supplied values take precedence.

`computed_domain` explicitly marks a convenience domain rather than a problem
restriction. Preparation converts it to an unbounded domain of the same type:
it supplies neither a domain violation nor a singleton default. Logical domains
on other aliases still apply; fixed initializers still supply their exact value.
The existing executable format expresses this distinction without extra fields,
so validated loading and parser-free Wasm checking use the same rules.

Other set operations, includes, output expressions, computed
initializers/objectives, additional predicates and unknown semantic annotations
are rejected explicitly. External executable/DLL callbacks and restart-history
hooks also diagnose; the checker does not execute search or external code.
Syntax acceptance by the shared parser does not imply checker support.

## Elements and conditionals

Selection supports Boolean, integer, finite integer-set and float values. Fixed
array spellings require fixed entries; `array_var_` spellings accept variable or
fixed entries. Float sets are parameters and cannot be selected values.

| Calls | Indexing and fixed arguments |
| --- | --- |
| `array_[var_]bool/int/set_element(index,values,result)` | One-based position; bare names and exact `gecode_`/`fzn_` prefixes |
| `array_[var_]float_element(index,values,result)` | One-based position; bare names only |
| `array_var_bool/int/set/float_element_nonshifted(index,values,result)` | One-based position over admitted `1..n` FlatZinc arrays; bare names only |
| `gecode_[var_]int/bool_element(index,offset,values,result)` | Position `index-offset`; fixed offset; variable or fixed array entries |
| `gecode_int/bool_element2d(row,column,values,rows,columns,result)` | Row-major flat array; fixed nonempty contiguous index sets; exact rectangular length |
| `gecode_set_element2d(values,column,width,row,height,result)` | Zero-based column and row; fixed positive width/height; exact rectangular length |
| `gecode_bool/int/set/float_ite(condition,then_value,else_value,result)` | Boolean condition chooses one typed input |

Every native `gecode_` name in the last four rows also accepts its literal
`gecode_` and `fzn_` registry prefixes, including doubled `gecode_gecode_` names.
Both matrix axes are checked before calculating the flat position. Empty 1D
arrays have no valid index. Out-of-range indices produce `element_range`
findings even when the result or another coordinate is unavailable. Findings
carry available typed indices, fixed layout and optional result values. Matrix
finding indices use `[row,column]`, including set matrix calls.
`element_mismatch` and `conditional_mismatch` carry the selected input and result
with variable references and the constraint's application metadata. Integer,
Boolean and set values compare exactly; floats use the model's saved tolerance.

A supported `defines_var` direction derives only the result. An unknown selector
blocks that relation on the selector; a known valid selector needs only the
selected input and result. An output in an unselected branch does not prevent
completion. A selected self-reference supplies no value. Independent unresolved
variables and domain checks still make the report incomplete. Supplied results
are checked without replacement, and each check starts with fresh values.

The nonshifted 1D calls operate on the admitted FlatZinc array positions. MiniZinc
must lower original source offsets before checking. Two-dimensional nonshifted
calls require source array shape information and follow compiler decomposition;
`if_then_else`/`fzn_if_then_else` wrappers also follow their compiler routes.
Direct support for the calls above does not establish full wrapper/profile closure.

Two compile-only controls use the pinned standard and Gecode libraries:

```sh
mkdir -p target/checker-selection-controls
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-selection-controls/layouts.paths --output-fzn-to-file target/checker-selection-controls/layouts.fzn examples/checker-compilation/element-layouts.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-selection-controls/conditionals.paths --output-fzn-to-file target/checker-selection-controls/conditionals.fzn examples/checker-compilation/conditional-values.mzn
```

MiniZinc 2.10.1 emits `gecode_int_element(index,-3,[1,entry,7],selected)`
and a row-major 2×3 `gecode_int_element2d` with row set `-2..-1` and column set
`4..6`. The candidate `entry=5;index=-2;row=-1;column=5;` derives both results
as 5. Supplying `selected_matrix=6` gives one element mismatch. The conditional
control emits the four native `ite` calls; `condition=true;then_bool=true;`
derives `true`, `2`, `{1,3}` and `1.25`. Changing `chosen_set` to `{2}` gives
one conditional mismatch. Both compilations preserve `defines_var`, labels,
paths and application identifiers/payloads. They start no solver.

The handwritten `examples/checker-compilation/selection.fzn` combines direct
calls for the native/Wasm smoke. It also checks output aliases in unselected
branches and compares selected float outputs under saved and custom tolerance.

## Float policy

Finite binary64 floats work as scalars, domains, arrays, aliases, objectives and
assignments. Decimal and exponent literals preserve their parsed binary64 value;
NaN and infinities diagnose. Signed zero becomes positive zero. No implicit
integer-to-float conversion occurs. JSON floats use finite numbers, for example
`{"kind":"float","value":1.25}`; integers keep decimal strings.

Each prepared model owns `FloatTolerance`: absolute and relative tolerance both
default to `1e-9`. Settings must be finite and nonnegative. Zero/zero gives exact
checking. Equality is
`abs(a-b) <= max(absolute,relative*max(abs(a),abs(b)))`.
The binary64 evaluation first accepts exact equality, then a finite absolute
distance within the absolute setting. If the relative setting is nonzero, let
`m=max(abs(a),abs(b))` and compare `abs(a-b)/m` with that setting. When subtraction
overflows, use `abs(a*0.5-b*0.5)/(m*0.5)` instead. Boundary results follow these
operations; no overflowing threshold is formed. Disequality is the complement.
Strict less means raw `a<b` and unequal; non-strict less means raw `a<b` or equal.
Greater comparisons mirror these rules. This policy governs comparisons,
reification, implication, float result checks and runtime domain membership.

Alias contradictions, fixed/default identity and singleton eligibility use exact
values. Tolerance supplies no missing value. A near-singleton interval supplies
no default. Reversed intervals are empty before tolerant membership checks.
`float_dom(x,endpoints)` uses complete consecutive pairs; it ignores an odd
trailing endpoint, and empty data denotes an empty union. `float_set_in` accepts
fixed sets of singleton/range alternatives; their executable transport is an
ordered nonoverlapping range list. Assignable float sets are unsupported.

Arithmetic checks every required intermediate for finiteness, in source order
for linear multiplication and accumulation, starting from zero. Underflow is
allowed. Zero divisors and nonfinite intermediates produce located execution
errors. Only `int2float` converts i64 values, with nearest-even binary64 rounding.
Distinct integers above `2^53` can convert to the same float while their original
integer values stay distinct. `float_ceil`, `float_floor` and `float_round` produce
exact i64 outputs; round ties go away from zero. Results must lie in
`[-2^63,2^63)` before casting, with an exclusive upper boundary.

Supported spellings are bare `float_{eq,ne,le,lt}` and `float_lin_{eq,ne,le,lt}`,
with `_reif` and `_imp` forms. The registered scalar/linear eq/le/lt forms and
reifiers, scalar ne, `float_abs`, `float_plus`, `float_times`, `float_div`,
`float_min`, `float_max` and `int2float` also accept their exact `gecode_` and
`fzn_` aliases. Other float-core names have only their bare spelling.
`gecode_float_rel_imp_fzn(x,code,y,guard)` also accepts the literal aliases
`gecode_gecode_float_rel_imp_fzn` and `fzn_gecode_float_rel_imp_fzn`. Codes 1–6
mean equal, not equal, less-or-equal, less, greater-or-equal and greater.
A false guard skips missing numerical operands/calculations; it is never an
output. Float membership and ceil/floor/round have only bare names. Float selection
uses the same saved comparison policy as float-core.

Roots, powers and transcendental functions use the generic Rust implementation in
`libm` 0.2.16, with default features disabled. Native and Wasm use this common
backend without host math, MPFR or solver libraries. Strict checking compares its
computed binary64 result exactly; another compiler's constant folding or an
interval solver's rounding can differ. The saved default/custom comparison policy
also applies to these results. Domain admission always uses exact bounds.

The unary operations are `float_{sqrt,exp,ln,log10,log2,sin,cos,tan,asin,acos,atan}`
and `float_{sinh,cosh,tanh,asinh,acosh,atanh}`. Only the first group also accepts
the exact `gecode_` and `fzn_` prefixes. Trigonometric inputs are radians; inverse
functions return principal values. `sqrt` requires `x >= 0`, logarithms `x > 0`,
`asin`/`acos` `-1 <= x <= 1`, `acosh` `x >= 1`, and `atanh` `-1 < x < 1`.
The other functions accept any finite input whose result is finite. Finite tangent
arguments close to irrational poles are accepted; the checker adds no pole heuristic.

`float_pow(base,exponent,result)` takes float operands. Negative bases require an
exactly integral binary64 exponent; a zero base rejects negative exponents.
Standard `0^0` is 1. Negative exponents use the backend's power function directly,
so an overflowing positive power does not prevent a finite reciprocal result.
`gecode_float_pow_base(base,exponent,result)` instead requires a fixed positive
float base, including 1. `gecode_float_pow(base,exponent,result)` takes an exact
fixed integer exponent in `0..2147483646`; it rejects `0^0` to match Gecode's
nonzero-base requirement for exponent zero. Checked exponentiation by squaring
keeps this exponent as an integer, transported as a decimal string. Both Gecode
power calls also accept their literal `gecode_` and `fzn_` registry prefixes.

Standard `array_float_minimum(result,values)` and `array_float_maximum(result,values)`
put the result first; `gecode_float_array_min(values,result)` and
`gecode_float_array_max(values,result)` put it last and accept their literal registry
prefixes. All require nonempty float arrays. Exact numeric order selects and copies
an input value; tolerance only compares a supplied result.

Each function can define its unique result through `defines_var` when all inputs
are available and the result does not occur among the inputs through an alias.
Supplied results are checked without replacement. Derived values feed domains and
downstream checks even when a declaration domain fails. Missing inputs produce
structured incompleteness alongside established violations. Integer-power findings
carry the typed base/result and exact exponent string; array-extremum findings
carry the operation, input arguments and result. Scalar functions use the existing
float-arithmetic finding.

Preparation and loading reject wrong fixed roles, invalid fixed bases, integer
exponents outside the supported range and empty extrema arrays. Undefined domains,
zero denominators and nonfinite calculations give located execution errors.
Underflow is allowed, including zero; signed zero normalizes to positive zero.
An underflowed result used as a divisor diagnoses zero division, and a subsequent
float-to-integer conversion still checks its i64 range. These direct operations do
not establish full compiler-profile closure; checker-030/031 own that work.

Rust callers configure the model directly:

```rust
use zincite_check::{FloatTolerance, prepare};
let mut model = prepare("model.fzn", "var float:x; solve minimize x;").unwrap();
assert_eq!(model.float_tolerance(), FloatTolerance::default());
model.set_float_tolerance(FloatTolerance::new(1e-6, 1e-8).unwrap());
model.set_float_tolerance(FloatTolerance::new(0.0, 0.0).unwrap()); // exact
```

Both CLI entry points accept `--float-absolute-tolerance NUMBER` and
`--float-relative-tolerance NUMBER`. Each option changes only its named setting.
Source checking/export starts from defaults; loading starts from saved settings.
New version-1 exports save both settings; old version-1 artifacts without them
load with defaults. For example, after building the native binaries:

```sh
target/debug/zincite-check --float-absolute-tolerance 1e-6 examples/checker-compilation/float-core.fzn examples/checker-compilation/candidates/float-valid.dzn
target/debug/zincite check --float-absolute-tolerance 0 --float-relative-tolerance 0 examples/checker-compilation/float-core.fzn examples/checker-compilation/candidates/float-valid.dzn
target/debug/zincite-check --float-relative-tolerance 0 --export target/float-core.json examples/checker-compilation/float-core.fzn
target/debug/zincite-check --load --float-absolute-tolerance 1e-6 target/float-core.json examples/checker-compilation/candidates/float-valid.dzn
```

Two small compiler controls exercise these primitives and Gecode's coded guard.
With the pinned clean library roots from the coverage inventory, compile them
without running a solver:

```sh
mkdir -p target/checker-float-controls
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-float-controls/primitive.paths --output-fzn-to-file target/checker-float-controls/primitive.fzn examples/checker-compilation/float-primitives.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-float-controls/coded.paths --output-fzn-to-file target/checker-float-controls/coded.fzn examples/checker-compilation/float-implication.mzn
```

The primitive candidate `x=1.0; total=1.25; rounded=2; small=true;` is valid;
changing `rounded` to 1 fails the compiled ceiling relations. The coded candidate
`x=1.0; guard=true;` is valid; `x=2.5; guard=true;` violates the guarded ordering.
These controls cover emitted definitions, paths, labels and float search metadata;
they do not establish complete standard/Gecode compiler profiles.

The [float-function control](../../examples/checker-compilation/float-function-controls.mzn)
exercises roots, standard power syntax, logarithms, hyperbolic cosine and an array
maximum against both pinned profiles:

```sh
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-float-controls/functions-standard.paths --output-fzn-to-file target/checker-float-controls/functions-standard.fzn examples/checker-compilation/float-function-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-float-controls/functions-gecode.paths --output-fzn-to-file target/checker-float-controls/functions-gecode.fzn examples/checker-compilation/float-function-controls.mzn
```

The standard profile emits `float_sqrt`, `float_pow`, `float_cosh`, `float_ln`
and a `float_max` chain. Supplying `x=2.0;` gives a valid complete candidate.
The Gecode profile emits `gecode_float_pow(x,3,cube)`,
`gecode_float_pow_base(2.0,x,exponential)` and `gecode_float_array_max`, and
expands cosh through two `float_exp` calls and float linear equations. Its power
and extrema wrappers drop the forward definitions, so the inspected candidate
also supplies `cube=8.0; exponential=4.0; X_INTRODUCED_27_=8.0;`. That introduced
name belongs to this compiled control; inspect new output before supplying it.
Without those values, the Gecode artifact gives an incomplete report. Changing
`cube` to 7 gives typed power and array-extremum findings. The direct
[FlatZinc control](../../examples/checker-compilation/float-functions.fzn) has
explicit forward annotations and needs only its primary input.

Both profiles preserve paths, source labels and the scalar application IDs.
Gecode's array rewrite drops the array application ID while keeping its label.
A separate inspected variant using an explicit named
`array_float_maximum(maximum_value,[root,cube,exponential])` call emitted a
standard `defines_var(maximum_value)` without its `is_defined_var` marker;
preparation diagnoses that mismatch. To reproduce, replace the maximum-expression
constraint with that call and declare `var float: maximum_value;`, using the same
standard command. These controls establish their observed behavior; full profile
closure and preservation of editing metadata belong to checker-030/031.

Wasm uses `setFloatTolerance(absolute,relative)` with the same validation;
`floatTolerance()` returns the current settings as JSON. The saved policy and
correctly rounded finite transport give identical native/Wasm reports; the smoke
compares reports exactly. Search precision never changes checker tolerance.

## Functional completion

The float names and directions are listed in the float policy above.
The table below covers integer, Boolean and set operations, including constraints emitted
by both application models:

| Predicate | Direct check | Completion through matching `defines_var` |
| --- | --- | --- |
| `int_eq(x,y)` | `x = y` | Either integer output if it occurs once |
| `int_ne`, `int_lt`, `int_le`, `int_gt`, `int_ge` `(x,y)` | `x != y`, `x < y`, `x <= y`, `x > y`, `x >= y` | None |
| `int_lin_eq(a,x,c)` | `sum(a[i]*x[i]) = c` | One integer output occurring once with a nonzero coefficient; all other terms known and exact division |
| `int_lin_ne`, `int_lin_lt`, `int_lin_le`, `int_lin_gt`, `int_lin_ge` `(a,x,c)` | Compare `sum(a[i]*x[i])` with `c` using the named relation | None |
| Each scalar/linear comparison with `_reif` and final Boolean `b` | `b <-> relation`, including `b=false` | Boolean `b` from all known integer inputs |
| Each scalar/linear comparison with `_imp` and final Boolean `b` | `b -> relation`; a false guard satisfies the constraint immediately | None |
| `set_in(x,s)`, `int_in(x,s)` | Integer membership; Boolean membership maps false/true to 0/1 against fixed sets only; `int_in` always requires a fixed set | None |
| `set_eq(s,t)`, `equal(s,t)` | Set equality | Either set output when it occurs once |
| `set_ne/le/lt/subset/superset(s,t)` | Disequality, sorted-element order, subset/superset inclusion | None |
| These comparisons and membership with `_reif` and final Boolean `b` | `b <-> relation`, including `b=false` | Final Boolean `b` |
| Bare `set_eq/ne/le/lt/subset/superset_imp(s,t,b)` and `set_in_imp`/`int_in_imp` | Guard implies the relation; false skips unavailable inputs | None |
| `set_card(s,n)` | Checked i64 cardinality equals `n` | Integer output `n` |
| `set_union/intersect/diff/symdiff(s,t,r)` | Exact compact range algebra | Final set `r`, absent from inputs |
| `gecode_set_rel_imp_fzn(s,code,t,b)` | Fixed native code 1–10; guard implies the coded relation | None |
| `fzn_disjunctive_strict(start,duration)` | Every pair satisfies `start[i]+duration[i] <= start[j]` or the reverse | None; durations must be fixed nonnegative integers |
| `int_abs(x,r)`, `int_negate(x,r)` | `r = abs(x)`, `r = -x` | Final argument `r` |
| `int_plus(x,y,r)`, `int_minus(x,y,r)`, `int_times(x,y,r)` | `r = x+y`, `r = x-y`, `r = x*y` | Final argument `r` |
| `int_div(x,y,r)`, `int_mod(x,y,r)` | Truncated quotient or modulus, with the dividend's sign | Final argument `r` |
| `int_min(x,y,r)`, `int_max(x,y,r)` | Minimum or maximum | Final argument `r` |
| `int_pow(x,y,r)`, `int_pow_fixed(x,y,r)` | Integer power; the latter requires a fixed exponent | Final argument `r` |
| `gecode_int_pow(x,y,r)`, `gecode_gecode_int_pow(x,y,r)`, `fzn_gecode_int_pow(x,y,r)` | Integer power with fixed exponent in `0..2147483646` | Final argument `r` |
| Boolean scalar comparisons `bool_eq/ne/lt/le/gt/ge(x,y)` | Compare Boolean values with `false < true` | Either unique operand for equality or disequality |
| Boolean scalar and linear comparisons with `_reif` | Final Boolean equals the named comparison | Final Boolean result |
| Boolean scalar and linear comparisons with `_imp` | Final Boolean implies the named comparison | None |
| `bool_not(x,y)` | `y = not x` | Either unique Boolean operand |
| `bool_and/or/xor(x,y,b)` | Final Boolean equals AND, OR or XOR | Final result `b` |
| Bare `bool_xor(x,y)` | Assert XOR | Either unique operand as the other's complement |
| `bool_left_imp(x,y,b)`, `bool_right_imp(x,y,b)` | `b = (y -> x)`, `b = (x -> y)` | Final result `b` |
| `array_bool_and/or/xor(x)` or `(x,b)` | Assert a true reduction or equate its result with `b` | Final result in the two-argument form |
| Scalar `bool_and/or/xor_imp` and reduction `_imp` | Final Boolean guards the named calculation | None |
| `bool_clause(p,n)` | Some positive literal is true or some negative literal is false | None |
| `bool_clause_reif(p,n,b)`, `bool_clause_imp(p,n,b)` | Final Boolean equals the clause; pinned Gecode `_imp` also uses equivalence | Final result `b` |
| `bool2int(b,n)` | `false=0`, `true=1` | Either unique output; integer-to-Boolean only for 0 or 1 |
| `bool_lin_eq(a,x,c)` | `sum(a[i]*int(x[i])) = c` | Unique integer bound or one nonzero-coefficient Boolean term giving exactly 0 or 1 |
| `bool_lin_ne/lt/le/gt/ge(a,x,c)` | Compare that sum and integer bound `c` | None |

The 26 assigned set registry literals accept their exact bare, `gecode_` and
`fzn_` spellings, including `gecode_gecode_set_rel_imp_fzn` and
`fzn_gecode_set_rel_imp_fzn`. The six named set comparison `_imp` forms are
bare-only source names. Together they give 84 set names. There is no `equal_imp`,
algebra reifier or cardinality reifier. Matching bodyless membership declarations
can use a fixed set; calls must satisfy that declared role.

Named `set_le` and `set_lt`, their reifiers, implications and named aliases
compare the sorted element lists: `{} < {1,3} < {2}`. A proper prefix is smaller.
Native codes mean `1 =`, `2 !=`, `3 subset`, `4 superset`, `5 disjoint`,
`6 complement`, `7 <=`, `8 <`, `9 >=`, `10 >`. Codes 7–10 use Gecode's
characteristic order: compare membership bits from the smallest differing
element, with `0 < 1`. In that order `{2} < {1}`, and the empty set is smallest.
Typed findings and text identify which order the checker used. Code 6 requires
both sets inside `-1073741822..1073741822`, disjoint and covering that universe
exactly. Preparation rejects an invalid code even under a false guard.

Difference, symmetric difference and both comparisons advance through ranges
without expanding large intervals. Cardinality and downstream integer arithmetic
report located checked-i64 overflow errors. Serialized models and assignment
JSON reject reversed, unsorted, overlapping or adjacent ranges and numeric JSON
endpoints; source lists normalize duplicates and ordering, and reversed source
intervals denote the empty set.

The direct [set specimen](../../examples/checker-compilation/set-primitives.fzn)
is shared by serialization and the native/Wasm comparison. The separate
[MiniZinc control](../../examples/checker-compilation/set-primitives.mzn) uses
small domains to inspect actual compilation against both pinned libraries:

```sh
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir crates/zincite-check/minizinc -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker017-standard.paths --output-fzn-to-file target/checker017-standard.fzn examples/checker-compilation/set-primitives.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker017-gecode.paths --output-fzn-to-file target/checker017-gecode.fzn examples/checker-compilation/set-primitives.mzn
```

Both controls emit `set_diff`, `set_symdiff`, `set_card`, `set_in_reif` and
`set_lt`. The four derived outputs carry `is_defined_var`/`defines_var`;
application IDs, entities, labels and paths survive. With `left={1}` and
`right={2}`, completion gives `difference={1}`, `changed={1,2}`, `size=2` and
`member=true`, and sorted order passes. Swapping the sets gives
`difference={2}`, `member=false` and one order finding. Omitted decisions give
an incomplete report. This control does not establish either complete compiler
profile or general preservation of editable domains and variables.

All 36 scalar/linear comparison forms accept exactly the bare, `gecode_` and
`fzn_` spellings, for 108 integer relation names. The 55 Boolean base names also
accept those exact prefixes, for 165 Boolean names and 761 admitted names overall.
Prefixed scalar XOR uses three arguments; only bare `bool_xor` also admits the
two-argument assertion. Matching declarations select one admitted arity and calls
must use that arity; duplicate declarations diagnose.
For example, `fzn_int_lin_gt_reif(a,x,c,b)` is full reification, while
`gecode_int_ge_imp(x,y,b)` checks the comparison only when `b` is true.
Linear arrays may be empty, with sum zero. A false implication guard bypasses
missing integer terms and checked accumulation, including otherwise-overflowing
products. Unresolved variables and their domain checks still make the report
incomplete. Strict comparisons use direct `<` and `>` at the i64 boundaries.

Empty Boolean AND is true; empty OR, XOR and clauses are false. Clause negative
arrays contain the variables to complement. Standard `bool_lin_eq` declares a
variable integer bound. Standard `bool_lin_le` declares a fixed bound, while
Gecode declares a variable bound. Both written `bool_lin_le` signatures work;
a written fixed declaration requires fixed bounds at its calls. Calls without
a declaration accept an integer variable bound for every Boolean linear form.
Their sums use checked i64 arithmetic in source order, with `false=0` and `true=1`.

Boolean functional results and full reifiers derive from known inputs. Equality,
disequality, negation and asserted scalar XOR can also copy or complement the
other unique operand. Boolean channeling and linear equality derive only the
unique directions in the table. Output occurrences count after alias resolution,
including bounds and conditions. A repeated output, nonexact division or result
outside 0/1 leaves a Boolean output unresolved. Ordinary false implication guards
skip missing inputs and arithmetic; false reifiers still evaluate their relation.
The pinned Gecode `bool_clause_imp` and its exact aliases use equivalence and can
define their final Boolean. This exception follows its registered handler.

The nine arithmetic names from `int_abs` through `int_max` also accept their
exact `gecode_` and `fzn_` prefixes, such as `gecode_int_minus` and `fzn_int_negate`.
Standard `int_pow` and `int_pow_fixed` have no additional prefix aliases.
Arithmetic completion requires a final output that occurs exactly once after
alias resolution, all inputs available and a matching definition annotation.
The checker does not calculate missing arithmetic inputs by reversing a relation.

Before checking domains and relations, a dependency worklist calculates the
supported `defines_var` outputs until no new values are available:

- `int_lin_eq(a,x,c)` defines an integer output that occurs exactly once with
  a nonzero coefficient. Its value is `(c - sum(other terms)) / coefficient`,
  only when all other terms are known and the division is exact. Negative
  coefficients and results are supported; nonexact division leaves the output
  unresolved.
- Plain `int_eq(x,y)` can copy either unique output from the other operand.
- Each scalar or linear `_reif` form defines only its Boolean result from known
  integer operands. A false comparison derives `false`; `_imp` never derives
  its guard.

The checker does not invert inequalities or reification, combine repeated
output occurrences, or choose values for unresolved cycles. A cycle with
enough supplied values can be checked. Each call starts completion afresh,
including after changing or removing primary assignments. Set definitions use
the directions in the table; membership and subset checks do not select members.

Every defining relation is also checked. A supplied output is used as written,
so its defining relation can report a conflict. A derived value outside its
domain feeds downstream definitions and checks; its domain violation does not
suppress other findings.

## Reports, integers and command status

`CheckReport` carries typed `ViolationKind` records, declaration/constraint
references, application IDs and integer entity payloads, source ranges,
constraint ordinals, labels and path annotations. Variable references also list
aliases and one-based array positions. Violations and blocked checks follow
FlatZinc source order. An ordinal identifies a constraint in one prepared model.

Completeness is independent of violations. `status` is `valid` only when every
variable and check is resolved and every check passes; it is `invalid` when a
violation is known, and `incomplete` otherwise. The `completeness` record always
includes its own `complete` flag, unresolved variables and blocked domain or
constraint checks with missing references. An incomplete report does not claim
that a satisfying completion exists.

All mathematical integers use `i64`. Linear products and accumulation use
checked arithmetic in array order. Completion also checks subtraction and exact
division. Set cardinality and strict disjunctive endpoints also use checked i64
results. Overflow, including intermediate overflow and `i64::MIN / -1`, is a
located execution error. A zero output coefficient is an unsupported direction,
so linear completion does not divide by zero. Integer division truncates toward
zero; modulus has the dividend's sign, with `MIN mod -1 = 0`. A zero divisor
is an execution error. Power defines `0^0 = 1` and rejects negative powers of
zero. Standard negative exponents calculate a checked absolute magnitude and
checked denominator, then `1 div denominator`. Exponent `MIN` and denominator
overflow are errors, even if the mathematical quotient would truncate to zero.
There is no u32 exponent cutoff, and `(-2)^63` fits the i64 boundary. Native
Gecode power instead requires its fixed exponent in `0..2147483646` during
preparation and loading. Arithmetic findings use `integer_arithmetic`, a typed
`operation`, integer `inputs` and the supplied `result` with variable references.
The existing `linear` and `reified_less_equal` finding kinds keep their shapes.
New comparisons use `integer_relation` or `linear_relation` with a typed `relation`
(`equal`, `not_equal`, `less`, `less_equal`, `greater`, `greater_equal`), typed
operands and a `condition`: `plain`, `reified` with its Boolean `result`, or
`implied` with its Boolean `guard`. Linear records also carry coefficients, sum
and bound. Boolean findings use `boolean_relation`, `boolean_linear`,
`boolean_operation`, `boolean_clause` and `boolean_to_integer`. Their arguments
carry Boolean/integer values and variable references; conditions distinguish
assertions, results and guards. Boolean linear findings carry a typed integer
bound argument as well as exact coefficients and sum.
Integer sets use compact sorted, disjoint, nonadjacent inclusive ranges; large
intervals are never expanded just to store a value. Cardinality above i64::MAX is
a located execution error. JSON writes values, coefficients, sums, bounds, domain endpoints
and application payload integers as decimal strings, so consumers do not round
integers beyond 2^53. Booleans stay Boolean; source offsets, ordinals and array
positions are ordinary JSON numbers. Report JSON is an output format, not a
prepared-model loading format.

Both commands accept `--format text` (the default) or `--format json`, followed
by a model path and an assignment path. With `--load`, the model path names
the prepared JSON artifact. Reports go to stdout. Errors go to
stderr with no partial stdout, including in JSON mode. Checking writes no input
files. `--export` creates a new output file.

| Status | Meaning |
| --- | --- |
| 0 | Valid, complete candidate or successful export |
| 1 | Invalid or incomplete report |
| 2 | Usage, I/O, syntax, preparation, loading, assignment or arithmetic error |


## Interactive measurements

The [measurement report](../../scripts/checker-performance.md) records a release
build on the user's M1 Max: 60 individual edits of the existing two-job schedule
gave native checking p95 of 0.0083 ms and browser assignment-to-parsed-report p95
of 0.20 ms, below the 100 ms target for that named example. It separates source
preparation, serialized loading, native checking and Wasm/JSON work, and gives
size and memory measurements for 10 and 100 independent copies. DOM rendering
and other models need their own measurements. The report includes the small
saved-run driver and exact repeat commands.


## Counts and value distributions

The checker admits 54 counting names from the 18 registry literals in
[family 019](../../docs/checker-coverage.md#family-019). Each literal has exactly
the empty, `gecode_` and `fzn_` prefixes. Prefixes apply to the whole literal;
`gecode_global_cardinality` therefore also accepts
`gecode_gecode_global_cardinality` and `fzn_gecode_global_cardinality`.
Unregistered wrappers such as `fzn_global_cardinality` and `fzn_count_geq` are
not direct calls.

Occurrence counts count positions, including repeated aliases. `among` uses a
fixed compact integer set; `nvalue` counts distinct concrete values. Positional
matches compare each entry with its fixed partner without subtracting integers.
Coded counts use `calculated_count relation supplied_count`, with codes 1–6 for
`=`, `!=`, `<=`, `<`, `>=`, `>`. Fixed at-least/at-most counts, bounded GCC and
windows have no count output. Equality derives only its count; full reification
derives only its Boolean. A false implication guard skips unavailable operands.
`count([], needle, n)` calculates zero without choosing a missing needle. The
needle's unresolved variable and domain still make the report incomplete;
a known count conflict can coexist with that state.

Open cardinalities allow values outside the cover. Closed cardinalities report
every out-of-cover position. Duplicate covers impose all their counts or bound
intervals simultaneously. Empty entries give zero counts; an empty closed cover
excludes every supplied entry. Negative, oversized and contradictory bounds are
ordinary mathematical comparisons. Integer and Boolean windows check every
contiguous window, require nonempty entries and a length in `1..length(entries)`,
and use a fixed set or Boolean membership value.

Exact GCC arrays support `defines_var(counts)` and `defines_var([first, second])`.
Target order does not assign count positions. Each target must identify the
count array's canonical slots with `is_defined_var` on their
canonical declarations or their assignable array declaration. The artifact keeps
scalar/named targets as JSON strings and writes explicit targets as JSON arrays
of scalar names. Repeated output aliases calculate a value only when all their
cover positions agree. Input/output cycles need supplied values. Partial supplied
outputs are checked without being overwritten; derived domain failures still
feed downstream checks. Every candidate starts with fresh values.

Typed `count`, `cardinality` and `count_windows` findings expose calculated and
supplied counts, selectors, codes, conditions, cover positions, outside-cover
positions and window positions. An irrelevant unavailable needle includes its
variable reference. Mathematical integers use decimal strings; positions use JSON
numbers. Existing scalar metadata, artifacts and report kinds keep their shapes.

The small controls compile without solving against both pinned profiles:

```sh
mkdir -p target/checker-counting-controls
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir crates/zincite-check/minizinc -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-counting-controls/standard.paths --output-fzn-to-file target/checker-counting-controls/standard.fzn examples/checker-compilation/counting-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-counting-controls/gecode.paths --output-fzn-to-file target/checker-counting-controls/gecode.fzn examples/checker-compilation/counting-gecode-controls.mzn
```

Both source controls were formatted and default-linted with the pinned standard
library and the checker annotation include. Both also compiled under the other
profile. The standard decomposition emits `is_defined_var` on a `bool_clause`
constraint; preparation rejects that context. The Gecode profile emits
`fzn_count_eq`, `fzn_nvalue`, `fzn_among` and native GCC, but drops definition
metadata for the distinct-value and GCC results. It also replaces the source
`counts` array name with an introduced name. Supplying those outputs gives a
valid complete report. The explicit native control emits named GCC counts and
both membership window types with labels, paths and application payloads, but
MiniZinc drops its GCC `defines_var` and scalar `is_defined_var` markers in both
profiles. Supplying only entries and flags therefore gives an incomplete report;
supplying the count outputs checks completely. These are checker-030/031 profile
gaps; runtime marker validation stays strict.

The direct [counting control](../../examples/checker-compilation/counting.fzn)
keeps valid scalar markers and a named array target. Both CLI forms and validated
export/load derive the counts, positional matches and reifier. `x=1;` is valid
and complete, `x=1; ones=9;` has typed cardinality and count conflicts, and an empty
candidate is incomplete. The native/Wasm smoke covers these same three edits.

## Ordering and integer channels

The checker admits 108 ordering names from 36 registry literals. Each literal
accepts its bare, `gecode_` and `fzn_` expansion,
including literals already beginning with `gecode_`. See the
[coverage inventory](../../docs/checker-coverage.md#family-020) for exact names.
Declarations validate positional fixed/variable roles independently of formal names.

Distinctness checks every position pair, including repeated aliases. Optional
presence flags exclude absent entries; except-distinct allows duplicates of its
fixed excluded value. Offset-distinct adds fixed offsets with checked i64 sums.
All-equal and increasing/decreasing allow empty/singleton arrays and equality;
not-all-equal is false for empty/singleton arrays. Boolean order is false < true.
Lexicographic arrays may have different lengths: compare the first unequal pair,
then proper prefixes. Array disequality is true for different lengths. Precedence
requires an earlier first value at each second value; equal fixed values cannot
occur. Member is existential equality, and empty membership is false.

Integer extrema require nonempty arrays and define their first scalar result.
Legacy `gecode_minimum/maximum_arg_int/bool_offset` forms choose the first tied
position plus offset. New `gecode_arg_min/max_int/bool` forms use their fixed
Boolean tie flag: true chooses first; false accepts any tied extremum and defines
an unsupplied index only for a unique extremum. Offsets for arg-extrema and inverse
must be nonnegative, including empty inverse arrays. Other offsets may be signed.
All indexes and offsets use exact checked i64; nonempty fixed intervals must fit.
Out-of-range candidate indexes yield typed faults without overflowing subtraction.

Sort defines its second array as the sorted multiset of the first. Sort-permutation
also checks that permutation[i]-offset is the zero-based sorted position of input[i].
Equal values allow different valid permutations; only distinct inputs determine
a permutation for completion. Inverse uses forward[i]-offset=j iff
inverse[j]-inverse_offset=i; either complete valid side defines the other. Empty
sort and inverse arrays pass. One-hot channel requires exactly one true entry and
integer=offset+its zero-based position. It derives either side when unique;
an empty Boolean array makes the relation false. Member defines only its full
Boolean reifier and never chooses a witness.

`defines_var` accepts scalar targets, named output arrays, or explicit arrays of
scalar names for sort outputs, sort permutations, either inverse side and the
Boolean channel array. Targets match the complete output-slot multiset and type,
independently of annotation order. Repeated aliases derive only if all calculated
values agree. Input/output cycles await supplied values, supplied outputs are
checked, and derived domain failures feed downstream checks. The existing GCC
count-array contract uses the same target validation.

Ordering findings carry typed operands and conflicting pairs, decisive lex
positions or lengths, extrema/permitted indexes, sort/inverse faults, one-hot true
positions and membership truth. Unavailable irrelevant operands are explicit.
Integer report values are decimal strings; structural positions are JSON numbers.
The [direct control](../../examples/checker-compilation/ordering.fzn) demonstrates
completion even when compilation drops definition metadata.

The absent legacy float/set decreasing includes have no source signatures to
admit directly. Adjacent `float_le(right,left)` or `set_le(right,left)` calls,
combined with Boolean operations/reification, express decreasing mathematics.
Float comparisons use the model's saved tolerance (zero/zero is strict); set
order compares sorted elements, rather than subset inclusion. The native test
uses `{1,2,100} <= {1,9}`, which is true in that order although neither set is a
subset. False implication guards skip the underlying comparisons. Include repair
and complete compiler-profile support belong to checker-030/031.

### Compile-only ordering controls

Use the same pinned sources as the coverage helper:

```sh
mkdir -p target/checker-ordering-controls
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir crates/zincite-check/minizinc -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-ordering-controls/standard.paths --output-fzn-to-file target/checker-ordering-controls/standard.fzn examples/checker-compilation/ordering-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-ordering-controls/gecode.paths --output-fzn-to-file target/checker-ordering-controls/gecode.fzn examples/checker-compilation/ordering-gecode-controls.mzn
```

Both controls format/default-lint and compile with MiniZinc 2.10.1 without a solver
run. In the standard profile, sort becomes three element calls, three permutation
disequalities and two sorted-order inequalities; all-different adds three more
disequalities. Minimum becomes two `int_min` calls and membership becomes Boolean
clauses plus three integer equality reifiers. Input/sorted domains stay `1..3`.
Labels and all four constraint IDs (`sort`, `minimum`, `member`, `distinct`) survive
on the corresponding decompositions, with `[1,2,3]` payloads. This raw artifact
also emits `is_defined_var` in constraint context and scalar `defines_var` targets
without matching variable markers. Preparation diagnoses the malformed metadata;
it does not silently discard it. This is a compiler-profile limitation.

The Gecode control emits `gecode_sort_permutation`, `gecode_bool_int_channel` and
`gecode_arg_min_int`. Input/sorted domains stay `1..3`, permutation domains `-2..0`,
selected `0..2`, and first_min `5..7`. Only `first_min` keeps its scalar
`is_defined_var`/`defines_var` pair. Array definition targets and scalar markers
for sorted/permutation/flags disappear. The duplicate sort call collapses with
its separate permutation ID; the sort, channel and arg_min IDs, labels and entity
payloads survive. That raw artifact prepares and checks supplied arrays, and can
derive `first_min`. For input `[3,1,2]`, sorted `[1,2,3]`, permutation `[0,-2,-1]`,
flags `[false,true,false]` and selected `1`, its candidate is valid and first_min
is 6. Supplied wrong sorted/permutation/flags/index values yield typed failures;
all-false flags establish a violation even with other inputs missing.

Source/load reports agree through `zincite-check` and `zincite check` for those
candidates and for the direct control. The raw standard metadata diagnosis agrees
through both forms. The direct FlatZinc control supplies the explicit array
metadata for native and parser-free Wasm completion. These observations do not
establish include/decomposition closure; checker-030/031 own profile repairs.


## Integer and Boolean tables

The checker admits the six `gecode_table_int/bool` literals with plain, `_reif`
and `_imp` forms, each with the literal empty, `gecode_` and `fzn_` prefixes.
The family has 18 exact names; the checker admits 761 names overall. The prefix applies to the
whole literal: `gecode_gecode_table_int` and `fzn_gecode_table_int` are direct
names; `table_int` and `fzn_table_int` are not.

The first argument is the candidate tuple; the second is fixed row-major data
of the same integer or Boolean type. The tuple length supplies the width. Data
must contain complete rows, even under a false guard. Duplicate rows and repeated
tuple aliases are legal. Empty data allows no tuple. Width zero with empty data
also means zero rows and false membership; width zero with nonempty data is
malformed. Both source preparation and validated loading reject malformed shapes.
Integer entries use the full existing i64 range, without solver integer limits.

Full reification compares membership with its supplied Boolean, including false.
It derives only a marked Boolean result absent from the tuple inputs, after every
tuple entry is known. An empty table does not bypass missing entries. A false
implication guard skips unavailable tuple entries. Tables never choose a row,
missing entry or guard. Supplied outputs are checked, derived domain failures
feed downstream checks, and each check starts with fresh values.

Typed `table` findings carry tuple argument values and variable references,
fixed `table` data (`kind` and `values`), structural `width`, `membership` and
`condition`, alongside constraint metadata. Integer entries use decimal JSON
strings; width uses a JSON number. Text output renders those same fields.
The [direct FlatZinc control](../../examples/checker-compilation/tables.fzn)
checks integers above 2^53, Boolean membership, repeated aliases and reifier
completion. `x=9007199254740993; flag=true;` is valid and complete; adding
`member=false;` reports a typed table conflict and its downstream Boolean failure.
An empty candidate is incomplete.

### Compile-only table controls

Both controls start with `globals.mzn` and `checker_annotations.mzn`. Format and
default-lint them with the pinned standard library and the checker include path.
Use MiniZinc 2.10.1 without starting a solver:

```sh
mkdir -p target/checker-table-controls
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir crates/zincite-check/minizinc -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-table-controls/standard.paths --output-fzn-to-file target/checker-table-controls/standard.fzn examples/checker-compilation/table-gecode-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-table-controls/gecode.paths --output-fzn-to-file target/checker-table-controls/gecode.fzn examples/checker-compilation/table-gecode-controls.mzn
```

The explicit [native control](../../examples/checker-compilation/table-gecode-controls.mzn)
compiles under both profiles into plain integer membership, integer/Boolean
reifiers and integer implication. Both keep named `entries` and `flags` arrays,
integer domains `1..3`, the two scalar definition markers, all four application
IDs, labels and paths. Supplying `entries=[1,2]; flags=[true,false]; guard=true;`
derives both reifiers and checks completely. `entries=[3,3]; flags=[true,true];
guard=true; member=true;` produces three table findings. Omitting entries with
`flags=[false,false]; guard=false;` gives an incomplete report. Both CLI forms
and source/export/load paths check these candidates.

The [standard table control](../../examples/checker-compilation/table-controls.mzn)
also exercises actual standard wrappers. The standard profile aborts in
`std/fzn_table_bool_reif.mzn`: the pinned implementation does not support reified Boolean tables.
The Gecode profile compiles it, folds the integer reifier to true because plain
membership already requires the same rows, and emits `bool_clause`, native
Boolean reification and native integer implication. That output drops membership
application IDs and emits `is_defined_var` on a constraint; preparation diagnoses
that misplaced annotation. These compiler-profile limits belong
to checker-030/031. The runtime keeps its existing marker rules.
