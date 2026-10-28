# zincite-check

A Rust and WebAssembly candidate checker for MiniZinc 2.10.1 text FlatZinc and
official FlatZinc-JSON.
Prepare a model once, then check candidate assignments repeatedly. Checking
performs no I/O and starts no compiler or solver.

The [full-library coverage inventory](../../docs/checker-coverage.md) records the
pinned standard/Gecode signatures, exact aliases, include closures and mathematical
execution evidence. Both profiles' emitted mathematical operations are supported
within the numeric and compiler limits below. External executable/DLL and search
history hooks receive located diagnostics. Checking supplies no search or
optimality proof.

Start with the [application integration guide](../../docs/checker.md) for a small
model, repeated Rust/Wasm checks and structured feedback. This README is the
reference for accepted forms, predicate semantics and compilation recipes. The
[maintainer guide](../../docs/checker-architecture.md) describes preparation,
execution and the existing validation paths.

Install the checker locally with
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
rejects reversed, unordered, overlapping or adjacent ranges. Format version 1
includes float values/settings/operations, set values/domains,
set operations, strict globals, scheduling, bin-packing, rectangle records and
integer relation variants. The original `Linear` executable with its
`equality` Boolean and `ReifiedLessEqual` executable keep their existing shapes. Loading rejects
incompatible versions, bad references, wrong types and invalid definition
annotations before exposing a checkable model. It rebuilds aliases, definition directions and dependencies from validated
operations rather than accepting serialized execution caches. A slot's seed
must come from a fixed or singleton declaration; other alias domains can still
report violations against that value. Defined slots use fixed initializers,
including when an older version-1 artifact omitted the corresponding default.
They do not use logical singleton domains as fallback values.

## Prepare official compiler JSON

`prepare_json(name, source)` accepts MiniZinc 2.10.1 FlatZinc-JSON version 1.0
through the optional `prepare` feature. Both native entry points select it with
`--input-format fzn-json`; text input uses `--input-format fzn` by default.
The flag also applies to `--export`:

```sh
zincite-check --input-format fzn-json --format json examples/checker-compilation/model.fzn.json examples/checker-compilation/candidates/infeasible.dzn
zincite check --input-format fzn-json --export /tmp/zincite-json-prepared.json examples/checker-compilation/model.fzn.json
zincite-check --load /tmp/zincite-json-prepared.json examples/checker-compilation/candidates/feasible.dzn
```

Compiler JSON has `version`, `variables`, `arrays`, `constraints`, `output` and
`solve` fields. It uses JSON numbers for mathematical integers. Preparation reads
their original tokens as exact `i64` values; an integer never passes through
binary64. Numeric literals in float roles become finite binary64 values under
the existing float policy. Prepared artifacts and Wasm assignments continue to
use decimal strings for integers. `--load` accepts only prepared artifacts and
cannot be combined with `--input-format fzn-json`. The parser-free Wasm runtime
loads the artifact exported by either source path.

Typed variables accept domains, `rhs` values or aliases, and introduced/defined
flags. `rhs` is specified in the interface text even though the published schema
omits it. Arrays have no declared element type: references supply their types,
fixed numeric literals take the type required by each use, and empty arrays work
in every array role. Array aliases and forward references work; cycles, unknown
references, incompatible types, fields, versions and semantic operations diagnose
during preparation. String literal objects (`{"string":"text"}`) differ from
identifier strings. An all-literal array listed in `output` becomes assignable;
using its name in a fixed role, such as a coefficient array, then diagnoses.
This is a current preparation boundary; the inspected compiler controls did not
emit that combination. Nested annotation calls use the existing metadata/search
validation, including application payloads, paths, output and definition hints.

Sets and domains use ordered, nonoverlapping, nonreversed range pairs. Integer
sets and disjoint integer domains stay compact across the full `i64` range.
Contiguous integer/float domains use the existing interval records; disjoint
integer/float domains and restricted Boolean domains add `integer_ranges`,
`float_ranges` and `boolean_values` records to prepared format version 1. Empty
integer domains use the existing empty enumeration. Domain membership uses the
same exact integer or tolerant float policy as text input; only exact singleton
domains supply defaults. `computed_domain` excludes these convenience domains
from checking and defaults too. Disjoint JSON domain findings describe ranges
rather than enumerating every integer.

The JSON preparation module validates shape and preserves raw numeric tokens
and source spans, then feeds expressions and types into the existing lowerer.
Both paths share operation admission, metadata checks, definitions and the
executable checker. Locations identify original JSON values or objects, with
byte offsets and line/column positions; text locations identify FlatZinc syntax.
Compare mathematical findings and application fields across formats, allowing
these representation-specific locations and equivalent domain descriptions.

The saved `model.fzn.json`, `standard-controls.fzn.json` and
`gecode-controls.fzn.json` are unedited compiler output. Their text counterparts
check the same valid, invalid, incomplete and supplied-conflict candidates,
including float/set operations, objectives and entity `9007199254740993`.
`json-values.fzn.json` is an authored format control for exact large domains,
`rhs`, aliases and contextual arrays. No compiler or solver runs during checking.

To compile JSON, add `--fzn-format json` to the documented profile recipe and
send both outputs to explicit scratch paths. For example:

```sh
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --fzn-format json --output-fzn-to-file target/checker-model.fzn.json --output-paths-to-file target/checker-model.paths examples/checker-compilation/model.mzn
```

## Compile the application model offline

The package includes `minizinc/checker_annotations.mzn`. It declares
`checker_constraint(string: id, array[int] of int: entities)` for constraints and
`checker_value` with the same arguments for variables. Application developers
choose these IDs and integer entity payloads. The commands in this section
reproduce the original scheduling example with
the installed Gecode backend and the narrow checker include directory. They are
historical example recipes, separate from the pinned
[standard](#standard-library-compilation) and [Gecode](#pinned-gecode-compilation)
profiles used for full-library coverage. The shared-bench example below uses the
same original backend with the packaged `fzn_disjunctive_strict.mzn` override.

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
| Explicit `int_pow_fixed(x,3,cube)` with a marked result | The standard decomposition can emit `defines_var(cube)` without `is_defined_var` on `cube`; the checker derives the marked result |
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
`output_var` onto constraints. Preparation accepts those compiler copies as metadata. Constraint IDs use
`Metadata.application`; copied value IDs use `Metadata.value_application`. Each
role validates repeated annotations independently.

## Standard-library compilation

The packaged [standard profile](minizinc/standard.msc) uses MiniZinc's standard
decompositions with declarations for nine supported primitives: clause
reification, integer and float array extrema, fixed integer power, and float
rounding. Eight small compatibility files supply the missing deprecated
Boolean, float, integer and set decreasing includes through reversed increasing
relations. The annotation declarations live inside this profile too. The
existing Gecode include path does not select these overrides.

Use MiniZinc 2.10.1 build 33348285743 with standard-library commit
`c3448c48e7f05f13805616cfd1b9e94972eef58c` and the source checkout from the
[coverage inventory](../../docs/checker-coverage.md#sources-and-repeat-command):

```sh
mkdir -p target/checker-standard
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc -O0 --keep-paths --no-half-reifications --no-output-ozn -D mzn_ignore_symmetry_breaking_constraints=true --output-fzn-to-file target/checker-standard/model.fzn --output-paths-to-file target/checker-standard/model.paths examples/checker-compilation/standard-controls.mzn
zincite-check --export target/checker-standard/model.json target/checker-standard/model.fzn
zincite-check --format json target/checker-standard/model.fzn examples/checker-compilation/candidates/standard-valid.dzn
zincite-check --load --format json target/checker-standard/model.json examples/checker-compilation/candidates/standard-valid.dzn
```

The `.msc` is an entry point for `--compile`; it supplies no search or solver
protocol. Its `mznlib` path resolves beside the configuration file, including in
the crate package. The native checker never invokes MiniZinc. The complete Gecode
profile below selects upstream globals and a separate compatibility include.

The [saved control](../../examples/checker-compilation/standard-controls.fzn)
has ten scalar slots and eight constraints. The compiler turns `y = x + 2`
into a defining linear relation carrying both `offset/[2]` and copied `y/[1]`
application metadata. It substitutes that expression into the capacity check,
eliminates `rounded`, `clause`, `order` and the source optional value name,
and emits a set-cardinality output without a definition hint. Callers supply
`X_INTRODUCED_18_ = card(chosen)` and the optional representation
`X_INTRODUCED_24_` (zero for absence, 1..3 for presence). The checker derives
`y` and both optional Boolean reifiers. Those introduced names belong to this
saved control; inspect each new compilation before supplying them.

| Saved candidate | Direct calculation | Report |
| --- | --- | --- |
| `standard-valid.dzn` | `x=0`, `y=2`, set size 1, rounded amount 1, true clause, absent optional value | Valid, complete |
| `standard-invalid.dzn` | `x=-3` derives `y=-1`; its domain fails, and capacity, rounding, clause and optional checks also fail | Invalid, complete; five findings |
| `standard-conflict.dzn` | Caller supplies `y=0` while `x+2=2`; supplied `y` also stays the objective value | Invalid, complete; one finding with both application IDs |
| `standard-partial.dzn` | No supplied values | Incomplete; ten unresolved scalar slots |

Compile-only controls also cover counting, ordering, circuits, sets and channels,
float functions, elements, packing, scheduling and automata. All four 2D element
types lower to supported 1D operations; regular expressions lower to DFA checks.
The [profile accounting](../../docs/checker-coverage.md#standard-profile-accounting)
checks every selected include, optional entry point, deprecated companion and
external predicate signature. Its optional `--checker` argument validates all
128 direct standard declaration signatures through native preparation.

The pinned standard library unconditionally aborts 32 bodies across 27 predicate
names, including Boolean table reification. Integer table reification accepts
only tuple widths 1..5. In fresh width-2..5 controls it also emits two
`defines_var` hints for the same integer auxiliary, one on `array_int_element`
and one on `bool2int`; the checker diagnoses these malformed hints. Explicitly
marking the source reifier produces the same duplicate. Width 1 exports; width 6
fails the compiler assertion. Gecode's native table reifiers export successfully.
These are compiler/metadata limits and provide no runtime support credit. Experimental external-execution and search-history hooks are
outside pure candidate checking. None of this establishes arbitrary MiniZinc
source equivalence. `-O0` still tightens domains and eliminates names and labels;
array metadata can disappear during flattening. Use the symmetry ignore flag
when an application permits every representative, and inspect the compiled
model when it needs symmetry-breaking constraints instead.

## Pinned Gecode compilation

The complete Gecode profile selects MiniZinc 2.10.1 commit
`c3448c48e7f05f13805616cfd1b9e94972eef58c` and Gecode main commit
`186551447db0f74639d65cb8de5af190f15ac2c2`. Use the clean source trees from the
[coverage inventory](../../docs/checker-coverage.md#sources-and-repeat-command).
The installed compiler is MiniZinc 2.10.1 build 33348285743; `--globals-dir`
selects the pinned upstream library, including `gecode.mzn`, `gecode_fzn.mzn`
and its versioned redefinitions. `minizinc/gecode` adds application annotations
and six missing decreasing compatibility relations. It contains no primitive
overrides. Keep that include path separate from `minizinc/standard` and the
original narrow checker library.

Reproduce the [saved application control](../../examples/checker-compilation/gecode-controls.fzn)
from the repository root:

```sh
mkdir -p target/checker031-validation
minizinc --compile --solver org.gecode.gecode --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc/gecode -O0 --keep-paths --no-half-reifications --no-output-ozn -D mzn_ignore_symmetry_breaking_constraints=true --output-fzn-to-file target/checker031-validation/gecode-controls.fzn --output-paths-to-file target/checker031-validation/gecode-controls.paths examples/checker-compilation/gecode-controls.mzn
zincite-check --export target/checker031-validation/model.json target/checker031-validation/gecode-controls.fzn
zincite-check --format json target/checker031-validation/gecode-controls.fzn examples/checker-compilation/candidates/gecode-valid.dzn
zincite-check --load --format json target/checker031-validation/model.json examples/checker-compilation/candidates/gecode-valid.dzn
```

Compilation is offline and compile-only. The selected native library emits
`gecode_gcd_int` and `gecode_float_pow`, alongside integer, Boolean and set
primitives. The checker derives `tool_capacity=budget_offset+2`, `combined=chosen union {1}`,
`gcd_result=gcd(budget_offset,6)`, `squared=amount^2` and `allowed=(budget_offset<=1)`. The cardinality
operation has no definition hint: the saved candidates supply
`X_INTRODUCED_22_=card(combined)`. This name belongs to the archived output.

The compiler tightens `budget_offset` to `-2..3` and `squared` to `0.0..1.0`, substitutes
`budget_offset+2` for `tool_capacity` in the capacity check, and removes that check's application ID
while keeping its label. Domain feedback uses the value metadata. The defining
linear relation carries both constraint ID `offset` and copied value ID `tool_capacity`,
with exact entity `9007199254740993`. Gcd, power, Boolean and set relations keep
their labels or paths where emitted. The compiler can still remove names,
constraints and metadata at `-O0`; checking does not reconstruct them.

| Saved candidate | Direct calculation | Report |
| --- | --- | --- |
| `gecode-valid.dzn` | `budget_offset=0` gives `tool_capacity=2`, gcd 6, union `{1,2}`, size 2 and square 1 | Valid, complete |
| `gecode-invalid.dzn` | `budget_offset=-3` gives `tool_capacity=-1`, gcd 3, union `{1,2,3}`, size 3 and square 4 | Invalid, complete; domains for budget_offset, tool_capacity and square fail, plus capacity |
| `gecode-conflict.dzn` | Supplied `tool_capacity=0` conflicts with calculated 2 and stays the objective value | Invalid, complete; one finding with both IDs |
| `gecode-partial.dzn` | Empty file supplies no values | Incomplete; eleven unresolved scalar slots |

Supplying only `budget_offset=-3` gives two known domain findings with incomplete checks.
Every check starts afresh, so the valid edit passes again after these states.
The [native/Wasm smoke](../../examples/checker-wasm/README.md) compares the same
five edits and the existing exact integer/set cases. No solver runs.

The symmetry ignore flag permits every representative. With it false, `budget_offset=2`
fails the compiled symmetry domain; with it true, the otherwise valid candidate
passes. `mzn_ignore_redundant_constraints=true` can also remove implied source
constraints. These flags act during compilation, before the checker sees a model.
Recognized search and propagation annotations are metadata, including Gecode's
search choices; they select no candidate values.

The [Gecode accounting](../../docs/checker-coverage.md#gecode-profile-accounting)
checks all 754 selected files, optional and deprecated includes, and every
external signature. Native preparation validates 316 direct Gecode signatures
and all registered mathematical aliases, including feature-guarded set, float
and MPFR names. Exact aliases use corresponding declarations or explicit
positional contracts; a shared handler does not merge fixed and variable array
signatures. Both bound roles work for `bool_lin_le`, `gecode_bool_lin_le` and
`fzn_bool_lin_le`.

The 27 existing compile controls and this application control prepare with the
pinned Gecode library. They include explicit-end schedules, optional rectangles,
set channels, floats and number theory. Separate checks compile deprecated
decreasing includes and native Boolean/integer table reifiers. The selected
standard fallbacks still contain 31 unconditional abort bodies across 26 names;
these source forms emit no FlatZinc and get no runtime-support credit. The
standard profile's integer-table width limit does not apply to Gecode's native
table override.

External executable/DLL hooks and restart status/history hooks produce specific
located diagnostics at declarations, calls and annotations. The seven malformed
upstream restart prototypes listed in the [hook inventory](../../docs/checker-coverage.md#external-hooks)
lack semicolons and are source compiler limits; valid declarations of those
hooks still reject unavailable history. The checker performs no external
execution, search or random value selection.

The authored model, candidates and compatibility files pass formatting and local
naming/label checks. Default linting of the complete include closure reports
the upstream naming warning for Gecode's parameter `S` and 44 explicit analysis
limitations. The pinned sources are unchanged. These diagnostics do not establish
complete semantic lint analysis.

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
  Operand `mzn_path` annotations, including set literals and range bounds, are
  validated and collected into their owning constraint or variable paths.
  Copied `checker_value` on a constraint uses the optional `value_application`
  field, independently of its `checker_constraint` application. The field is
  omitted when absent, so older prepared artifacts load with the same shape.
  Expression names and documentation strings are accepted as metadata.
- `defines_var` identifies an assignable scalar or supported complete output
  array in one unique defining relation. It does not require a declaration
  `is_defined_var` marker. A standalone declaration marker and a bare marker
  copied onto a constraint are informational; neither creates a definition.
  Supported completion directions appear below; other well-formed directions
  can be checked with supplied values.
- Variable output markers `output_var` and `output_array` are metadata.
  A bare `output_var` copied onto a constraint is also accepted as metadata.
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
  value greater than one (a literal or fixed float parameter) and a positive
  integer scale. Float search precision is a finite fixed float metadata argument,
  separate from checker tolerance.
- Propagation metadata on constraints includes `domain`, `bounds`, `boundsZ`,
  `boundsR`, `boundsD`, `priority(k)` with a fixed integer argument,
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
to a logical singleton domain when inputs are missing. A fixed initializer
still supplies its exact value and can make a definition cycle checkable.
Supplied values take precedence and are checked against fixed initializers too.

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

Two compile-only controls use the pinned Gecode library with the MiniZinc
standard library:

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
float-to-integer conversion still checks its i64 range. The
[profile inventory](../../docs/checker-coverage.md) separately checks the compiler
include/decomposition closures.

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
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --output-paths-to-file target/checker-float-controls/functions-standard.paths --output-fzn-to-file target/checker-float-controls/functions-standard.fzn examples/checker-compilation/float-function-controls.mzn
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
preparation accepts that definition hint. To reproduce, replace the maximum-expression
constraint with that call and declare `var float: maximum_value;`, using the same
standard command. These controls establish their observed behavior. The
[profile inventory](../../docs/checker-coverage.md) records include/decomposition
closure; compilation can still change editing metadata as described above.

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
| `fzn_disjunctive_strict(start,duration)` | Every pair satisfies `start[i]+duration[i] <= start[j]` or the reverse | None; durations must be nonnegative integers |
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
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --output-paths-to-file target/checker017-standard.paths --output-fzn-to-file target/checker017-standard.fzn examples/checker-compilation/set-primitives.mzn
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
accept those exact prefixes, for 165 Boolean names.
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
division. Set cardinality, scheduling/rectangle endpoints, resource sums, bin
label intervals and bin loads also use checked i64 results. Overflow, including intermediate overflow and `i64::MIN / -1`, is a
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

The [measurement report](../../scripts/checker-performance.md) records the named
`m1-max-release-2026-10-28-full-acceptance-v2` run on the user's M1 Max. Its 60
individual edits of the two-job schedule gave native checking p95 of 0.008834 ms
and browser assignment-to-parsed-report p95 of 0.10 ms. Those measurements are
below the 100 ms target for that recorded build and workload. The report separates preparation, loading, checking and
Wasm/JSON costs, with size and memory measurements for 10 and 100 copies inside
one model. Timing excludes DOM rendering. It includes build identities, the
saved-run driver and repeat commands.


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
count array's canonical slots. Declaration `is_defined_var` markers are optional.
The artifact keeps
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
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --output-paths-to-file target/checker-counting-controls/standard.paths --output-fzn-to-file target/checker-counting-controls/standard.fzn examples/checker-compilation/counting-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file target/checker-counting-controls/gecode.paths --output-fzn-to-file target/checker-counting-controls/gecode.fzn examples/checker-compilation/counting-gecode-controls.mzn
```

Both source controls were formatted and default-linted with the pinned standard
library and the checker annotation include. Both also compiled under the other
profile. The standard decomposition can copy `is_defined_var` onto a `bool_clause`
constraint; the checker accepts it as an informational hint. The Gecode profile emits
`fzn_count_eq`, `fzn_nvalue`, `fzn_among` and native GCC, but drops definition
metadata for the distinct-value and GCC results. It also replaces the source
`counts` array name with an introduced name. Supplying those outputs gives a
valid complete report. The explicit native control emits named GCC counts and
both membership window types with labels, paths and application payloads, but
MiniZinc drops its GCC `defines_var` and scalar `is_defined_var` markers in both
profiles. Supplying only entries and flags therefore gives an incomplete report;
supplying the count outputs checks completely. These compilation limits affect
which values an application must supply.
The checker validates definition targets and duplicate relations.

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
subset. False implication guards skip the underlying comparisons. The packaged
profiles supply compatibility includes; their
[include accounting](../../docs/checker-coverage.md) is separate from these
direct relation checks.

### Compile-only ordering controls

Use the same pinned sources as the coverage helper:

```sh
mkdir -p target/checker-ordering-controls
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --output-paths-to-file target/checker-ordering-controls/standard.paths --output-fzn-to-file target/checker-ordering-controls/standard.fzn examples/checker-compilation/ordering-controls.mzn
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
without matching variable markers. Preparation accepts these optional hints and
validates each actual definition target and direction.

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
metadata for native and parser-free Wasm completion. The
[profile inventory](../../docs/checker-coverage.md) records the wider
include/decomposition closure separately.


## Integer and Boolean tables

The checker admits the six `gecode_table_int/bool` literals with plain, `_reif`
and `_imp` forms, each with the literal empty, `gecode_` and `fzn_` prefixes.
The family has 18 exact names. The prefix applies to the
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
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --output-paths-to-file target/checker-table-controls/standard.paths --output-fzn-to-file target/checker-table-controls/standard.fzn examples/checker-compilation/table-gecode-controls.mzn
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
application IDs and emits `is_defined_var` on a constraint. The checker accepts
that copied hint as metadata. Boolean table reification is a standard source
compiler limit. The [Gecode profile accounting](../../docs/checker-coverage.md#gecode-profile-accounting)
records its native table route.


## Deterministic and nondeterministic automata

The checker admits `gecode_regular`, `gecode_regular_set` and
`gecode_regular_nfa_int_fzn`, each with the literal empty, `gecode_` and `fzn_`
prefixes. The family has nine exact names. The prefix applies to
the whole literal, including `gecode_gecode_regular`; standard wrapper names and
reified forms are not direct checker operations.

All three forms take an integer word, positive state count Q, fixed transition
data, initial state and accepting-state set. States are numbered 1..Q.
`gecode_regular(word,Q,S,d,q0,F)` uses alphabet 1..S.
`gecode_regular_set(word,Q,symbols_min,symbols_max,d,q0,F)` uses inclusive alphabet
endpoints. Its fourth argument is not a count.
`gecode_regular_nfa_int_fzn(word,Q,first_symbol,symbol_count,d,q0,F)` uses a first
symbol and positive count, within the pinned NFA alphabet limits
-2147483646..2147483646. The DFA forms use the existing checked i64 boundary.

Transition data contains exactly Q times the alphabet width entries, in state
then symbol order. A DFA cell contains a target in 0..Q; zero ends the path.
An NFA cell contains a compact set of target states in 1..Q. Each transition
consumes one symbol, with no epsilon transitions. Nondeterminism follows all
successors and accepts when any final reachable state belongs to F. Empty
accepting and successor sets are legal. The empty word accepts exactly when
q0 belongs to F. Preparation and loading validate dimensions, checked alphabet
and index arithmetic, states and targets before any candidate check.

Every word operand must be known before checking, even for an empty language.
Missing operands produce blocked checks. Automata never supply symbols or
witness states, including through `defines_var`. Each check starts from q0.
Typed `automaton` findings carry word arguments and variable references, initial,
accepting and reached states, alphabet endpoints, and a rejection reason:
`out_of_alphabet`, `dead_path` or `nonaccepting_end`. The first two include a
one-based position and the offending symbol. Mathematical integers use decimal
JSON strings; positions use numbers. CLI text renders these fields alongside
labels, locations and application metadata.

The [direct FlatZinc control](../../examples/checker-compilation/automata.fzn)
checks an exact symbol above 2^53, a DFA dead transition and an NFA branch.
`x=9007199254740993; y=1; a=-1; b=0;` is valid and complete.
`x=9007199254740992; y=2; a=-1; b=-1;` reports all three rejection kinds.
`y=2;` reports a known dead path with incomplete word checks.

### Compile-only automaton controls

The [public control](../../examples/checker-compilation/automata-controls.mzn)
and [native control](../../examples/checker-compilation/automata-gecode-controls.mzn)
exercise wrapper decompositions and the three native encodings.

Use the compilation command from the [standard](#standard-library-compilation)
or [pinned Gecode](#pinned-gecode-compilation) recipe with these model paths and
fresh output paths. The observations below compare pinned Gecode with the
original narrow checker backend, which selected installed Gecode with the custom
`crates/zincite-check/minizinc` globals directory. That historical backend uses
standard decompositions for these wrappers and is separate from the packaged
`standard.msc` profile.

The native control emits the same three names under both compilation routes,
without auxiliary definitions. Both keep `word` in 1..2, `shifted_word` and `nfa_word` in
-1..0, named arrays, all three application IDs, labels and paths.
`word=[1,2]; shifted_word=[-1,0]; nfa_word=[-1,0];` checks valid and complete;
`word=[2,1]; shifted_word=[0,-1]; nfa_word=[0,-1];` reports three violations.
Supplying only `word=[2,1];` reports a violation with incomplete checks. Both CLI
forms agree through source and export/load paths.

The [standard-wrapper control](../../examples/checker-compilation/automata-controls.mzn)
compiles under both compilation routes and preserves the three IDs, labels and
word domains.
The Gecode profile emits native DFA and NFA operations; its set-alphabet NFA
wrapper lowers -1..0 to first=-1, count=2 using the same NFA primitive. Shifted
DFA membership decomposes into two `gecode_int_element2d` calls, one defining its
intermediate state. The three candidates above check completely for full words,
including four violations for the invalid words, and incompletely for partial
words. The original backend decomposes DFA/NFA calls into element, set membership
and linear relations. Five linear outputs have definitions, but intermediate
state and target-set witnesses have no definition markers. Supplied primary
words therefore produce incomplete reports without known violations. The checker
does not infer these witnesses. Applications checking this decomposition must
supply them; relation support alone cannot complete unmarked auxiliaries.

## Circuits and costs

The checker admits `gecode_circuit`, `gecode_circuit_cost` and
`gecode_circuit_cost_array`, each with the literal `gecode_` and `fzn_` registry
prefixes, for nine exact names. The prefix applies
to the complete name: `gecode_gecode_circuit` and `fzn_gecode_circuit` are valid;
`fzn_circuit`, `circuit_cost` and reified variants use other compiler routes.
Declarations check argument roles independently of formal parameter names.

| Native signature | Relation and marked outputs |
| --- | --- |
| `gecode_circuit(int: offset, array[int] of var int: x)` | `x` is an in-range permutation with one cycle; no output direction |
| `gecode_circuit_cost(array[int] of int: c, array[int] of var int: x, var int: z)` | Zero-based circuit; `z=sum(c[i*n+x[i]])`; marked `z` derives from all selected costs |
| `gecode_circuit_cost_array(array[int] of int: c, array[int] of var int: x, array[int] of var int: y, var int: z)` | Zero-based circuit; each `y[i]=c[i*n+x[i]]`; `z=sum(y)` using actual supplied or derived `y` |

Here `i` is a zero-based structural row and `c` contains `n*n` row-major costs.
The array form requires exactly `n` edge costs. Admission rejects empty successor
arrays, negative plain offsets, overflowing fixed successor intervals and malformed
cost shapes. A singleton self-cycle passes. Candidate successors outside the
interval report a fault before index conversion. Repeated successor aliases are
legal candidates and can fail the permutation check. Checked i64 arithmetic
preserves wide offsets and negative costs; source-order sum overflow is a located
execution error. Gecode's solver integer representation does not bound these values.

Known in-range successors determine corresponding marked edge costs even if
other edges are missing or the topology fails. A repeated output slot needs all
its occurrences to select the same known cost. Complete `y` arrays and combined
`[y...,z]` slot multisets support the existing array `defines_var` syntax,
independently of annotation order. Marked array-form totals wait for actual `y`;
they do not invent unmarked edge costs. A supplied edge cost stays available to
the sum even when it conflicts with the matrix. Targets also used as successors,
and targets used as both an edge cost and its total, have no completion direction.
The checker never chooses successors or solves sums backwards.

`ViolationKind::Circuit` carries optional typed successor/cost arguments,
references for unresolved arguments, the offset, and all available faults:
out-of-range or repeated successors, positions in proper subcycles, selected
edge-cost conflicts and total conflicts. Fault positions are one-based JSON
numbers; mathematical integers are decimal strings. Known faults can report
while other inputs are missing. Unresolved values and blocked checks describe
completeness independently. Labels, application IDs/entities and locations use
the same references as other families.

The [direct control](../../examples/checker-compilation/circuits.fzn) includes a
wide offset and costs above 2^53, combined reordered output targets, a scalar
cost definition and a downstream bound. With
`a=1;b=0;p=9007199254740994;q=9007199254740993;`, it derives
`costs=[9007199254740993,-4]` and both totals `9007199254740989`, then passes.
`a=0;` establishes proper subcycles while other inputs and checks are unresolved.
The [Wasm smoke](../../examples/checker-wasm/README.md) checks complete conflicts,
incomplete reports and fresh completion against this same exported control.

### Compile-only circuit controls

The [public control](../../examples/checker-compilation/circuit-controls.mzn)
and [native control](../../examples/checker-compilation/circuit-gecode-controls.mzn)
exercise circuit wrappers and native cost relations.

Use the compilation command from the [standard](#standard-library-compilation)
or [pinned Gecode](#pinned-gecode-compilation) recipe with these model paths and
fresh output paths. The observations below compare pinned Gecode with the
original narrow checker backend, which selected installed Gecode with the custom
`crates/zincite-check/minizinc` globals directory. That historical backend uses
standard decompositions for these wrappers and is separate from the packaged
`standard.msc` profile.

The native control emits the three native names under both compilation routes.
Domains stay `5..7` for the plain successors, `0..1` for cost successors, `-4..7` for
edge costs and `-4..14` for totals. Named array aliases, all three constraint
labels/IDs, value IDs/entities and paths survive. The compiler drops both
`is_defined_var` and `defines_var` for the edge-cost array, while the scalar
`scalar_total` keeps its markers. Thus
`positive_successors=[6,7,5];successors=[1,0];costs=[7,-4];array_total=3;`
checks valid and derives `scalar_total=3`; omitting `costs` and `array_total`
leaves an incomplete report. Supplied `[8,9]` with `array_total=16` reports two
edge conflicts and the actual total 17, independently of circuit topology.

The public control uses index sets `5..7` and `-2..0`. Gecode emits
`gecode_circuit(5,...)` for the positive array and `gecode_circuit(0,...)` for
the shifted negative array, with marked `int_lin_eq` definitions for the shift.
Source domains stay `5..7` and `-2..0`; shifted domains are `0..2`. The array
aliases are `X_INTRODUCED_16_` and `X_INTRODUCED_21_` in this compilation.
Their assignments `[6,7,5]` and `[-1,0,-2]` pass after completion. Self-loop
assignments `[5,6,7]` and `[-2,-1,0]` fail both circuits. Both IDs, labels,
value metadata and paths survive. Generated names belong to that compilation.

The original backend's public decomposition emits integer linear relations,
element calls, Boolean clauses and reifiers, with tighter successor domains excluding self-loops.
It also emits `defines_var` on an element result without `is_defined_var` on
that declaration; preparation accepts the definition hint. The standard-profile
controls prepare these operations directly. Tighter domains and rewritten array
names still affect application editing. The
[Gecode profile accounting](../../docs/checker-coverage.md#gecode-profile-accounting)
covers its include/decomposition closure.


## Legacy scheduling

The checker accepts these exact positional contracts. The three native names
beginning `gecode_schedule_` and `cumulatives` also accept their literal
`gecode_` and `fzn_` registry prefixes. For example,
`gecode_gecode_schedule_unary` and `fzn_cumulatives` work;
`schedule_unary`, `fzn_unary` and reified variants do not.

| Operation | Argument roles and relation |
| --- | --- |
| `fzn_disjunctive_strict(s,d)` | Variable integer starts and durations; matching bodyless declaration required |
| `gecode_schedule_unary(s,d)` | Variable integer starts; fixed nonnegative durations |
| `gecode_schedule_unary_optional(s,d,p)` | Same fixed durations; variable Boolean presence |
| `gecode_schedule_cumulative_optional(s,d,u,p,b)` | Fixed nonnegative durations/usages; variable Boolean presence and integer capacity |
| `cumulatives(s,d,u,b)` | Variable integers; concrete upper-bound relation below |
| `cumulatives(s,d,u,m,b,upper)` | Variable starts/durations/usages/machines; fixed integer capacity array and Boolean upper/lower flag |

Task arrays must have equal lengths. Declarations validate fixed/variable roles,
shapes and literal domain restrictions regardless of formal names. The four-, six-
and native seven-argument overloads can be declared in one model. Scheduling selects
no output: starts, durations, presence and machines come from assignments,
fixed declarations or other supported definitions. Missing inputs produce
ordinary incomplete reports. Absent optional tasks do not block their constraint
on a missing start, although that variable's own domain check is still incomplete.

Strict and native unary checks require nonnegative durations and, for each pair
of present tasks, `s[i]+d[i] <= s[j]` or its reverse. A zero-duration task strictly
inside another task conflicts; endpoint ties and simultaneous zero tasks pass.
Optional unary durations must be valid constants even when the task is absent.

Optional cumulative requires nonnegative capacity even for empty or entirely
absent tasks. Fixed durations and usages must be nonnegative for every task,
including absent tasks. Positive-duration present tasks consume their usage on
`[s[i],s[i]+d[i])`. Each present zero-duration task separately adds its demand to
tasks strictly spanning its start; tasks starting/ending there and other zero
tasks at that time add nothing to that zero task's check. Thus `[3,0]` durations
with starts `[0,1]`, usages `[2,1]` and capacity 2 fail when both tasks are present;
starts `[0,0]` and `[0,3]` pass.

Six-argument cumulatives uses zero-based machine indexes into the capacity
array. The checker reports negative durations and out-of-range machines as
separate caller-obligation faults; the native resource relation itself does not
supply those obligations. Usages and capacities may be signed. For each machine,
check `sum(usage) <= capacity` when `upper=true`, or `>= capacity` otherwise,
only at times covered by at least one positive-duration task. Zero-duration
tasks and idle intervals impose no resource bound. The legacy interface derives
ends by checked addition of the supplied starts and durations.

### Concrete four-argument relation

This interface deliberately checks candidate values independently of declared
start domains, fixedness and posting order. It does not emulate Gecode's
branch selection or propagation. The same rule governs all three aliases.

Empty task arrays pass. A singleton checks only `d[1] > 0 -> u[1] <= b`; it
needs no start value. With two or more tasks, use the concrete integer horizon
`min(s)..max(s+d)`, excluding its upper endpoint. At each time in that horizon,
require `sum(u[i] where s[i] <= time < s[i]+d[i]) <= b`. Idle gaps count with
usage zero. An empty horizon passes. Usages, capacity and durations may be
signed; a nonpositive duration contributes no active interval. No additional
nonnegativity condition is imposed by this concrete relation.

For example, starts `[0,0]`, durations `[0,0]`, usages `[1,1]` and capacity `-1`
pass regardless of whether the declared start ranges are `0..0` or `0..1`.
Starts `[0,2]`, durations `[1,1]`, usages `[-1,-1]` and capacity `-1` fail at the
idle time 1. The corresponding six-argument single-machine call passes because
its resource relation checks active intervals only. Pinned Gecode's
[handler](https://github.com/Gecode/gecode/blob/186551447db0f74639d65cb8de5af190f15ac2c2/gecode/flatzinc/registry.cpp#L1444)
can select different constraints from declaration domains and earlier posting;
its solver behavior is therefore not the compatibility contract for this call.

All scheduling checks visit task event boundaries, never each time unit.
Endpoint and resource-sum arithmetic is checked at the i64 boundary, including
intermediate sums in task-array order. Failures are located execution errors.
Known negative optional capacities, negative strict/multi-machine durations and
invalid machines report even when unrelated values are missing.

Fixed-duration strict calls keep their existing `DisjunctiveStrict` executable
and `disjunctive_strict` report shape. Variable-duration strict and the newly
supported native forms use an additive `Scheduling` executable and
`scheduling` finding. Its typed `faults` distinguish duration, machine, capacity,
conflicting pairs, singleton demand and resource bounds; task values and capacity
arguments carry variable references when available. Missing task values are
`null`. Resource faults identify time, machine when relevant, upper/lower bound,
summed usage and one-based contributing positions. All mathematical integers
are decimal strings. Application metadata and completeness use the existing
report contracts.

The [direct control](../../examples/checker-compilation/legacy-scheduling.fzn)
combines a duration derived by integer addition with strict, optional and signed
resource checks over integers above JavaScript's exact-number range. The
[native/Wasm smoke](../../examples/checker-wasm/README.md) exercises feasible,
conflicting, omitted and incomplete-invalid edits through the same artifact.
The explicit-end contracts follow below. The standard seven-argument
`fzn_cumulatives` is a compiler wrapper with different argument roles, as
described in that section.

### Compile-only legacy scheduling controls

The [public control](../../examples/checker-compilation/legacy-scheduling-controls.mzn)
and [native control](../../examples/checker-compilation/legacy-scheduling-gecode-controls.mzn)
exercise strict scheduling and both native `gecode_cumulatives` overloads.

Use the compilation command from the [standard](#standard-library-compilation)
or [pinned Gecode](#pinned-gecode-compilation) recipe with these model paths and
fresh output paths. The observations below compare pinned Gecode with the
original narrow checker backend, which selected installed Gecode with the custom
`crates/zincite-check/minizinc` globals directory. That historical backend uses
standard decompositions for these wrappers and is separate from the packaged
`standard.msc` profile.

The native control emits the same optional unary/cumulative operations and both
`gecode_cumulatives` overloads under both compilation routes. Starts keep
`0..8` domains; presence stays Boolean. Array aliases, value IDs, all four constraint IDs,
labels and paths survive. Starts `[0,3]` with presence true pass; `[0,1]` with
presence true report both optional conflicts; the same starts with presence
false pass. Omitting values gives an incomplete report, and an out-of-domain
start still reports alongside blocked checks.

The original backend's public strict control emits one variable-duration
`fzn_disjunctive_strict` with the named start/duration arrays and metadata.
The pinned Gecode override decomposes that variable-duration call into two
integer-linear implications and a Boolean clause. It marks both implication
guards as defined, but implication gives no unique guard value. Primary-only
assignments therefore leave an incomplete report. Supplying those explicit
witnesses permits a complete check; the runtime does not invent them.


## Explicit-end scheduling

The five native flex operations accept these exact argument roles. Each also
accepts its literal `gecode_` and `fzn_` registry prefixes: for example,
`gecode_gecode_unary_flex` and `fzn_gecode_cumulative_flex_optional`.
These five operations have fifteen exact names. Bare `unary_flex` and
reified/implication spellings are unsupported.

| Operation | Argument roles |
| --- | --- |
| `gecode_unary_flex(s,d,e)` | Variable integer starts, durations and ends |
| `gecode_unary_flex_optional(s,d,e,p)` | Same integer arrays; variable Boolean presence |
| `gecode_cumulative_flex(s,d,e,u,b)` | Same integer arrays; fixed nonnegative usage array; variable integer capacity |
| `gecode_cumulative_flex_optional(s,d,e,u,b,p)` | Same roles; final variable Boolean presence array |
| `gecode_cumulatives_flex(s,d,e,u,m,b,upper)` | Variable integer starts/durations/ends/usages/machines; fixed integer capacities and Boolean upper/lower flag |
| `cumulatives(s,d,e,u,m,b,upper)` | Same native seven-argument contract, also under `gecode_` and `fzn_` prefixes |

All task arrays have equal lengths. Matching declarations validate types and
fixed/variable roles independently of formal names. The standard-library
seven-argument `fzn_cumulatives(s,d,u,m,b,upper,min_machine)` has different roles
and is not a direct native call. Its wrapper follows compiler decomposition;
the [profile inventory](../../docs/checker-coverage.md) accounts for that route.

The checker uses the supplied starts and ends for every flex resource check,
independently of duration values and Gecode's posting-time rewrites. It does not
add `s[i]+d[i]=e[i]`. The caller supplies separate constraints for that equation,
conditional on presence when appropriate. A marked supported arithmetic equation
can derive an end through the existing worklist. Scheduling itself derives no
start, duration, end, presence, usage or machine value.

Unary and cumulative require nonnegative durations for every task, including
absent tasks. Cumulative also requires fixed nonnegative usages for every task
and nonnegative capacity, including empty or entirely absent arrays. Those
requirements can report with unrelated values missing. Absent tasks consume no
resource and need no start/end value for their resource check, but their duration
must be known and nonnegative. Missing values produce ordinary incomplete reports.

For each present unary pair, require `e[i] <= s[j] || e[j] <= s[i]`.
Thus an empty interval strictly inside another task conflicts; ties at either
endpoint and simultaneous zero-length points pass. Cumulative checks the total
usage on half-open intervals `[s[i],e[i])`. It also checks each present task's own
usage plus usages of tasks strictly spanning its start: `s[j] < s[i] < e[j]`.
Tasks starting or ending at that point add nothing to this individual check.
Coincident zero-length tasks are checked individually. For starts `[0,1]`, ends
`[3,1]`, usages `[2,1]` and capacity 2, the point task fails with usage 3;
starts/ends `[0,0]/[3,0]` or `[0,3]/[3,3]` pass.

Multi-machine flex uses zero-based capacity indexes, signed usages/capacities,
and upper (`sum <= bound`) or lower (`sum >= bound`) limits only while at least
one task is active on that machine. Zero-length tasks contribute nothing. It
imposes no duration-sign or machine-domain guard: out-of-range machines create
no resource events. A missing duration does not block the resource check,
and a known empty interval needs no usage value.
Separate caller constraints/domains check those obligations. Legacy six-argument
calls keep their documented caller-obligation faults.

These formulas also define inconsistent tuples. For example, unary starts
`[0,2]`, durations `[1,1]` and ends `[4,4]` conflict; changing the durations to
`[4,4]` and ends to `[1,3]` passes the resource check. Separate end equations fail
in each example. Reversed ends make half-open resource intervals empty;
cumulative still checks each task's own usage at its start. Unary uses its
pairwise formula even for reversed ends: a reversed task can conflict with a
second interval that spans both endpoints. This is a deliberate concrete
convention. Pinned Gecode can discard supplied ends when it rewrites assigned
durations to a fixed-duration operation, and its propagators can treat
inconsistent tuples differently. The checker does not reproduce those
posting-dependent effects or claim propagation equivalence outside coherent
caller-supplied tuples.

Checks visit task event boundaries and use checked i64 resource sums. Flex
resource checks perform no endpoint addition; arithmetic occurs only in a
separate end equation when one exists. Typed `scheduling` findings include an
`end` argument with its variable reference when available, alongside starts,
durations, presence, usages, machines, capacities and faults. Legacy task reports
omit `end`, and legacy executable records omit `ends`, so their shapes stay
compatible. All mathematical integers use exact decimal strings.

The [direct control](../../examples/checker-compilation/explicit-end-scheduling.fzn)
combines a marked end equation with all five operations over values above
`2^53`. The native/Wasm smoke compares boundary, interior, contradictory supplied
end, incomplete-invalid and omitted candidates through the same prepared model.

### Compile-only explicit-end control

Compile the [small native control](../../examples/checker-compilation/explicit-end-scheduling-gecode-controls.mzn)
through the pinned Gecode library without solving:

```sh
checker_run=$(mktemp -d "$PWD/target/checker-flex.XXXXXX")
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file "$checker_run/gecode.paths" --output-fzn-to-file "$checker_run/gecode.fzn" examples/checker-compilation/explicit-end-scheduling-gecode-controls.mzn
```

The output contains all five flex calls, their application IDs and labels,
and a separate `int_lin_eq([1,-1,-1],[ends[1],starts[1],task_duration],0)` with
`defines_var(ends[1])` and `is_defined_var` on its slot. The compiler aliases
`ends[2]` to `starts[2]`; it does not emit a second equality. Starts keep domains
`0..8`, task duration `0..3`, and the first end `0..11`. No resource operation owns an
end equation.

For `starts=[0,3];task_duration=3;second_present=true;`, completion gives `ends=[3,3]`.
Both unary pairs separate at time 3. Each cumulative demand fits capacity 2,
and the multi-machine load is 2 on `[0,3)`, so the candidate is valid and complete.
For `starts=[0,1];task_duration=3;second_present=true;`, completion gives `ends=[3,1]`.
Both unary checks conflict, and both cumulative checks see the point task's
usage 1 plus the spanning task's usage 2 at time 1. Those four findings coexist;
the end equation and signed machine relation pass. Both CLI forms agree through
source checking and export/load. Supplying `ends=[0,1]` for those same starts
instead fails only the end equation: resource checks use the supplied intervals.


## Bin packing and rectangles

The checker accepts these five native names and their literal `gecode_` and
`fzn_` prefixes, for fifteen exact names:

| Native name | Positional arguments | Deterministic output |
| --- | --- | --- |
| `gecode_bin_packing_load` | variable integer loads, variable integer bins, fixed nonnegative integer weights, fixed integer minimum bin label | marked scalar or complete load array |
| `gecode_nooverlap`, `gecode_nooverlap_fzn` | variable integer x, width, y, height arrays | none |
| `gecode_nooverlap_optional` | variable integer x, fixed nonnegative width, variable integer y, fixed nonnegative height, variable Boolean presence arrays | none |
| `gecode_nooverlap_flex_optional` | variable integer x, width, right, y, height, top and variable Boolean presence arrays | none |

Declarations match these argument roles regardless of formal parameter names.
The public `gecode_nooverlap` wrapper has optional coordinates and a different
signature; it belongs to compiler lowering. Bare `bin_packing`,
`bin_packing_load`, `bin_packing_capa` and `diffn` are also library wrappers,
whose decomposition routes are recorded in the
[profile inventory](../../docs/checker-coverage.md).

Bin and weight arrays have equal lengths. The load array independently fixes the
number of bins: position 1 represents the minimum label, position 2 the next
label, and so on. Every item's bin must lie in that interval, including items
with zero weight. Empty item arrays produce zero loads. An empty load array
admits no items. The nonempty label interval and each weight sum use checked i64
arithmetic; allocation depends on the load count, never the label offset.
A bad bin reports its item position and still allows checking loads for the
represented bins. Marked loads derive from known bins; repeated output aliases
need equal calculated values, and a load that also occurs as a bin input cannot
define itself. Supplied loads are checked without replacement. Domain failures
on calculated loads do not suppress downstream constraints. No inverse operation
chooses bins from loads.

All rectangle arrays have equal lengths. Widths and heights must be nonnegative,
including absent entries; negative fixed optional sizes are malformed model
parameters. An absent rectangle needs no coordinates for this relation. Every
present pair must satisfy
`right[i] <= x[j] || right[j] <= x[i] || top[i] <= y[j] || top[j] <= y[i]`.
Touching boundaries pass. Zero-sized objects participate: an interior point or
line can conflict, while coincident points separate. This is strict `diffn`
behavior; the zero-size exemptions of `diffn_nonstrict` are different.

Mandatory and fixed optional forms calculate right/top with checked addition.
The flex form uses the supplied right/top coordinates, including contradictory
or reversed tuples. It adds neither coordinate equalities nor endpoint-order
checks. Callers express `x + width = right` and `y + height = top` separately;
marked arithmetic equations can derive those outputs. This concrete convention
keeps explicit endpoints stable across candidate edits. Gecode instead rewrites
assigned sizes to its fixed-size relation and can discard supplied endpoints;
its posting/propagation behavior is not the checker convention. Rectangles
choose no coordinates, sizes or presence values.

Typed `bin_packing` findings contain every item, load and known bin/load fault.
Load records give the bin label, supplied value, calculated value and contributing
item positions. Typed `rectangles` findings give argument references, all known
dimension faults and conflicting pairs in position order. Mathematical integers
use exact decimal strings. Known precondition faults can accompany incomplete
reports; malformed arrays and artifacts fail admission, and arithmetic overflow
produces a located execution error. Existing report shapes and artifact version
1 are unchanged. The [direct control](../../examples/checker-compilation/packing.fzn)
exercises load completion, optional rectangles and a separate endpoint equation
with values above 2^53.

### Compile-only packing controls

Compile [public wrappers](../../examples/checker-compilation/packing-controls.mzn)
under both pinned profiles, and [native interfaces](../../examples/checker-compilation/packing-gecode-controls.mzn)
under pinned Gecode, without solving:

```sh
checker_run=$(mktemp -d "$PWD/target/checker-packing.XXXXXX")
minizinc --compile --solver crates/zincite-check/minizinc/standard.msc -O0 --keep-paths --no-half-reifications --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --no-output-ozn --output-paths-to-file "$checker_run/standard.paths" --output-fzn-to-file "$checker_run/standard.fzn" examples/checker-compilation/packing-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file "$checker_run/gecode.paths" --output-fzn-to-file "$checker_run/gecode.fzn" examples/checker-compilation/packing-controls.mzn
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib -I crates/zincite-check/minizinc --no-output-ozn --output-paths-to-file "$checker_run/native.paths" --output-fzn-to-file "$checker_run/native.fzn" examples/checker-compilation/packing-gecode-controls.mzn
```

The public Gecode control emits native load calls with minimum label `-2` and
`gecode_nooverlap_fzn` for strict `diffn`. MiniZinc normalizes the source load
index set to a one-based generated array, drops its `is_defined_var` and
`defines_var` markers, and merges the identical capacity relations, losing one
source label. The standard profile emits linear load equalities, integer
reifiers, Boolean conversions and a separation clause. Its load markers also
disappear; marked auxiliary reifiers and conversions survive. These inspected
models accept supplied load values but do not complete the unmarked loads.
Applications must use the emitted FlatZinc array names. The native control emits
all four declared interfaces, preserves labels and payloads, and keeps a marked
linear right-end equation; its load array has the same missing-marker gap.
The checker needs usable definition hints to derive these outputs. Without
them, applications must supply the values.

## Aggregate, ordered and weighted sets

The checker admits the literal registry roots below and their `gecode_` and
`fzn_` prefixes, applied once. A root that starts with `gecode_` therefore also
has a `gecode_gecode_` spelling. These sixteen roots have 48 spellings. Predicate
declarations match positional types and roles regardless of their formal names.
Set arguments accept compact integer ranges.

| Root | Positional arguments | Marked scalar output |
| --- | --- | --- |
| `disjoint` | Two variable sets | None |
| `array_set_union` | Variable set array, variable set result | Result |
| `array_set_partition` | Variable set array, fixed universe or variable set result | Variable result |
| `gecode_array_set_intersect` | Variable set array, variable set result | Result |
| `array_set_seq` | Variable set array | None |
| `array_set_seq_union` | Variable set array, variable set result | Result |
| `set_convex` | Variable set | None |
| `gecode_set_convex_hull` | Variable set, variable set hull | Hull |
| `gecode_set_atmost_one` | Variable set array, fixed nonnegative cardinality | None |
| `gecode_set_weights` | Fixed element array, fixed weight array, variable selected set, variable integer total | Covered total |
| `gecode_set_min`, `gecode_set_max` | Variable set, variable integer | Integer for nonempty input |
| `gecode_set_min_reif`, `gecode_set_max_reif` | Variable set, variable integer, variable Boolean | Boolean truth |
| `gecode_set_min_imp`, `gecode_set_max_imp` | Variable set, variable integer, variable Boolean guard | None |

Union, partition and sequence-union of an empty array equal the empty set.
Partition also requires every pair to be disjoint. Repeated set aliases check
normally and can expose a pair conflict. The empty Gecode intersection is exactly
`-1073741822..1073741822`, with cardinality `2147483645`; exported constants
`GECODE_SET_MIN`, `GECODE_SET_MAX` and `GECODE_SET_CARDINALITY` name that universe.
Nonempty intersection starts from its first set and does not clip i64 members
outside the Gecode universe. Generic storage still uses the full i64 range.

A sequence requires each member of a set to precede every member of every later
nonempty set. An empty middle set does not break this comparison. Sequence-union
checks that order and the ordinary union. Convexity accepts empty sets and sets
stored as one inclusive range. Hull is empty for empty input and otherwise spans
the minimum through the maximum. Atmost_one requires each set to have the fixed
cardinality and every pair intersection to have cardinality at most one. The
individual cardinality rule applies even with fewer than two sets or cardinality
zero or one. Cardinalities above i64::MAX give located execution errors.

Plain min/max asserts nonemptiness and equality to the extremum. Empty input
makes the proposition false. Full reification checks equivalence; implication
checks guard → proposition. A false implication guard skips unavailable inputs.
An empty set determines false reified truth without knowing the integer. A plain
empty extremum supplies no integer and can report a violation with incomplete
input.

Weights use equally long fixed element/weight arrays. Pair order is arbitrary;
weights may be negative or zero, and empty arrays are valid. The first original
row for an element supplies its weight. Calculation visits distinct elements in
ascending order and checks each i64 addition, including intermediate overflow.
The selected set must be a subset of listed elements. A coverage finding carries
compact uncovered ranges and does not derive a total. Calculation iterates the
fixed mapping, so a huge selected interval never causes member enumeration.
Original row positions and weights are available in typed findings. This direct
Gecode mapping differs from the standard `fzn_sum_set` decomposition, which sums
every matching row, including duplicates, and ignores unlisted members.

Completion uses `defines_var`, canonical targets and the usual competing-definition
and default rules. Declaration `is_defined_var` markers are optional. A scalar set result that
also occurs among the inputs has no supported direction. Partition or sequence
failure does not prevent deriving a known aggregate. Supplied outputs stay
supplied and are checked; derived domain violations do not suppress downstream
checks. These operations do not invert a result into missing sets or define a
whole array output.

`set_global` findings distinguish aggregates, hull/convexity, sequence pairs,
individual/intersection cardinalities, extrema and weights. They carry available
arguments or unresolved variable references, one-based input positions, expected
and supplied values, pair conflicts and uncovered ranges. Mathematical integers
use decimal strings; positions use JSON numbers. Known pair/cardinality/coverage
faults can accompany incomplete input. Existing set relations and native relation
codes 1–10 keep their contracts and report shapes.

### Compile-only set controls

[set-global-controls.mzn](../../examples/checker-compilation/set-global-controls.mzn)
uses public union, intersection, partition, disjoint, minimum and weighted-set
calls. [set-global-gecode-controls.mzn](../../examples/checker-compilation/set-global-gecode-controls.mzn)
uses native hull, convexity, ordering, fixed cardinality, extrema and intersection,
plus the union and sum wrappers. Compile the public control with either the
[standard](#standard-library-compilation) or
[pinned Gecode](#pinned-gecode-compilation) recipe. The native control includes
`gecode_fzn.mzn` and needs the Gecode profile. Substitute the model path in the
chosen compilation command and use fresh FlatZinc/path output files. Earlier
public controls selected installed Gecode with the narrow checker globals
directory; that original recipe is separate from the packaged standard profile.

The compiled controls expose changes to domains and metadata. The public standard
partition folds its union to `1..4` and intersection to empty, merges the
standalone disjoint check, and adds an argument-level `mzn_path` to a set literal;
the checker validates that path and records it with the constraint. Its weighted
total domain tightens from `-10..10` to `-1..7`. Both public profiles emit
`defines_var` for the minimum while dropping its `is_defined_var`; the checker
accepts that hint. Gecode's weighted total keeps `-10..10`. The standard control
prepares with the packaged profile; full Gecode profile accounting is separate.

The native Gecode control prepares unchanged and keeps the new operations,
constraint labels, application annotations and explicit scalar definition
markers. Its `common`, `hull_result`, minimum, reifier and weighted total derive.
The union wrapper introduces two equality-linked intermediate sets without
definition markers; supplying those sets makes the candidate complete. The
sequence-union check also constrains the union but has no definition marker.
The [direct FlatZinc control](../../examples/checker-compilation/set-globals.fzn)
provides a complete application-edit path without those compiler gaps, including
values above 2^53 and original duplicate weight rows. Both native CLI forms and
source/export/load checks exercise the direct and admitted native controls.

## Selected sets and channels

These ten literal roots each accept exactly three spellings: the root itself,
`gecode_` plus the root, and `fzn_` plus the root. For example,
`gecode_range`, `gecode_gecode_range` and `fzn_gecode_range` are admitted;
`range` is a MiniZinc wrapper. These ten roots have 30 spellings.
Matching bodyless declarations may use any legal formal names. Set, integer and
Boolean value arguments have variable roles; offsets and precedence constants
are fixed integers, and the explicit universe is a fixed set.

| Literal root and positional arguments | Supported marked output |
| --- | --- |
| `gecode_link_set_to_booleans(set, flags, offset)` | Set from all flags, or flags from the set |
| `gecode_int_set_channel(integers, integer_offset, sets, set_offset)` | Buckets from all integers; an integer from fully known buckets with exactly one membership |
| `gecode_inverse_set(forward, inverse, forward_offset, inverse_offset)` | Either set-array side from the complete opposite side |
| `gecode_range(integers, offset, selection, result)` | Selected integer image `result` |
| `gecode_precede_set(sets, first, second)` | None |
| `gecode_set_channel_sorted(integers, set)` | Member set from integers, or sorted integers from a set of exactly the array's cardinality |
| `gecode_array_set_element_union(indices, sets, result)` | Selected union `result` |
| `gecode_array_set_element_intersect(indices, sets, result)` | Selected intersection `result` |
| `gecode_array_set_element_partition(indices, sets, result)` | Selected union `result`; overlapping selected sets also fail |
| `gecode_array_set_element_intersect_in(indices, sets, result, universe)` | Raw selected intersection `result`; result must also be a subset of `universe` |

Selected-set indices are `1..length(sets)`; zero is explicitly excluded. Only
selected sets are needed to calculate an aggregate. An unselected unknown set,
including an unselected reference to the output, does not block it. Empty union
and partition are empty. Empty ordinary intersection is the pinned Gecode
universe `-1073741822..1073741822`; empty `intersect_in` is its explicit universe.
For nonempty `intersect_in`, the checker calculates the raw intersection and
checks its subset obligation separately. It never clips that intersection.
Selected references at different positions also participate in partition checks.
An overlap or universe fault does not suppress a determined aggregate; invalid
selection indices supply no invented result.

Offsets are nonnegative mathematical labels. Array intervals must fit native
index arithmetic. Fixed parameters also satisfy the registry's explicit
Gecode set/integer domain guards, including precedence constants and the
Boolean channel's disjoint prefix. Candidate integers and set members use the
usual checked i64 policy. Padding uses compact intervals, so large offsets and
wide invalid sets do not allocate or enumerate their members.

For the Boolean channel, flag position `p` (zero-based) represents membership
of `offset+p`. No other set members are allowed. Empty flags require an empty
set, even with a nonzero offset. For range, a nonempty integer array has virtual
zero entries at `0..offset-1`; any selected prefix position contributes the
single value zero. An empty integer array has no virtual prefix. Only selected
visible integers are needed for its image.

The integer/set channel follows the full padded equivalence `x[i]=j` iff
`i in y[j]`. Let `a=integer_offset`, `b=set_offset`, `N=0` for empty integers
and otherwise `a+length(integers)`, and `M=b+length(sets)`. A nonempty integer
array has fixed zero entries below `a`. Set-array prefixes always exist: bucket
zero contains `0..a-1`, and other prefix buckets are empty. Visible integers
must lie in `b..M-1`, and visible buckets must be subsets of `a..N-1`.
These obligations can expose a padding fault even with unavailable visible
inputs. In particular, a visible bucket zero cannot accommodate a nonempty
integer prefix, and a nonempty dummy bucket cannot fit an empty integer array.
Inverse-set prefixes are empty on both sides; visible members must fall within
the opposite visible array, and membership is transposed using the two offsets.

Set precedence uses exclusive membership: a position containing `second`
without `first` needs an earlier position containing `first` without `second`.
A set containing both contributes neither condition. Equal constants make the
relation vacuous. Sorted channels require strict increasing order, equality of
the member set and exact cardinality. Missing integer positions are calculated
only after set cardinality equals the actual array length.

Definitions require `defines_var`; declaration `is_defined_var` markers are optional.
Scalar targets and complete canonical array target groups are supported; group
slot multisets and element types must match the whole output array. Repeated
output aliases derive only when all corresponding calculated values agree.
Actual self-dependencies stay unresolved. Competing definitions fail preparation.
Supplied values are immutable, and derived domain failures still feed downstream
checks. Every check starts from fresh values.

`set_channel` findings carry named argument roles, available typed values or
unresolved references, one-based visible positions, logical index labels and
compact outside ranges. Faults distinguish membership, membership counts,
padding, expected values, partition overlap, precedence, strict order and
cardinality. Mathematical integers are decimal strings; structural positions
are JSON numbers. Known faults can accompany incomplete input. Text rendering
uses these fields directly. Existing artifact and report shapes keep version 1.

### Compile-only channel controls

[set-channel-controls.mzn](../../examples/checker-compilation/set-channel-controls.mzn)
uses public Boolean/integer/set channels, inverse sets, range, roots, set member,
value precedence and sequence precedence. Compile it with both pinned profiles
using the flags in the preceding compile-only set controls section.
[set-channel-gecode-controls.mzn](../../examples/checker-compilation/set-channel-gecode-controls.mzn)
uses the Gecode profile and emits all ten literal roots, including the registry
union declaration absent from the public Gecode entry point. Its six marked
scalar outputs keep both definition annotations, domains and source labels.
The native control prepares unchanged; directly calculated candidates pass
through both CLIs and source/export/load paths.

The public standard profile decomposes these wrappers into admitted Boolean,
integer and set primitives. The Gecode profile emits the native channels,
selected union, set precedence, extrema and existing primitives. The standard
control's roots decomposition copies `is_defined_var` onto a clause; the Gecode
control's member-set decomposition copies it onto a guarded set relation. Both
copies are accepted metadata. The standard control prepares with the packaged
profile, including operand paths and optional variable markers. The
[Gecode profile accounting](../../docs/checker-coverage.md#gecode-profile-accounting)
records its full include/decomposition closure.

The [direct FlatZinc specimen](../../examples/checker-compilation/set-channels.fzn)
provides complete marked array outputs, repeated edits, supplied conflicts and
incomplete selection feedback, including a range image above 2^53. Native and
parser-free Wasm checks use it without invoking MiniZinc or a solver.

## Number theory, products and distances

The checker admits these literal Gecode registry roots and each once-applied
`gecode_` or `fzn_` prefix, for 60 exact spellings. Formal parameter names do not
affect admission. The existing integer power forms keep their documented rules.

| Roots | Concrete relation | Supported marked output |
| --- | --- | --- |
| `gecode_gcd_int` and `_reif` / `_imp` | Nonnegative gcd of operand magnitudes; `gcd(0,0)=0` | Argument 3, or full reifier Boolean |
| `gecode_divides_int` and `_reif` / `_imp` | Divisor has an integer multiple equal to the dividend; zero divides only zero | Full reifier Boolean |
| `gecode_product_int` and `_reif` / `_imp` | Exact signed product; empty product is one | Argument 2, or full reifier Boolean |
| `gecode_product_mod_fixed_fzn` / `gecode_product_mod_var_fzn` and `_reif` / `_imp` | Positive modulus and canonical Euclidean product residue in `0..modulus-1` | Argument 3, or full reifier Boolean |
| `gecode_int_nroot(x,n,y)` | `y=floor(root(x,n))`; negative `x` requires odd `n` | Argument 3 |
| `gecode_inter_distance` / `gecode_inter_distance_fzn` | Separation is nonnegative and every positional pair is at least that far apart | None |
| `gecode_minimum_distance_fzn` / `gecode_minimum_distance_required_fzn` | Exact matrix minimum over all selected position pairs, plus inclusive pair requirements when supplied | Argument 4 |

Completion requires `defines_var` and alias checks; declaration
`is_defined_var` markers are optional. Full reification derives only its last Boolean; implications supply
no output. The checker never chooses factors, radicands, divisors or sites.
Supplied outputs are checked without replacement. Every check starts with fresh
values, and derived values outside their domains still feed downstream checks.

Fixed moduli and root degrees must be in `1..2147483646`, including under a false
implication guard. A nonpositive variable modulus instead makes the proposition
false: false full reification passes, and an inactive implication imposes no
restriction. Negative factors use Euclidean residues; an empty modular product
is `1 mod modulus`. Modular accumulation handles large factors without forming
their full product. Other required arithmetic outside i64 produces a located
execution error, as does an even root of a negative radicand. Integer root search
uses no floating-point approximation and no loop proportional to the degree.
For example, the floor cube root of `-9` is `-3`, while that of `-8` is `-2`.

Inter-distance accepts empty and singleton arrays but still requires nonnegative
separation. Repeated values and aliases participate as distinct positions.
Matrix minimum distance requires at least two selected positions and a positive
fixed site count. Sites are zero-based labels into a fixed row-major square
matrix. Matrix entries and pair requirements are in `0..2147483646`; both
matrices must be symmetric with zero diagonals. The optional requirements matrix
has one row per selected position, rather than per site. No triangle inequality
or distinctness condition applies. The number of position pairs must fit the
same Gecode integer limit. Invalid labels produce range findings before matrix
lookup. Valid known sites determine the minimum even when a pair requirement
fails; invalid labels supply no minimum.

`ViolationKind::NumberTheory` carries named argument roles, one-based factor
positions, available values or unresolved references, the calculated result when
defined, relation truth and condition. `ViolationKind::Distance` carries selected
positions and typed negative-separation, range, pair-bound and minimum conflicts.
One distance finding can include several pair failures and a wrong supplied
minimum. JSON integers use decimal strings throughout.

The [direct FlatZinc specimen](../../examples/checker-compilation/arithmetic-extensions.fzn)
supports repeated native/Wasm edits, products above 2^53, supplied conflicts,
invalid labels and unresolved results for nonpositive moduli. The
[compile-only Gecode control](../../examples/checker-compilation/arithmetic-extensions-gecode-controls.mzn)
uses the pinned sources from the coverage inventory:

```sh
minizinc --compile --solver org.gecode.gecode -O0 --keep-paths --no-output-ozn \
  --stdlib-dir target/checker-upstream/minizinc-2.10.1/share/minizinc \
  --globals-dir target/checker-upstream/gecode-main/gecode/flatzinc/mznlib \
  -I crates/zincite-check/minizinc \
  --output-fzn-to-file target/arithmetic-extensions.fzn \
  --output-paths-to-file target/arithmetic-extensions.paths \
  examples/checker-compilation/arithmetic-extensions-gecode-controls.mzn
```

MiniZinc 2.10.1 emits its eight native predicates with labels and source paths,
plus all seven explicit definition marker pairs. Source preparation, export and
validated loading check directly calculated candidates through both CLI forms.
For `left_value=-9;right_value=6;residue_modulus=5;chosen_sites=[0,1];`,
the derived results are gcd `3`, product `-54`, both residues `1`, floor root `-3`, minimum `7` and divisibility
`true`. Equal selected sites fail both the pair requirement and inter-distance;
the derived minimum is still zero. No solver runs. This control does not settle
the compiler annotation and decomposition limits documented above.

The control passes Zincite formatting. Default linting reports the pinned
`gecode.mzn:140` parameter `S` naming warning and 44 analysis limitations.
These diagnostics describe upstream source and incomplete static analysis;
the pinned library files are unchanged.
