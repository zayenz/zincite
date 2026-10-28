# Checker architecture and testing

`zincite-check` evaluates a supplied candidate against a compiled FlatZinc model.
An application can reuse the prepared model after each edit, including edits that
violate constraints or omit values. Each call derives the supported outputs,
checks the available domains and relations, and reports completeness separately
from violations.

Start with the [integration guide](checker.md) for application code and compiler
recipes. The [predicate reference](../crates/zincite-check/README.md) owns the
relation semantics and supported definition directions. The
[coverage inventory](checker-coverage.md) connects those operations to exact
upstream signatures and compiler include profiles. This guide explains where
those contracts live in the implementation and how to test a change.

## Code boundaries

All paths in this table are under `crates/zincite-check/src/`.

| Code | Responsibility |
| --- | --- |
| `prepare.rs` | Text FlatZinc admission, name and alias resolution, types, shapes, domains, fixed arguments, objectives and construction of the executable model. |
| `prepare/json.rs` | Official FlatZinc-JSON normalization for the same lowerer, with exact numeric tokens and locations in the original JSON. |
| `prepare/annotations.rs` | Annotation contexts, argument validation, definition targets and application metadata. |
| `lib.rs` | Public values and reports, canonical value slots, assignment validation, the definition worklist, domain checks and relation dispatch. |
| `arithmetic.rs`, `boolean.rs`, `relations.rs`, `floats.rs`, `sets.rs`, `selection.rs` | Scalar operations, numeric policy, compact sets and selected-value dependencies. |
| `counting.rs`, `ordering.rs`, `tables.rs`, `automata.rs`, `circuits.rs` | Concrete global relations and their supported outputs. |
| `scheduling.rs`, `packing.rs`, `rectangles.rs`, `set_globals.rs`, `set_channels.rs`, `number_theory.rs`, `distance.rs` | The other global families, including typed findings and family-specific admission checks. |
| `serialized.rs` | Versioned executable JSON, validation before use, and reconstruction of aliases and definition dependencies. |
| `cli.rs`, `wasm.rs` | Input/output adapters around the library. The standalone and root binaries use the same CLI. |

Family modules keep their exact names, fixed/variable argument roles and
mathematics together. Preparation and validated loading both use these contracts.
The runtime does not depend on lint facts or query execution. The application
mapping in the workshop example also sits outside it.

## Preparing an executable model

`prepare(name, source)` parses FlatZinc through `zincite-syntax` and lowers it to
private executable fields. Accepted MiniZinc syntax alone is insufficient:
FlatZinc item order, signatures, types, array shapes and annotation contexts must
also pass admission. Unknown predicates and malformed input produce located
`Diagnostic` records.

`prepare_json(name, json)` accepts official compiler JSON. It normalizes the
model into syntax consumed by the same lowerer and maps diagnostics back to the
original JSON. Raw numeric tokens avoid rounding integer literals through a
float. This route needs the `prepare` feature, just like text preparation.
Compiler JSON is distinct from a prepared artifact.

JSON array declarations do not specify element types. Preparation infers them
from literals and referenced declarations. An `output` entry or `defined: true`
flag can classify a literal array as assignable. Such an array can be rejected
in a fixed-parameter role: for example, a nonempty array of float literals used
by name as `float_lin_eq` coefficients. Empty arrays have no variable operands
to reject, and integer literal arrays requested as float arrays can be re-read
with float element types. For hand-authored JSON, put fixed coefficient literals
directly in the constraint argument. These are current admission rules; schema
validity alone does not establish acceptance.

The executable model has canonical `Slot`s for values and `Binding`s for names.
A scalar name or array element can alias another slot. Declaration references
keep their own domains and reporting metadata, so several references can produce
domain findings for the same value. `link_aliases` supplies the alias lists used
in reports. Array positions in these references are one-based.

## One check call

`PreparedModel::check(&assignment)` follows a direct sequence:

1. `assign` allocates fresh empty slots, validates supplied names and values, then
   fills unsupplied slots from their defaults.
2. `complete` runs the supported functional definitions until no additional
   values become available.
3. `check` records unresolved declaration references, checks each available
   domain and relation, and collects blocked checks.
4. It returns typed findings, independent completeness and any available
   objective value.

Neither supplied assignments nor the model's value state are changed by a check.
Removing an entry on the next call removes that supplied value; no derived value
from the preceding call survives. Reuse the prepared model, but build each
assignment from the current application state.

Assignment validation rejects unknown names, duplicate entries, wrong types or
array lengths, and contradictory values supplied through aliases. Alias identity
is exact, including for floats. Two different float values cannot be assigned to
one slot merely because a tolerant relation would consider them equal.
A correctly typed value outside a declared domain is an ordinary violation.

### Fixed values and singleton domains

A literal initializer supplies a known value represented by `Domain::Fixed`.
An ordinary singleton domain can also supply a default. These have different
roles when a slot is defined by a constraint: a literal fixed value can start the
definition calculation, while a singleton bound must not replace the calculated
output. The distinction applies across aliases of the canonical slot.

For example, this cycle has a fixed starting value:

```minizinc
var int: x :: is_defined_var = 4;
var int: y :: is_defined_var;
constraint int_plus(y, 1, x) :: defines_var(x);
constraint int_plus(x, -1, y) :: defines_var(y);
solve minimize y;
```

An empty assignment establishes `x = 4`, derives `y = 3`, and checks both
relations. Supplying `x = 5` takes precedence over the default, derives `y = 4`,
and reports the fixed-domain violation for `x`. The value is not clipped to 4.
Likewise, a derived result outside a logical domain is available to every
downstream definition and check.

`computed_domain` marks solver-convenience bounds. Such bounds provide neither
a domain check nor a singleton default. Logical domains on other aliases still
apply, and literal initializers still supply their exact value.

### Definition directions and dependencies

`is_defined_var` is a declaration hint. The constraint's `defines_var` target
selects a proposed output, and `definition_direction` determines whether the
checker supports calculating it. A definition annotation does not authorize
arbitrary inversion. For example, integer addition can calculate its result;
a supported linear output calculation also requires exact divisibility.
Full reification can define its Boolean result. An implication does not generally
supply that equivalence.

Preparation diagnoses missing targets and multiple defining annotations for one
canonical slot. A well-formed target with no supported calculation supplies no
derivation. It can still be assigned explicitly and its relation checked.
Ordinary self-dependencies and cycles without a supplied or fixed starting value
leave unresolved values. Supplying enough values can make a cycle fully checkable.

`complete` initially queues every definition. A successful derivation writes only
an empty slot and queues its dependents. The dependency lists include potential
inputs; the family calculation decides which values are actually needed. An
element operation needs the index and selected element, for example. An unknown
unselected element, even an occurrence of the output itself, need not block that
calculation. Unresolved declarations still contribute independently to report
completeness.

Every defining relation is also checked as a constraint. A supplied output is
never overwritten to make its definition true. This is essential for reporting
conflicts in a candidate that an application wants to keep editing.

## Available checks and report meaning

The checker evaluates a relation when its required operands are available.
Known inactive implications need no evaluation of their guarded relation. Some
families also identify faults using only the relevant known operands, such as an
out-of-range element index. Other global checks conservatively wait for all their
required values. A partial candidate can therefore contain a mathematically
entailed conflict that the checker does not yet report. Incompleteness is not a
claim that a satisfying completion exists.

`CheckReport.status` is `Invalid` whenever there is a violation, even when
`completeness.complete` is false. With no violations it is `Incomplete` if values
or checks are unresolved, and `Valid` otherwise. Available findings are sorted
by source location. The objective is reported only when its value is available;
it says nothing about optimality.

`ViolationKind` contains typed relation details. `CheckRef` identifies the
constraint or domain, with application identifiers, payloads and source context.
`Completeness` contains unresolved variable references and blocked checks with
missing references. The workshop maps these fields to jobs without parsing
rendered messages or reimplementing scheduling constraints. Several flattened
constraints may share an application ID, and each finding is useful feedback.

Preparation and assignment failures are errors rather than report statuses.
Checked arithmetic failures, including overflow, division by zero and nonfinite
results, are execution errors when the operation is evaluated. A blocked
operation does not necessarily expose an error yet. The CLI returns 0 for a
valid complete report or export, 1 for invalid/incomplete reports, and 2 for
errors. See the [command reference](cli.md#exit-codes).

## Loading and numeric boundaries

`to_json` exports format version 1 with executable values, operations and report
metadata. It contains no CST. `PreparedModel` cannot deserialize unchecked
executable fields directly; `from_json` uses a private transport type, validates
references, types, shapes, domains, defaults, predicate variants and objectives,
then rebuilds aliases, definition directions and dependents.

Loading also reconstructs literal fixed defaults for defined slots from their
`Domain::Fixed` declarations, including version-1 artifacts that omit those
defaults. A serialized default on a defined slot must match a fixed declaration.
It cannot create a new source of candidate values. Version-1 artifacts without
float settings use the documented defaults.

Mathematical integers use checked i64 arithmetic and decimal strings in prepared
JSON, typed assignment JSON and report JSON. This includes set endpoints and application
payloads above JavaScript's exact `Number` range. Indexes and source offsets use
JSON numbers. Float values are finite binary64 numbers with signed zero
normalized to positive zero. Compact integer sets use canonical ranges rather
than enumerating every member.

`FloatTolerance` validates finite nonnegative absolute and relative settings;
both default to `1e-9`, and zero/zero selects exact comparison. The policy belongs
to the model and governs float relations, calculated-output checks and domains.
Alias identity and default selection use exact values. The
[float policy](../crates/zincite-check/README.md#float-policy) specifies the
comparison formula, rounding and arithmetic boundaries.

With `--no-default-features`, the library excludes preparation and
`zincite-syntax`. The `wasm` feature exposes the same validated loader and checker
through string-based bindings. JavaScript callers send typed assignment JSON and
parse report JSON; integer values must not pass through `Number`. Invalid and
incomplete candidates return normal reports. The binding throws strings for
errors: loader/checker errors contain serialized diagnostic records, while
malformed assignment JSON produces parse-error text. The
[Wasm example](../examples/checker-wasm/README.md) documents construction,
tolerance setters and disposal.

## Compiler profiles and research context

Compilation is an offline application step. The standard profile combines
MiniZinc 2.10.1's decompositions with Zincite's primitive declarations and small
compatibility wrappers. The Gecode profile uses the pinned upstream library plus
its Zincite wrappers. An installed Gecode release is not a substitute for that
source revision. The exact pins and repeatable commands are in the
[integration guide](checker.md) and [coverage inventory](checker-coverage.md).

A compiler may tighten domains, eliminate editable variables or decompose a
source constraint into several checks. Inspect the emitted model and candidate
mapping even with `-O0`. The profile inventory distinguishes direct mathematical
operations from source wrappers that abort or fail compilation. In particular,
standard integer-table reification has a width limit, and some compiled controls
carry duplicate definition hints that the checker correctly rejects. Those
compiler limits provide no evidence about runtime relation correctness.

Gecode's posting and propagation can depend on declared domains. Zincite checks
the concrete relation documented in the predicate reference. For scheduling,
this includes the specified four-argument cumulatives convention and explicit-end
relations that use supplied endpoints. Separate constraints own end equations
and other caller obligations; the checker does not invent them from a posting
routine. Search and recognized propagation annotations are metadata. The constraint
markers `boundsZ`, `boundsR`, `boundsD` and `priority(k)` accept the standard
shapes; `priority` requires a fixed integer. Search metadata such as
`restart_geometric` resolves a fixed float base without executing search. External
executable/DLL calls and restart-history hooks receive exclusion diagnostics
because their required external execution or search state is absent.

Two papers help explain these boundaries. Monette, Flener and Pearson's
[auxiliary elimination paper (2015)](https://justinkennethpearson.github.io/files/pubs/papers/2015_cp.pdf)
assumes non-constraining domains for the auxiliaries it eliminates. For an
interactive checker, that is a reason to inspect domains and editable-variable
mapping after compilation. Knutar Lewander, Flener and Pearson's
[invariant graph paper (2025)](https://justinkennethpearson.github.io/files/pubs/papers/2025_jair.pdf)
separates valuation from neighbour selection and describes value-dependent
required inputs. That distinction helps explain selected-element completion and
why checking a candidate requires no search. Zincite uses a fresh sequential
worklist; it does not implement these papers' elimination or incremental
propagation algorithms.

## Testing a change

Tests use small models through public preparation, checking and loading APIs.
Choose cases where the mathematical outcomes differ: an empty table versus an
empty accepted word, tied indexes, absent tasks, zero-length intervals, offsets,
signed resources or a numeric boundary. Exact-name loops check routing and
signatures; they do not replace these semantic cases.

The existing test files give the main paths:

| Test file | Observable behavior |
| --- | --- |
| `tests/checking.rs` | Scalar and global relations, functional completion, fresh calls, typed findings and independent incompleteness. |
| `tests/grammar.rs` | Text admission, annotation contexts, fixed/variable roles and computed domains. |
| `tests/fzn_json.rs` | Compiler JSON normalization, exact values, source/JSON report agreement and malformed input. |
| `tests/serialized.rs` | Source/export/load report agreement and rejection of invalid executable data. |
| `tests/cli.rs` | Reports, statuses, exports, loading, settings and error output. |
| Root `tests/cli.rs` | Dispatch through `zincite check`. |

For completion changes, useful starting points in `tests/checking.rs` are
`definition_chains_use_computed_values_despite_singleton_aliases`,
`fixed_initializers_seed_defined_cycles_and_independent_checks`,
`cycles_need_supplied_values_and_do_not_hide_known_violations`, and
`selections_require_only_current_selected_values_and_keep_conflicts_and_domain_failures`.
They exercise the distinctions that a worklist or default change can erase.
Global-family tests combine real relation cases with changed/removed inputs and
supplied-output conflicts. Add to the relevant family rather than building a
second test framework.

Run the repository's Rust checks after code changes:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

When changing a family or compiler profile, use the existing inventory helper
with clean source trees at the documented pins and a freshly built checker:

```sh
cargo build --locked -p zincite-check --bin zincite-check
uv run scripts/checker-coverage.py target/checker-upstream/minizinc-2.10.1 target/checker-upstream/gecode-main --checker target/debug/zincite-check
```

The helper checks names, signatures, registration aliases and include closure.
It does not compile models or prove relation semantics. Compile the affected
source controls with the documented profile, inspect emitted operations and
metadata, then check known candidates against the intended source constraints.
A solver comparison can resolve a concrete semantic uncertainty; it is not a
substitute for the chosen concrete checking contract.

For a runtime, serialization or binding change, use the existing
[native/Wasm comparison](../examples/checker-wasm/README.md#execute-the-nativewasm-comparison).
It compares complete reports across reused models, removed assignments, exact
integers, float settings and malformed inputs. The
[browser workshop](../examples/checker-browser/README.md#try-the-edits) adds actual
edits, keyboard controls, feedback replacement and no edit-time network requests.
Native tests alone do not establish browser behavior.

The [2026-10-28 measurement report](../scripts/checker-performance.md) describes a
named build and workload. Its preparation, load, check and JSON timings are
separate, and browser timings exclude DOM rendering. It is historical evidence
for that build, not a timing claim for every later binary. Keep performance
measurement outside `cargo test`; investigate a measured problem before adding
incremental state to the checker.
