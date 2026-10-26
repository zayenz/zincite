# Prepared models in WebAssembly

The `wasm` feature exposes the native loader and checker through `wasm-bindgen`.
Build with default features disabled to exclude source preparation and the native
CLI. The Wasm runtime takes strings and performs no filesystem, compiler, solver
or network operations. The caller loads the prepared artifact and supplies it.

## Build the bindings

From the repository root, with stable Rust and Node.js installed:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo build --locked -p zincite-check --lib --target wasm32-unknown-unknown --no-default-features --features wasm --release
wasm-bindgen --target nodejs --out-dir target/checker-wasm/node target/wasm32-unknown-unknown/release/zincite_check.wasm
wasm-bindgen --target web --out-dir target/checker-wasm/web target/wasm32-unknown-unknown/release/zincite_check.wasm
cargo tree --locked -p zincite-check --target wasm32-unknown-unknown --no-default-features --features wasm --edges normal,build
cargo tree --locked -p zincite-check --target wasm32-unknown-unknown --no-default-features --features wasm --edges features
```

The binding library and CLI must use the same version, currently 0.2.129. The
dependency tree includes Serde, generic Rust libm 0.2.16 and binding support; it
excludes `zincite-syntax` and the formatter, linter and query crates. The package builds an `rlib` for Rust
callers and a `cdylib` for the Wasm bindings. Native builds use the default
`prepare` feature and keep both CLI entry points.

The [official CLI guide](https://wasm-bindgen.github.io/wasm-bindgen/reference/cli.html)
describes binding generation. Its
[deployment guide](https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html)
documents `nodejs` CommonJS output and `web` browser ES modules. No bundler or
frontend framework is needed. These commands were checked with Rust 1.98.1,
wasm-bindgen 0.2.129 and Node.js 24.22.1.

## Load and check

Node.js uses the generated CommonJS module:

```js
const { PreparedModel } = require("./target/checker-wasm/node/zincite_check.js");
const model = new PreparedModel(preparedJson);
try {
    const assignment = { entries: [{
        name: "start",
        value: { kind: "array", value: [
            { kind: "integer", value: "1" },
            { kind: "integer", value: "4" }
        ] }
    }] };
    const report = JSON.parse(model.check(JSON.stringify(assignment)));
    console.log(report.status, report.completeness);
} finally {
    model.free();
}
```

`preparedJson` is the unmodified text from a native `--export` artifact. The
constructor validates it through `PreparedModel::from_json`. Each `check` call
uses the shared `PreparedModel::check` and returns the native `CheckReport` JSON.
The same instance supports repeated edits, including removed assignments.

Assignments have an `entries` array, including `[]` for no supplied values. Each
entry has a `name` and a `value`. Array values use the shape above; scalars use
`{ kind: "scalar", value: { kind: "integer", value: "9007199254740993" } }`
or `{ kind: "scalar", value: { kind: "boolean", value: true } }`. Integer sets use
`{ kind: "scalar", value: { kind: "integer_set", value: { ranges:
[{ lower: "9007199254740993", upper: "9007199254740993" }] } } }`.
Empty sets have `ranges: []`. Ranges must be sorted, disjoint, nonadjacent and
nonempty; the loader and assignment reader reject other representations.
Array entries can use these same set values. The optional
entry `location` accepts the shared location record or `null`; ordinary callers
omit it. Entries preserve order and duplicates for the checker's validation.

Every mathematical integer in assignments, artifacts and reports is a decimal
string in the signed 64-bit range. JSON numbers for integer values are rejected.
Keep those strings, or convert them to JavaScript `BigInt` for application
calculations; converting them to `Number` can lose precision. Booleans use JSON
Booleans. Floats use finite JSON numbers: `{ kind: "float", value: 1.25 }`.
The model saves both tolerance settings; `model.floatTolerance()` returns their JSON
and `model.setFloatTolerance(absolute,relative)` validates and changes them.
Defaults are `1e-9` each; zero/zero gives exact checking. Search precision is separate.
Source offsets, array positions and ordinals use JSON numbers.

Malformed artifact or assignment input and checking errors throw JavaScript
strings. Loader diagnostics and `CheckError` records use their shared JSON
records; malformed assignment JSON reports its parse error as text. Invalid or
incomplete candidates return ordinary reports. Applications can use
`violations[].check` and typed `details` with application IDs and entity payloads
to choose their feedback, and read completeness independently.

For a browser, serve the generated `web` directory with its `.wasm` file and
initialize the ES module before constructing a model:

```js
import init, { PreparedModel } from "./web/zincite_check.js";
await init();
const preparedJson = await (await fetch("./prepared.json")).text();
const model = new PreparedModel(preparedJson);
// Use model.check(...) as above; call model.free() when finished.
```

The browser caller controls loading and presentation. The
[local workshop schedule](../checker-browser/README.md) demonstrates graphical
edits and job feedback using these bindings and the saved compiled model.

## Execute the native/Wasm comparison

Build the bindings above, then export seven artifacts with the native CLI. This
setup uses both saved compiled workshop models and a tiny handwritten FlatZinc
integer/set model, the saved float-core control, a direct selection control,
a direct set control, and a float-function control.
This setup does not run MiniZinc:

```sh
cargo build --locked -p zincite-check --bin zincite-check
checker_run=$(mktemp -d "$PWD/target/checker-wasm/run.XXXXXX")
target/debug/zincite-check --export "$checker_run/schedule.json" examples/checker-compilation/model.fzn
cat > "$checker_run/exact.fzn" <<'FZN'
var int: x;
var set of int: exact_tools;
var int: incremented ::is_defined_var;
var bool: at_least ::is_defined_var;
var bool: opposite ::is_defined_var;
var bool: both ::is_defined_var;
var bool: clause_value ::is_defined_var;
var int: boolean_total ::is_defined_var;
var int: flag_int ::is_defined_var;
constraint int_lin_eq([1],[x],9007199254740993)::checker_constraint("exact",[9007199254740993]);
constraint set_eq(exact_tools,{9007199254740993});
constraint int_plus(x,1,incremented)::defines_var(incremented);
constraint int_pow_fixed(incremented,1,9007199254740994);
constraint fzn_int_ge_reif(x,9007199254740993,at_least)::defines_var(at_least);
constraint int_gt(x,9007199254740992);
constraint gecode_int_lin_lt_reif([1],[x],9007199254740994,at_least);
constraint int_ge_imp(x,9007199254740993,at_least);
constraint int_lin_eq_imp([2],[9223372036854775807],0,false);
constraint bool_not(at_least,opposite)::defines_var(opposite);
constraint bool_left_imp(opposite,at_least,both)::defines_var(both);
constraint array_bool_xor([at_least,opposite],true);
constraint bool_clause_imp([both],[at_least],clause_value)::defines_var(clause_value);
constraint bool_lin_eq([9007199254740993,-1],[at_least,opposite],boolean_total)::defines_var(boolean_total);
constraint bool2int(opposite,flag_int)::defines_var(flag_int);
constraint bool_eq_reif(at_least,opposite,false);
constraint bool_lin_ge([1],[both],flag_int);
solve minimize x;
FZN
target/debug/zincite-check --export "$checker_run/exact.json" "$checker_run/exact.fzn"
target/debug/zincite-check --export "$checker_run/bench.json" examples/checker-compilation/shared-bench.fzn
target/debug/zincite-check --float-absolute-tolerance 2e-9 --float-relative-tolerance 3e-9 --export "$checker_run/float.json" examples/checker-compilation/float-core.fzn
target/debug/zincite-check --float-absolute-tolerance 2e-6 --float-relative-tolerance 3e-6 --export "$checker_run/selection.json" examples/checker-compilation/selection.fzn
target/debug/zincite-check --export "$checker_run/sets.json" examples/checker-compilation/set-primitives.fzn
target/debug/zincite-check --float-absolute-tolerance 2e-9 --float-relative-tolerance 3e-9 --export "$checker_run/functions.json" examples/checker-compilation/float-functions.fzn
node examples/checker-wasm/smoke.cjs target/checker-wasm/node/zincite_check.js "$checker_run/schedule.json" "$checker_run/exact.json" "$checker_run/bench.json" "$checker_run/float.json" "$checker_run/selection.json" "$checker_run/sets.json" "$checker_run/functions.json" examples/checker-wasm/candidates.json target/debug/zincite-check
```

`smoke.cjs` reads only built bindings, prepared artifacts and typed assignment
data. It needs no model source or compiler. It loads each artifact once and
compares every observable report with the native CLI's `--load` path. The CLI
receives literal `.dzn` assignments made from the same typed values in a temporary
directory. The consumer removes those temporary assignments when finished.

The thirty-four named cases cover valid, invalid, incomplete and invalid-with-incomplete
reports, application metadata, Boolean and array input, set definitions and domain
failures, strict resource conflicts, and the distinct integers 9007199254740992
and 9007199254740993 as both integers and set endpoints. The exact-value model
also derives arithmetic and reified Boolean outputs and checks conflicting
supplied outputs above JavaScript's exact-number range. It compares scalar and
linear relation findings and skips an overflowing product under a false
implication guard. Boolean cases exercise negation, functional implication,
parity, clause equivalence, channeling and a Boolean linear result above 2^53.
Float cases compare saved, custom and strict policy, arithmetic/conversion
completion, malformed finite values/settings, repeated edits and removed inputs.
They compare typed conflicts and an incomplete candidate with a known channel
violation. Selection cases cover offset indexing, rectangular Boolean/set calls,
all four selected value types, unselected output aliases, typed conflicts and an
extreme invalid selector with an unavailable result. They use saved, custom and
strict float tolerance. Set cases compare difference and symmetric difference,
cardinality and reifier completion, sorted-element and native characteristic
order findings, supplied-output conflicts, and incomplete input with a known
violation. Their endpoints above 2^53 cross the same exact-string boundary.
Float-function cases compare square root, fixed integer and fixed-base powers,
inverse hyperbolic sine and both extrema conventions. They exercise saved, custom
and strict policy, typed supplied-output conflicts, missing input and a located
domain error against the native CLI diagnostic. A second pass exercises reuse
after changing and removing inputs. Small boundary checks also reject a
malformed artifact, a JSON number supplied as an integer and an unknown assignment
name. Expected output ends with `Native/Wasm reports and errors agree across 68 checks; finite floats/functions, saved/custom/strict tolerance, exact integers, arithmetic, relations, Booleans, sets, selections, global findings and input errors verified.` The native suite owns
the wider evaluator coverage.


For real-browser latency, serialized-load costs, JSON boundary costs and scaling,
see the [interactive checker report](../../scripts/checker-performance.md). Its
repeatable browser collection uses these release web bindings; Node is used only
for the correctness smoke above.
