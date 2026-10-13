# Edit a data instance

Query transformations produce a complete data candidate. Preview it before an
explicit write: the edit changes instance meaning, and successful parsing checks
syntax only. The [query reference](reference.md#assignment-edits) defines byte
preservation, comment handling and candidate validation.

## Replace an assignment expression

Create a small data file:

```sh
printf 'capacity = 20;\nweights = [1, -2, 3];\nunused = 0;\n' > data.dzn
zincite query --diff 'filter(name("capacity")) | set_value("30")' data.dzn
zincite-query 'filter(name("capacity")) | set_value("10 + 20")' data.dzn
```

`set_value` inserts the supplied expression as the selected assignment's
right-hand side. It does not evaluate the expression or create a missing
assignment. Default editing prints the candidate; `--diff` prints a preview.
Both leave the original file unchanged. Inspect the candidate, then apply it:

```sh
zincite query --write 'filter(name("capacity")) | set_value("30")' data.dzn
```

## Filter literal elements or remove an assignment

```sh
zincite query --diff 'filter(name("weights")) | filter_elements(gt(0))' data.dzn
zincite query --diff 'filter(name("unused")) | remove' data.dzn
```

The reference describes [literal inspection and filtering](reference.md#query-library),
including supported comparisons and collection shapes. Computed expressions
are not arbitrary evaluated values. Removal includes attached comments; separated
section comments remain.

## Reduce enum membership

An enum with explicit member-keyed data can be reduced without a model:

```sh
printf 'Guests = {A, B, C};\n' > guests.dzn
zincite query --diff 'reduce_enum("Guests", keep("A", "C"))' guests.dzn
```

Supply `--model model.mzn` when positional array alignment needs declarations.
Read the [enum reduction contract](reference.md#enum-reduction) before reducing
dependent arrays, records or nested sets. Incomplete candidates retain located
diagnostics and exit 1; `--write` refuses them. A smaller enum alone does not
prove that all dependent data has been reduced.

Write restrictions and failure statuses are in the [command reference](cli.md).
The [support guide](support.md) describes the semantic boundaries.

## Construct new data

An edit program evaluates bounded expressions and validates their constructed
values against read-only model declarations. Save this model as `counts.mzn`:

```minizinc
array[1..2] of int: counts;
array[1..2] of int: doubled_counts;
solve satisfy;
```

Start with `counts = [3, 5];` in `counts.dzn`, and save this as `create.zq`:

```text
source old_counts = literal("counts");
create doubled_counts = [old_counts[i] * 2 | i in index_set(old_counts)];
emit_document;
```

Both command forms produce the same candidate, adding
`doubled_counts = [6, 10];`:

```sh
zincite query --model counts.mzn -f create.zq counts.dzn
zincite-query --model counts.mzn --diff -f create.zq counts.dzn
zincite query --model counts.mzn --write -f create.zq counts.dzn
```

Supply `--stdlib-dir DIR` or `MZN_STDLIB_DIR` when model loading needs an
explicit library location. Missing model context or target identity causes a
located refusal. Creation requires that the assignment is absent. To start a
new data document, create an empty `.dzn` file and use a source-free program:

```text
create counts = [i * 2 | i in 1..2];
create doubled_counts = [i * 4 | i in 1..2];
emit_document;
```

Created assignments use the input's line ending and appear after the last
assignment's attached trailing comment, before detached trailing comments.
A BOM and unrelated bytes remain intact. Creation refuses any placement that
would change an existing next-item directive's target.

## Replace constructed values

Save this as `replace.zq` to replace `counts` with `[4, 6]`:

```text
source old_counts = literal("counts");
let saved = old_counts;
replace counts = [saved[i] + 1 | i in index_set(saved)];
emit_document;
```

Run `zincite-query --model counts.mzn --diff -f replace.zq counts.dzn`, then
use `--write` for an explicit replacement. Unlike `set_value`, this computes a
value. The same constructor handles records, tuples, sets and rectangular keyed
arrays or matrices. A replacement requires exactly one original assignment and
changes only its RHS range. Internal RHS comments need a placement decision,
so the request refuses them.

All sources and `let` bindings see the original input. Scalar edits supply
final axis bounds, but never become a later edit's source. Unique target names,
compatible types, written integer scalar domains, established nominal keys
and complete rectangular coverage are required. Unknown dependencies, conflicting targets, arithmetic errors or
unresolved comments refuse the entire batch without partial output or writes.

The library exposes the same candidate through `Program`:

```rust
use zincite_query::{Input, Program, ProgramLimits};
use zincite_lint::ModelContext;

fn preview(input: &Input, model: &ModelContext) -> Result<Vec<u8>, zincite_query::QueryError> {
    let limits = ProgramLimits::default();
    let program = Program::parse(
        "create counts = [i * 2 | i in 1..2]; emit_document;",
        limits,
    )?;
    let mut result = program.evaluate_with_model(input, Some(model), limits)?;
    assert!(result.candidate().is_some());
    result.render()
}
```

Evaluation and rendering prepare bytes; the library performs no filesystem write.
In-place CLI writes additionally require a complete result and unchanged original
file bytes.

## Double entities and their dependent data

The [complete public example](query-comprehension-contract.md#a-complete-doubled-instance)
provides `doubled.mzn`, `doubled.dzn` and a five-replacement program. The same
files are retained under `crates/zincite-query/tests/fixtures/` as
`doubled.mzn`, `doubled.dzn` and `double.zq`. From the repository root:

```sh
zincite query --model crates/zincite-query/tests/fixtures/doubled.mzn \
  -f crates/zincite-query/tests/fixtures/double.zq \
  crates/zincite-query/tests/fixtures/doubled.dzn
zincite-query --diff --model crates/zincite-query/tests/fixtures/doubled.mzn \
  -f crates/zincite-query/tests/fixtures/double.zq \
  crates/zincite-query/tests/fixtures/doubled.dzn
```

The program explicitly maps two original entities to four and one group to two.
It retains weights, remaps peers to `2, 1, 4, 3` and groups to `1, 1, 2, 2`,
copies the two-column capacity rows, and maps group membership to `{1,2}` and
`{3,4}`. Changing only a count would leave known array coverage stale and is
refused. Cross-copy relationships and enum member mappings require explicit
caller choices. The program never adds enum members or edits model/include
files. Syntax and bounded shape checks do not prove satisfiability.
