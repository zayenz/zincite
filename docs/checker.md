# Check an application's candidate assignments

An editor often needs to show why a candidate is infeasible while allowing the
user to keep editing it. `zincite-check` evaluates that candidate against a
compiled FlatZinc model and returns structured findings. Prepare the model once,
then call it with the application's current values after each edit.

The application maps its state to FlatZinc names and maps findings back to its
entities. The checker derives supported auxiliary values and checks the compiled
relations. It does no search, starts no solver and makes no claim about
optimality. Missing input is an ordinary report outcome.

This guide uses a small delivery model to show that boundary. The
[checker reference](../crates/zincite-check/README.md) contains the predicate
catalogue, precise numeric semantics and compiler recipes. The
[coverage inventory](checker-coverage.md) records the pinned libraries and their
compilation limits. For a complete graphical example, see the
[workshop editor](../examples/checker-browser/README.md).

## Start with one candidate

Suppose an application edits a delivery's start time. Delivery takes three hours,
and the model requires it to finish by hour 7. This FlatZinc model makes the
functional relation explicit:

```minizinc
var 0..8: start :: checker_value("start", [37]);
var 0..9: finish :: checker_value("finish", [37]);
constraint int_plus(start, 3, finish)
    :: defines_var(finish)
    :: checker_constraint("travel_time", [37]);
constraint int_le(finish, 7)
    :: checker_constraint("deadline", [37]);
solve minimize finish;
```

Save it as `delivery.fzn`. A candidate file `candidate.dzn` contains literal
assignments, for example `start = 1;`. The checker derives `finish = 4`, checks
both domains and both constraints, and reports a valid, complete candidate with
objective value 4. The objective is the value of this candidate; it does not
establish that 4 is the best possible finish.

From the repository root, either command checks the same files:

```sh
cargo run -p zincite-check -- delivery.fzn candidate.dzn
cargo run -p zincite -- check --format json delivery.fzn candidate.dzn
```

The checker is available from this checkout and is not included in the published
0.3.0 packages. To use the shorter commands shown below, install the standalone
binary with `cargo install --locked --path crates/zincite-check` and the root
binary with `cargo install --locked --path .`.

Try these candidate contents in sequence:

| Candidate | Result |
| --- | --- |
| `start = 1;` | Valid and complete; finish is 4. |
| `start = 8;` | Invalid and complete; finish is 11, outside its domain and past the deadline. |
| Empty file | Incomplete; start and finish are unresolved. |
| `start = 1; finish = 5;` | Invalid and complete; the supplied finish conflicts with the travel-time equation. |
| `start = 1;` again | Valid and complete; the earlier supplied finish has no effect. |

A derived value outside its domain still feeds downstream checks. For the second
candidate, the checker reports both the domain failure and the deadline failure.
A supplied output is checked as supplied, so the fourth candidate's objective
is 5. Each call starts with fresh values.

## Prepare once in Rust

A native application can parse FlatZinc once and construct typed assignments for
each edit. Add a path dependency on this checkout:

```toml
[dependencies]
zincite-check = { path = "../zincite/crates/zincite-check" }
```

Adjust the path relative to the application's `Cargo.toml`. With the FlatZinc
above beside `src/main.rs` as `src/delivery.fzn`, this is a complete program:

```rust
use zincite_check::{
    Assignment, AssignmentEntry, AssignmentValue, PreparedModel, Status, Value,
    parse_assignment, prepare,
};

fn main() {
    let model = prepare("delivery.fzn", include_str!("delivery.fzn")).unwrap();
    let candidate = Assignment {
        entries: vec![AssignmentEntry {
            name: "start".into(),
            value: AssignmentValue::Scalar(Value::Integer(1)),
            location: None,
        }],
    };
    let valid = model.check(&candidate).unwrap();
    assert_eq!(valid.status, Status::Valid);
    assert!(valid.completeness.complete);
    assert_eq!(valid.objective.unwrap().argument.value, Value::Integer(4));

    let invalid = parse_assignment("candidate.dzn", "start = 8;").unwrap();
    let report = model.check(&invalid).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 2);

    let missing = model.check(&Assignment::default()).unwrap();
    assert_eq!(missing.status, Status::Incomplete);
    assert!(!missing.completeness.complete);
    assert_eq!(model.check(&candidate).unwrap().status, Status::Valid);

    let artifact = model.to_json().unwrap();
    let loaded = PreparedModel::from_json(&artifact).unwrap();
    assert_eq!(loaded.check(&candidate).unwrap().status, Status::Valid);
}
```

The example uses `unwrap()` to keep the lifecycle visible. An application should
handle preparation diagnostics, assignment errors and execution errors at their
respective calls. Invalid and incomplete reports arrive as `Ok(CheckReport)`.
Checking never mutates the supplied assignment or saves its derived values.

## Read findings and completeness separately

`CheckReport` has `status`, `violations`, `completeness` and an optional
`objective`. The status answers what the available checks established:

| `status` | Meaning |
| --- | --- |
| `valid` | Every variable is resolved and every check passed. |
| `invalid` | At least one violation was established. Read completeness to see whether other checks are blocked. |
| `incomplete` | No violation was established, but some variables or checks are unresolved. |

An invalid report may have `completeness.complete: false`. Show its known
findings alongside the missing-input feedback. `unresolved_variables` identifies
missing values; each `blocked_checks` entry identifies a domain or constraint
check and its missing variables. An incomplete report says nothing about whether
a satisfying completion exists. The saved
[`incomplete-invalid.dzn`](../examples/checker-compilation/candidates/incomplete-invalid.dzn)
candidate for the workshop model demonstrates a known finish-domain violation
with other checks blocked.

Many global relations need their full input arrays before they can be checked.
Some checks can establish a finding earlier, such as an available out-of-domain
value or an invalid element index. The checker does not promise to infer every
violation implied by partial input. A blocked check is distinct from a failed
check, even when an application could deduce more from its own state.

The native CLI writes reports to stdout and errors to stderr. Its exit status
is 0 for a valid complete report or a successful export, 1 for an invalid or
incomplete report, and 2 for an input or execution error. Exit 1 does not mean
the library check failed. This distinction matters when invoking the CLI from a
script that normally stops at any nonzero exit status.

Unknown assignment names, duplicate entries, contradictory assignments through
aliases, wrong types and wrong array lengths are assignment errors. Correctly
typed values outside logical domains are violations. Checked arithmetic overflow,
division by zero and undefined float operations are execution errors. Error
results do not provide a partial `CheckReport`.

## Connect feedback to application entities

The delivery's entity ID is 37. `checker_value("finish", [37])` associates the
variable with it, and `checker_constraint("deadline", [37])` associates the
constraint with it. The annotations are declared in the packaged
[`checker_annotations.mzn`](../crates/zincite-check/minizinc/checker_annotations.mzn)
for MiniZinc source compilation.

Each violation contains a `check` reference and typed `details`:

- `CheckRef::Constraint` has the predicate, ordinal, location and metadata.
  `metadata.application` contains the constraint's `id` and `entities`.
- `CheckRef::Domain` has the variable reference and checked domain. The variable's
  metadata identifies its application value; aliases and one-based array
  positions identify an entry of an application array.
- `ViolationKind` supplies the typed relation, arguments or family-specific
  details needed to choose feedback. Available argument values may also carry
  variable references.

Use those fields to highlight delivery 37 and choose wording for its deadline
or finish-domain finding. Labels and paths provide additional context. Avoid
parsing the checker's text messages to recover IDs or values.

For an array annotated `checker_value("start", [37, 52])`, positions 1 and 2 map
to the corresponding entity IDs by application convention. Constraint payloads
can name several affected entities. The
[`schedule-feedback` example](../crates/zincite-check/examples/schedule-feedback.rs)
shows both mappings in Rust. Compiler copies of value annotations on constraints
use `metadata.value_application`, independently of `metadata.application`.

Several flattened constraints may share one source ID or label. Process their
individual findings before choosing how to group them in the UI. Constraint
ordinals are stable only within one prepared model; recompiling requires checking
the application's name and entity mapping again.

## Export a model for deployment

Preparation resolves declarations, validates supported operations and selects
usable definition directions. Export its result once, then ship the artifact
to native or browser applications:

```sh
zincite-check --export delivery.prepared.json delivery.fzn
zincite-check --load --format json delivery.prepared.json candidate.dzn
zincite check --load delivery.prepared.json candidate.dzn
```

Export creates a new file and refuses an existing path. Loading validates the
artifact before exposing a checkable model. It requires neither the source text
nor a MiniZinc installation. Rust callers that only load prepared artifacts can
set `default-features = false` on their dependency to exclude source parsing and
the CLI, then construct `Assignment` values directly.

There are three different JSON formats at this boundary:

| Format | Public entry point | Integer representation |
| --- | --- | --- |
| Official compiler FlatZinc-JSON, version 1.0 | `prepare_json(name, source)` or `--input-format fzn-json` | JSON number tokens, read exactly as i64 in integer roles |
| Zincite prepared model, `format_version: 1` | `PreparedModel::from_json` or `--load` | Decimal strings |
| Typed assignment and report JSON | Wasm `check`, shared Rust records, CLI report output | Decimal strings |

Official compiler JSON is source input. It is not a prepared artifact. For
example, this uses the saved compiler output in the repository:

```sh
zincite-check --input-format fzn-json --format json examples/checker-compilation/model.fzn.json examples/checker-compilation/candidates/feasible.dzn
zincite check --input-format fzn-json --export compiler.prepared.json examples/checker-compilation/model.fzn.json
zincite-check --load compiler.prepared.json examples/checker-compilation/candidates/feasible.dzn
```

Keep compiler JSON as text when passing it to `prepare_json`; parsing and
rewriting it through JavaScript `Number` first can round large integer tokens.
Zincite reads those tokens directly. It also accepts the specified `rhs` field
for fixed values and aliases. The
[FlatZinc-JSON reference](../crates/zincite-check/README.md#prepare-official-compiler-json)
describes contextual array types and admission limits. In particular, listing
an all-literal array in JSON `output` makes that array assignable; its name then
cannot serve as a fixed coefficient array. That is a current preparation
boundary, not a demonstrated defect in MiniZinc's emitted output.

## Use the parser-free Wasm runtime

Build the `wasm` feature with `--no-default-features` and generate bindings using
the [Wasm build instructions](../examples/checker-wasm/README.md#build-the-bindings).
Those instructions pin the matching wasm-bindgen library and CLI versions and
produce Node and browser modules. Export `delivery.prepared.json` as above.
With the Node bindings at `target/checker-wasm/node`, run this from the repository
root:

```sh
node <<'JS'
const { readFileSync } = require("node:fs");
const { PreparedModel } = require("./target/checker-wasm/node/zincite_check.js");
const model = new PreparedModel(readFileSync("delivery.prepared.json", "utf8"));
const assignment = (start) => ({ entries: [{
    name: "start",
    value: { kind: "scalar", value: { kind: "integer", value: start } }
}] });
try {
    for (const input of [assignment("1"), assignment("8"), { entries: [] }, assignment("1")]) {
        const report = JSON.parse(model.check(JSON.stringify(input)));
        console.log(report.status, report.completeness.complete);
    }
} finally {
    model.free();
}
JS
```

The outputs are `valid true`, `invalid true`, `incomplete false` and `valid true`.
The file read belongs to the caller; the Wasm runtime only receives strings.
Browser applications initialize the generated ES module and load the artifact
before editing, as shown in the [browser example](../examples/checker-browser/README.md).

Keep mathematical integers as decimal strings, including application entity
IDs, set endpoints and report values. For example, `"9007199254740993"` is exact;
its JavaScript `Number` conversion is not. Use `BigInt` if application arithmetic
needs that range, and convert back to a decimal string before serialization.
Booleans are JSON Booleans, floats are finite JSON numbers, and structural array
positions and source offsets are JSON numbers. The
[typed assignment reference](../examples/checker-wasm/README.md#load-and-check)
gives array and integer-set forms. An array assignment supplies all its entries;
to omit values, leave out their assignments instead of sending null placeholders.

A successful `check` returns report JSON even for invalid or incomplete candidates.
Malformed artifacts and assignments, or execution errors, throw JavaScript
strings. Loader diagnostics and checker errors are JSON-encoded records;
malformed assignment JSON and invalid tolerance settings produce plain text.
Handle that error boundary separately from normal report presentation. Release
the model with `free()` when the application no longer uses it.

## Choose completion and numeric policies deliberately

`defines_var(finish)` selects a deterministic output direction for its relation.
The declaration marker `is_defined_var` is informational and cannot create a
definition on its own. Every defining relation is also checked. Unsupported
inverse directions, missing inputs or cycles without enough values leave outputs
unresolved. The checker does not choose a start time merely because some finish
time would satisfy the deadline.

Fixed initializers supply exact values, including for defined outputs. Logical
singleton domains may supply defaults for other variables, but a defined output
does not fall back to its singleton domain when its defining inputs are missing.
A supplied value takes precedence and is checked against its fixed value or
logical domain. `computed_domain` marks a solver convenience domain: it supplies
neither a domain violation nor a default. Logical domains on aliases still apply.
These rules also hold after validated loading.

Integers use checked i64 arithmetic on native and Wasm targets. Integer sets use
compact inclusive ranges. Floats are finite binary64 values; each prepared model
owns an absolute and relative tolerance, both `1e-9` by default. Zero for both
gives exact comparison. Tolerance changes comparisons, including float domain
membership, but does not fill missing values or relax exact alias identity.

Rust callers use `FloatTolerance::new(absolute, relative)` and
`model.set_float_tolerance(...)`. Wasm callers use
`model.setFloatTolerance(absolute, relative)` and can inspect the JSON returned
by `model.floatTolerance()`. The CLI options are `--float-absolute-tolerance` and
`--float-relative-tolerance`; each changes only its named component. Export saves
both settings. Read the [float policy](../crates/zincite-check/README.md#float-policy)
for the comparison formula, strict inequalities, rounding and transcendental
behavior before relying on approximate equality.

## Compile the application's MiniZinc model offline

The delivery example above is authored FlatZinc so its completion and feedback
are easy to inspect. For a MiniZinc application model, compile outside the
checker using one of the exact recipes in the reference:

- The [standard-library profile](../crates/zincite-check/README.md#standard-library-compilation)
  uses MiniZinc 2.10.1 standard decompositions and Zincite's primitive declarations.
- The [pinned Gecode profile](../crates/zincite-check/README.md#pinned-gecode-compilation)
  selects Gecode commit `186551447db0f74639d65cb8de5af190f15ac2c2` with
  MiniZinc 2.10.1. Selecting an installed Gecode backend alone does not select
  this pinned library.

The recipes include paths, application annotations and symmetry handling, and
use `--compile` without running a solver. Add `--fzn-format json` to produce
official compiler JSON. The [MiniZinc 2.10.1 interface specification](https://docs.minizinc.org/en/2.10.1/fzn-spec.html)
defines the source formats and library interface.

Inspect the generated model before connecting an application's editor to it.
Even at `-O0`, MiniZinc can tighten domains, eliminate names, merge constraints
and drop application metadata or definition hints. A supported relation may
therefore require a supplied auxiliary value if compilation supplies no usable
definition. A complete valid report establishes feasibility of the admitted
compiled model; equivalence to the editable source model depends on these
compiler transformations.

The reference documents those limits with saved controls and separates
historical installed-backend examples from the two pinned profiles. External
executable/DLL and search-history hooks are diagnosed because their behavior
requires execution or search state. Recognized search and propagation metadata
does not cause value selection or alter the checked relation.
