# Edit a data instance

Use data edits to change a parameter, remove unwanted literal entries or make
a smaller instance for investigation. By default, Zincite prints the proposed
data file without changing the original. Preview that result before writing it;
a syntactically valid edit can still change whether the model has a solution.
The [query reference](reference.md#assignment-edits) defines byte
preservation, comment handling and candidate validation.

## Replace an assignment expression

Save these assignments as `data.dzn`:

```minizinc title="data.dzn"
capacity = 20;
weights = [1, -2, 3];
unused = 0;
```

Preview a replacement:

```sh
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

To keep only guests A and C, save this input as `guests.dzn`:

```minizinc title="guests.dzn"
Guests = {A, B, C};
scores = [A: 2, B: 5, C: 3];
```

Preview the reduction:

```sh
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

`create name = expression;` calculates a value and adds a new assignment to the
proposed `.dzn` file. The final `emit;` returns the complete proposed file,
including the original assignments. By default, the command prints that file
to standard output. It does not print a JSON report or write the input file.

The [staffing walkthrough](first-model.md#querying-staff-shortages) reports
shortages as JSON. Here, we want to store the daily shortfalls as a parameter
in a reusable data instance. Save these two files in a new directory:

```minizinc title="staffing.mzn"
array[1 .. 3] of int: required;
array[1 .. 3] of int: available;
array[1 .. 3] of int: shortfall;
solve satisfy;
```

```minizinc title="staffing.dzn"
required = [2, 3, 4];
available = [3, 4, 3];
```

The model declares the expected type and indices of `shortfall`; the
assignment is still missing from the data file. Zincite reads those declarations
to validate the edit. It does not run the model or a solver. Configure the
[MiniZinc standard library](queries.md#inspect-semantic-facts) before running
these model-backed edits.

Save the edit program as `add-shortfall.zq`:

```zirql title="add-shortfall.zq"
let required = items | filter(name("required")) | values | one;
let available = items | filter(name("available")) | values | one;
create shortfall = [
    if required[day] > available[day] then
        required[day] - available[day]
    else
        0
    endif
    | day in index_set(required)
];
emit;
```

The two `let` bindings read the original arrays. `create` adds an assignment
named `shortfall`, with zero where demand is covered and the difference where
it is not. `shortfall` must be declared in the model and absent from the data
file. Use `replace` for an assignment that already exists.

Run the program:

```sh
zincite query --model staffing.mzn -f add-shortfall.zq staffing.dzn
```

The command prints the complete candidate below and exits 0:

```minizinc title="Standard output"
required = [2, 3, 4];
available = [3, 4, 3];
shortfall = [0, 0, 1];
```

Day 3 needs one extra person. The original assignments remain in the output,
and the input file still contains only `required` and `available`. To save the
candidate as a separate data file, redirect the output:

```sh
zincite query --model staffing.mzn -f add-shortfall.zq staffing.dzn > staffing-with-shortfall.dzn
```

The next example uses `staffing-with-shortfall.dzn`. You can also preview a diff
or apply the edit directly to the input file:

```sh
zincite query --model staffing.mzn --diff -f add-shortfall.zq staffing.dzn
zincite query --model staffing.mzn --write -f add-shortfall.zq staffing.dzn
```

`--diff` prints the changes without writing. `--write` updates `staffing.dzn`
and prints no data file. The model and query files are read-only inputs.

## Replace constructed values

Now suppose one extra person is available on every day. Update both availability
and its calculated shortfall in `staffing-with-shortfall.dzn`. Save this program
as `add-staff.zq`:

```zirql title="add-staff.zq"
let available = items | filter(name("available")) | values | one;
let revised_available = [n + 1 | n in available];
let required = items | filter(name("required")) | values | one;
replace available = revised_available;
replace shortfall = [
    if required[day] > revised_available[day] then
        required[day] - revised_available[day]
    else
        0
    endif
    | day in index_set(required)
];
emit;
```

```sh
zincite query --model staffing.mzn -f add-staff.zq staffing-with-shortfall.dzn
```

The command prints:

```minizinc title="Standard output"
required = [2, 3, 4];
available = [4, 5, 4];
shortfall = [0, 0, 0];
```

`replace` calculates a new value for an existing assignment and changes its
right-hand side. Both replacements form one batch. Adding staff now covers
every day's demand.

The shortfall calculation uses `revised_available`, the value calculated in the
query. Reading `available` from the input again would still return `[3, 4, 3]`,
even after the `replace` statement. Every input read sees the original file;
edits do not read each other's results. Bind a calculated value and reuse it
when another edit depends on it.

As with `create`, add `--diff` to preview changes or `--write` to apply them.
A replacement requires exactly one original assignment. If its right-hand side
contains comments, Zincite refuses the replacement rather than choosing where
those comments should move.

Zincite checks target types, known scalar domains and array coverage against
the final set of edits. Conflicting targets, arithmetic errors or dependencies
it cannot resolve stop the whole batch without a partial write. A changed
scalar bound can therefore require changes to arrays indexed by that bound.
The [data-expression contract](query-comprehension-contract.md#report-expressions-and-edit-grammar)
lists the operations available in edits.

To start a new data document, supply an empty `.dzn` file and use `create` for
each assignment. The model must still declare the targets, and the program
must end with `emit;`.

For Rust use, the [program reference](reference.md#program-invocation-and-rust-results)
describes `Program`, candidate inspection and rendering. Evaluation prepares
bytes without writing files.

## Double entities and their dependent data

The [complete public example](query-comprehension-contract.md#a-complete-doubled-instance)
provides `doubled.mzn`, `doubled.dzn` and a five-replacement program. The same
files are retained under `crates/zincite-query/tests/fixtures/` as
`doubled.mzn`, `doubled.dzn` and `double.zq`. From the repository root:

```sh
zincite query --max-work 5000000 --model crates/zincite-query/tests/fixtures/doubled.mzn \
  -f crates/zincite-query/tests/fixtures/double.zq \
  crates/zincite-query/tests/fixtures/doubled.dzn
zincite query --diff --max-work 5000000 --model crates/zincite-query/tests/fixtures/doubled.mzn \
  -f crates/zincite-query/tests/fixtures/double.zq \
  crates/zincite-query/tests/fixtures/doubled.dzn
```

The higher work allowance covers loading the standard library and checking the
dependent assignments. This example copies two entities into two independent
pairs and copies their group. It changes the counts, per-entity arrays and references together.
It retains weights, remaps peers to `2, 1, 4, 3` and groups to `1, 1, 2, 2`,
copies the two-column capacity rows, and maps group membership to `{1,2}` and
`{3,4}`. Changing only a count would leave the arrays with too few entries, so
Zincite refuses that edit. The program must specify any relationships between
copies and any enum member mappings. It never adds enum members or edits model
or include files. Syntax and bounded shape checks do not prove satisfiability.
