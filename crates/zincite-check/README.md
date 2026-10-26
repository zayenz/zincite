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
at version 1 with additive set value/domain, set operation and strict global
records; Boolean and integer records keep their existing shapes. Loading rejects
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
| Labelled `start <= 2` for a `var 0..8` scalar | Domain becomes `0..2`; the source constraint ID and label disappear; `start=8` gives a domain finding with variable metadata |
| The model's `forall` finish equations | Two `int_lin_eq` constraints share a label and carry distinct `[1]` and `[2]` entity payloads |
| Optional order symmetry breaker | With the recipe's ignore flag, `[8,1]` passes the control; with the flag false, its emitted `int_lin_le` fails with `job_order/[1,2]` |

Use the ignore flag when an application allows either representative of a
symmetry. The main model has no symmetry breaker. Inspect each compiled model's
predicates, declarations and metadata before admitting it; the controls establish
these source forms, not equivalence for arbitrary MiniZinc models. The detailed
control sources and observations are in the repository's
[compilation contract](../../.zdev/checker/background/compilation-contract.md).

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

- Scalar Boolean, signed 64-bit integer and finite integer set parameters and
  variables. Integers use decimal, `0x` hexadecimal or `0o` octal spelling, with
  an optional minus sign. Integer domains use literal inclusive intervals or
  enumerations. A set domain is an upper bound: `var set of 1..3` allows every
  subset, including the empty set. `var set of int` admits finite i64 sets.
- Parameters require literal initialization and have no annotations. Variables
  accept fixed literals, earlier scalar references, literal arrays indexed by
  `1..n` (including `1..0`), and earlier same-shape array aliases. Literal array
  access such as `values[2]` selects a scalar; bounds and types are checked.
- Items follow predicate, parameter, variable, constraint, solve order. There
  is exactly one final solve item: `satisfy`, or `minimize`/`maximize` of a scalar
  integer literal or reference. An available objective value appears in reports;
  the checker makes no optimality claim.
- Exactly the ten predicates in the coverage table below. Linear coefficients
  and bounds are fixed integers; coefficient and operand arrays have equal lengths.
  Bodyless declarations for these names validate types, fixed/variable roles and
  scalar/array shapes independently of formal spelling. A declared fixed length
  or restricted domain also constrains calls. `fzn_disjunctive_strict` requires
  its declaration; the other nine names are builtins. Unknown, duplicate,
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
  `set_search`, nested `seq_search`, `warm_start`/`warm_start_array`, `int_assign`,
  integer/Boolean/set/float default searches, restart strategies and
  `relax_and_reconstruct`. Referenced arrays, fixed values, lengths and strategy
  roles are validated recursively. `restart_geometric` accepts a finite float
  literal base greater than one and a positive integer scale; this annotation
  constant does not introduce mathematical float values.
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

Mathematical floats, other set operations, includes, output expressions, computed
initializers/objectives, additional predicates and unknown semantic annotations
are rejected explicitly. External executable/DLL callbacks and restart-history
hooks also diagnose; the checker does not execute search or external code.
Syntax acceptance by the shared parser does not imply checker support.

## Functional completion

This is the complete predicate coverage, including the constraints emitted
by both application models:

| Predicate | Direct check | Completion through matching `defines_var` |
| --- | --- | --- |
| `int_lin_eq(a,x,c)` | `sum(a[i]*x[i]) = c` | One integer output occurring once with a nonzero coefficient; all other terms known and exact division |
| `int_lin_le(a,x,c)` | `sum(a[i]*x[i]) <= c` | None |
| `int_le_reif(x,y,b)` | `b <-> (x <= y)`, including `b=false` | Boolean result `b` from known integer operands `x` and `y` |
| `set_in(x,s)` | Integer membership | None |
| `set_eq(s,t)` | Set equality | Either set output when it occurs once |
| `set_subset(s,t)` | Subset inclusion | None |
| `set_card(s,n)` | Set cardinality equals integer `n` | Integer output `n` |
| `set_union(s,t,r)` | `r = s union t` | Third argument `r`, absent from inputs |
| `set_intersect(s,t,r)` | `r = s intersect t` | Third argument `r`, absent from inputs |
| `fzn_disjunctive_strict(start,duration)` | Every pair satisfies `start[i]+duration[i] <= start[j]` or the reverse | None; durations must be fixed nonnegative integers |

Before checking domains and relations, a dependency worklist calculates the
supported `defines_var` outputs until no new values are available:

- `int_lin_eq(a,x,c)` defines an integer output that occurs exactly once with
  a nonzero coefficient. Its value is `(c - sum(other terms)) / coefficient`,
  only when all other terms are known and the division is exact. Negative
  coefficients and results are supported; nonexact division leaves the output
  unresolved.
- `int_le_reif(x,y,b)` defines only its Boolean result, calculating `x <= y`
  when both integer operands are known. A false comparison derives `false`.

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
so completion does not divide by zero. This subset has no division or remainder
predicate. Integer sets use compact sorted, disjoint, nonadjacent inclusive ranges; large
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
