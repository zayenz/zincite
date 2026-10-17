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
zincite query --diff 'filter(name("weights")) | filter_elements(value => value > 0)' data.dzn
zincite query --diff 'filter(name("unused")) | remove' data.dzn
```

The first command keeps `1` and `3` in `weights`; the second removes the
`unused` assignment. Both are previews, so each starts from the unchanged file.
Filtering compares written literal elements. It cannot evaluate an arbitrary
MiniZinc expression to decide whether to keep it. Removal includes comments
attached to the removed entry or assignment; separated section comments remain.
The implicit spelling is `filter_elements(_ > 0)`. These bodies use the
[data-edit expression subset](query-comprehension-contract.md#report-expressions-and-edit-grammar);
report-only calls and effects remain unavailable.
Direct `eq`, `lt`, `le`, `gt`, `ge` and `eq(member("A"))` comparisons also remain
available.

A program can supply visible Data bindings to one scalar edit pipeline:

```zirql
let cutoff = 0;
items | filter(name("weights")) | filter_elements(value => value > cutoff) | emit
```

It produces one complete candidate and cannot combine that pipeline with other
edits or report outputs.

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

`[n + 1 | n in available]` visits the stored values in order and adds one to
each, producing a positional array. The second comprehension visits array keys
with `index_set(required)` and reads each day's values with `[day]`. Its
`if ... then ... else ... endif` chooses the shortage or zero; the `else` is
required.

`replace` calculates a new value for an existing assignment and changes its
right-hand side. Both replacements form one batch. The extra staff cover
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
covers arithmetic, comprehension scope, array ordering and the operations an
edit allows. Read-only reports support more of the language. Zirql's
[value guide](https://zayenz.se/zirql/guides/values/) offers additional background
on those shared calculations.

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
`doubled.mzn`, `doubled.dzn` and `double.zq`. From the
[v0.1.0 checkout](getting-started.md#build-from-a-checkout) root:

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

## Grow a wedding instance

The [`grow-wedding` Rust example](../crates/zincite-query/examples/grow-wedding.rs)
handles the seating model's nominal guest identifiers and commented records.
It uses Zincite's public literal views and source-edit API. The ordinary query
language keeps its existing restrictions on adding enum members and replacing
commented values.

Run it from a source checkout, choosing a separate output file:

```sh
cargo run --quiet -p zincite-query --example grow-wedding -- \
  crates/zincite-query/examples/fixtures/wedding.dzn 2 > /tmp/wedding-grown.dzn
```

The [public input](../crates/zincite-query/examples/fixtures/wedding.dzn) has two
guests and two tables. Factor `2` produces four of each; factor `1` preserves
the original bytes. The factor is the total number of copies, including the
original. Original guest symbols stay unchanged. Additional copies use
`GrownGuest_1_1`, `GrownGuest_1_2`, and so on, with the copy number followed by
the guest's position in the original enum. A name collision stops the command.

Each copied guest keeps the original record, including display-name strings,
interest values and internal comments. Copies of `same_table` and
`different_tables` refer to guests from their own copy. Table capacities repeat
in their original order, and `Tables = T(1..N)` becomes `T(1..N*factor)` with
the calculated integer written in place. Unrelated assignment bytes and all
original comments remain unchanged. Comments inside copied collections are
copied too; prose in a comment is not updated to describe the larger instance.

The example accepts exactly these data assignments:

- `Topics` and `Guests`: nonempty literal sets of distinct enum member names.
- `guests`: an explicitly guest-keyed array covering `Guests` exactly once.
  Each record has string `name` and `surname`, `gender` equal to `Man`, `Woman`
  or `NA`, and a positional integer `interest` array with one value per topic.
- `same_table` and `different_tables`: positional arrays of literal guest sets.
- `Tables`: the constructor `T(1..N)` with a positive decimal integer `N`.
- `table_capacities`: a positional array of `N` literal integers.
- `max_gender_imbalance_per_table`: a literal integer.

Extra or repeated assignments, computed values, unknown group members and
unsupported shapes are refused. Member keys must be direct identifier tokens;
parenthesized keys and formatter directives are unsupported. The factor is
limited to `1..1000`, input to 1 MiB and output to 16 MiB. Original token count
times the factor must also fit 200,000 tokens; the literal reader retains its
normal query limits.

The command buffers and reparses the complete candidate before printing. It
has no in-place option. Invalid input or exceeded limits produce a diagnostic
on stderr, exit status `2` and no candidate on stdout. Check the candidate with
the matching model before using it. For the public control:

```sh
minizinc --instance-check-only \
  crates/zincite-query/examples/fixtures/wedding.mzn /tmp/wedding-grown.dzn
```

This grows the written data deterministically. It does not create random guests
or establish satisfiability: the seating model still chooses tables for all
copies together, and capacity, symmetry and objective constraints can couple
them. The [integer-indexed doubling example](#double-entities-and-their-dependent-data)
above remains the query-language recipe for explicitly mapped integer keys.
