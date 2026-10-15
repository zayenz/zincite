# Edit a data instance

Use data edits to change a parameter, remove unwanted literal entries or make
a smaller instance for investigation. By default, Zincite prints the proposed
data file without changing the original. Preview that result before writing it;
a syntactically valid edit can still change whether the model has a solution.
The [query reference](reference.md#assignment-edits) defines byte
preservation, comment handling and candidate validation.

## Replace an assignment expression

Create a small data file:

```sh
printf 'capacity = 20;\nweights = [1, -2, 3];\nunused = 0;\n' > data.dzn
zincite query --diff 'filter(name("capacity")) | set_value("30")' data.dzn
zincite query 'filter(name("capacity")) | set_value("10 + 20")' data.dzn
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

The first command keeps `1` and `3` in `weights`; the second removes the
`unused` assignment. Both are previews, so each starts from the unchanged file.
Filtering compares written literal elements. It cannot evaluate an arbitrary
MiniZinc expression to decide whether to keep it. Removal includes comments
attached to the removed entry or assignment; separated section comments remain.
See [literal filtering](reference.md#assignment-edits) for supported shapes.

## Reduce enum membership

To keep only guests A and C, create this small input:

```sh
printf 'Guests = {A, B, C};\nscores = [A: 2, B: 5, C: 3];\n' > guests.dzn
zincite query --diff 'reduce_enum("Guests", keep("A", "C"))' guests.dzn
```

The candidate keeps `Guests = {A, C}` and the score entries `A: 2` and `C: 3`.
Their explicit keys let Zincite associate them with the enum without a model.
A positional array such as `[2, 5, 3]` needs model declarations to establish
which member belongs to each position; supply `--model model.mzn` for that
case. Read the [enum reduction contract](reference.md#enum-reduction) before
reducing dependent arrays, records or nested sets. Incomplete candidates retain located
diagnostics and exit 1; `--write` refuses them. A smaller enum alone does not
prove that all dependent data has been reduced.

Write restrictions and failure statuses are in the [command reference](cli.md).
The [support guide](support.md) describes the semantic boundaries.

## Construct new data

To add a calculated assignment, use an edit program with model declarations.
The model supplies the expected types and indices; the data file supplies the
values. Save this model as `counts.mzn`:

```minizinc
array[1..2] of int: counts;
array[1..2] of int: doubled_counts;
solve satisfy;
```

Start with `counts = [3, 5];` in `counts.dzn`, and save this as `create.zq`:

```zirql
source old_counts = literal("counts");
create doubled_counts = [old_counts[i] * 2 | i in index_set(old_counts)];
emit;
```

Preview the result, then its diff:

```sh
zincite query --model counts.mzn -f create.zq counts.dzn
zincite query --model counts.mzn --diff -f create.zq counts.dzn
```

The candidate adds `doubled_counts = [6, 10];` and leaves `counts` unchanged.
To apply it, run:

```sh
zincite query --model counts.mzn --write -f create.zq counts.dzn
```

Supply `--stdlib-dir DIR` or `MZN_STDLIB_DIR` when model loading needs an
explicit library location. Missing model context or target identity causes a
located refusal. Creation requires that the assignment is absent. To start a
new data document, create an empty `.dzn` file and use a source-free program:

```zirql
create counts = [i * 2 | i in 1..2];
create doubled_counts = [i * 4 | i in 1..2];
emit;
```

Created assignments use the input's line ending and appear after the last
assignment's attached trailing comment, before detached trailing comments.
A BOM and unrelated bytes remain intact. Creation refuses any placement that
would change an existing next-item directive's target.

## Replace constructed values

Continue with `counts.dzn`, whose original `counts` are still `[3, 5]`.
Save this as `replace.zq` to add one to each count:

```zirql
source old_counts = literal("counts");
replace counts = [old_counts[i] + 1 | i in index_set(old_counts)];
emit;
```

Run `zincite query --model counts.mzn --diff -f replace.zq counts.dzn`, then
use `--write` for an explicit replacement. Unlike `set_value`, this computes a
value. The same constructor handles records, tuples, sets and rectangular keyed
arrays or matrices. A replacement requires exactly one original assignment
and changes only its right-hand side. If that expression contains comments, Zincite refuses the
replacement rather than choosing where those comments should move.

Every `source` and `let` reads the original input. In this example, replacing
`counts` does not recalculate `doubled_counts`: it remains `[6, 10]` unless
you explicitly replace it too. Edits in one program do not read each other's
results.

Zincite checks target types, known scalar domains and array coverage against
the final set of edits. Conflicting targets, arithmetic errors or dependencies
it cannot resolve stop the whole batch without a partial write. A changed
scalar bound can therefore require changes to arrays indexed by that bound.

For Rust use, the [program reference](reference.md#program-invocation-and-rust-results)
describes `Program`, candidate inspection and rendering. Evaluation prepares
bytes without writing files.

## Double entities and their dependent data

The [complete public example](query-comprehension-contract.md#a-complete-doubled-instance)
provides `doubled.mzn`, `doubled.dzn` and a five-replacement program. The same
files are retained under `crates/zincite-query/tests/fixtures/` as
`doubled.mzn`, `doubled.dzn` and `double.zq`. From the repository root:

```sh
zincite query --model crates/zincite-query/tests/fixtures/doubled.mzn \
  -f crates/zincite-query/tests/fixtures/double.zq \
  crates/zincite-query/tests/fixtures/doubled.dzn
zincite query --diff --model crates/zincite-query/tests/fixtures/doubled.mzn \
  -f crates/zincite-query/tests/fixtures/double.zq \
  crates/zincite-query/tests/fixtures/doubled.dzn
```

This larger example copies two entities into two independent pairs and copies
their group. It changes the counts, per-entity arrays and references together.
It retains weights, remaps peers to `2, 1, 4, 3` and groups to `1, 1, 2, 2`,
copies the two-column capacity rows, and maps group membership to `{1,2}` and
`{3,4}`. Changing only a count would leave the arrays with too few entries, so
Zincite refuses that edit. Cross-copy relationships and enum member mappings require explicit
caller choices. The program never adds enum members or edits model/include
files. Syntax and bounded shape checks do not prove satisfiability.
