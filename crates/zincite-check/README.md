# zincite-check

A native Rust checker for a small FlatZinc dialect targeting MiniZinc 2.10.1.
Prepare a model once, then check candidate assignments repeatedly. Checking
performs no I/O and starts no compiler or solver.

This checkout introduces the checker. Install it locally with
`cargo install --locked --path crates/zincite-check`, or install the root command
with `cargo install --locked --path .`. It is not part of the published 0.3.0
packages.

## Try the compiled example

From the repository root:

```sh
cat > /tmp/check-complete.dzn <<'DATA'
start = [1, 4];
finish = [4, 6];
starts_early = true;
DATA
cargo run -p zincite-check -- examples/checker-compilation/model.fzn /tmp/check-complete.dzn
cargo run -p zincite -- check --format json examples/checker-compilation/model.fzn /tmp/check-complete.dzn
```

Both commands report a valid, complete candidate. Changing the assignment to
`start = [8,5]; finish = [11,7]; starts_early = false;` reports three violations:
the first finish domain, precedence and the start budget. Each finding includes
its location and application metadata. Missing values produce an incomplete
report, alongside any known violations.

Supply auxiliary values such as `finish` and `starts_early` in this increment.
Definition annotations are validated and recorded, but functional definition
calculation is not implemented yet. The saved candidate fixtures in
`examples/checker-compilation/candidates/` supply only primary decisions for
that later capability. For the verified offline compilation recipe, read
[the compilation contract](../../.zdev/checker/background/compilation-contract.md).

## Library use

```rust
use zincite_check::{prepare, parse_assignment, Status};

let model = prepare("small.fzn", "var 0..3: x; constraint int_lin_le([1],[x],2); solve satisfy;").unwrap();
let assignment = parse_assignment("candidate.dzn", "x=3;").unwrap();
let report = model.check(&assignment).unwrap();
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
parser and CLI. Prepared-model loading and WebAssembly bindings are later work;
this increment provides no parser-free model constructor.

## Accepted model and assignment forms

- Scalar Boolean and signed 64-bit integer parameters and variables. Integer
  domains can be literal inclusive intervals or literal enumerations. Every
  variable declaration's domain is checked, including restrictions on aliases.
- Fixed literal initializers, scalar aliases, one-dimensional literal arrays
  indexed by `1..n` (including `1..0`), and same-shape array aliases. Parameters
  must be fixed. Arrays need a literal initializer or array alias.
- Exactly three direct predicates: `int_lin_eq(a,x,c)`, `int_lin_le(a,x,c)` and
  `int_le_reif(x,y,b)`. Linear coefficients and bounds are fixed integers;
  coefficient and operand arrays have equal lengths. Reification checks
  `b <-> (x <= y)`, including when `b` is false. Half reification is unsupported.
- One solve item: `satisfy`, or `minimize`/`maximize` of a scalar integer literal
  or reference. An available objective value appears in the report; the checker
  makes no optimality claim.
- `checker_value(string, literal_integer_array)` on declarations and
  `checker_constraint(string, literal_integer_array)` on constraints;
  `mzn_path(string)`, `mzn_constraint_name(string)`, `output_var`, and
  `output_array([1..n])` in their documented contexts. Definition annotations
  require matching `is_defined_var` and a unique `defines_var` relation. Linear
  outputs must occur once with a nonzero coefficient in an equality;
  a reified definition must name the Boolean result.
- Recognized solve search metadata: `int_search`, `bool_search` and nested
  `seq_search`. It does not change candidate checking.

Names are unquoted FlatZinc identifiers. `.dzn` assignments contain only Boolean
or signed decimal integer literals, or one-dimensional arrays of those literals.
No missing-value placeholders, expressions, indexed assignment targets, JSON
input or solver solution streams are accepted. A partial candidate names the
available scalars; an array assignment must supply every element.

Unknown names, duplicate entries, contradictory explicit assignments through
aliases, wrong types and wrong array lengths are input errors. Assignments
cannot change parameters. A correctly typed supplied value outside a domain is
a violation. Fixed values and singleton domains establish unsupplied values;
they never replace a supplied value. A supplied defined output is checked by
its defining relation. No failed domain check suppresses a later relation.

Float/set values, user predicate declarations, includes, output expressions,
computed initializers/objectives, additional predicates and unknown annotations
are rejected explicitly. Syntax acceptance by the shared parser does not imply
checker support. This is a direct checker for the documented subset, not a
MiniZinc interpreter or a general FlatZinc solver.

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
checked arithmetic in array order; overflow is a located execution error,
including intermediate overflow. There is no division or remainder operation
in this subset. JSON writes values, coefficients, sums, bounds, domain endpoints
and application payload integers as decimal strings, so consumers do not round
integers beyond 2^53. Booleans stay Boolean; source offsets, ordinals and array
positions are ordinary JSON numbers. Report JSON is an output format, not a
prepared-model loading format.

Both commands accept `--format text` (the default) or `--format json`, followed
by a model path and an assignment path. Reports go to stdout. Errors go to
stderr with no partial stdout, including in JSON mode. Neither command writes
input files.

| Status | Meaning |
| --- | --- |
| 0 | Valid, complete candidate |
| 1 | Invalid or incomplete report |
| 2 | Usage, I/O, syntax, preparation, assignment or arithmetic error |
