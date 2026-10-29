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
[v0.5.0 checkout](getting-started.md#build-from-a-checkout) root:

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

Making a larger test instance often means copying entities and updating the
data that refers to them. As one example, consider a wedding seating problem:
guests are assigned to tables, some groups should sit together, and others
should sit apart. Each guest has a record with a name, gender and interests in
conversation topics. The data also gives the available tables and their capacities.

The query below copies guests and their groups, adds tables and repeats their
capacities. The factor includes the original: `2` doubles the instance, while
`1` preserves the original bytes. Each additional copy gets fresh guest
identifiers and references to guests from that copy.

These operations are available since Zincite 0.1.2. From the
[v0.5.0 checkout](getting-started.md#build-from-a-checkout) root, build both
command forms:

```sh
cargo build --release -p zincite -p zincite-query
```

Configure the [MiniZinc standard library](queries.md#inspect-semantic-facts) for
model-backed queries. The commands below use the checkout binaries and the
retained files under `crates/zincite-query/examples/fixtures/`. The complete
files are also shown here so you can adapt them to your own data.

### Declare the input and supply a small instance

The model declares input types and a trivial solve item. It contains no seating
variables or constraints:

```minizinc title="wedding.mzn"
enum Topics;
enum Guests;
enum Genders = {Man, Woman, NA};
type GuestInfo = record(
    string: name,
    string: surname,
    Genders: gender,
    array[Topics] of int: interest
);
array[Guests] of GuestInfo: guests;
array[int] of set of Guests: same_table;
array[int] of set of Guests: different_tables;
enum Tables;
array[Tables] of int: table_capacities;
int: max_gender_imbalance_per_table;
solve satisfy;
```

`Guests` supplies the keys of `guests`. Each record's `interest` array has one
integer per member of `Topics`, in topic order. `same_table` and
`different_tables` contain sets of guests; their `array[int]` declarations allow
the number of groups to grow. `Tables` indexes `table_capacities`. The gender
imbalance limit is a separate scalar setting.

This synthetic input has two guests, two topics and two tables:

```minizinc title="wedding.dzn"
% unchanged heading
Topics = {Music, Food};
Guests = {A, B};
guests = [
  A: (name: "A", surname: "Example", gender: NA,
      % inside the record
      interest: [1, 2]),
  % between records
  B: (name: "B", surname: "Example", gender: Woman, interest: [2, 1])
];
same_table = [{A, B}];
different_tables = [{A}, {B}];
Tables = T(1..2);
table_capacities = [2, 3];
max_gender_imbalance_per_table = 1; % unchanged setting
```

The comments are intentional. Growing a source collection retains the original
comments and copies those inside it with the elements.

### Choose each edit in a query

Save this program as `grow-wedding.zq`, or use the
[saved query](https://github.com/zayenz/zincite/blob/v0.5.0/crates/zincite-query/examples/fixtures/grow-wedding.zq):

```zirql title="grow-wedding.zq"
param factor: int;
let additional = factor - 1;

let copies = items | filter(name("Guests")) | values
    | clone_members(additional, "GrownGuest");

append Guests = copies;
append guests = items | filter(name("guests")) | values
    | clone_elements(copies);
append same_table = items | filter(name("same_table")) | values
    | clone_elements(copies);
append different_tables = items | filter(name("different_tables")) | values
    | clone_elements(copies);
scale_enum Tables = factor;
append table_capacities = items | filter(name("table_capacities")) | values
    | repeat_elements(additional);
emit;
```

`param factor: int;` requires a command-line value such as `--param factor=2`.
`additional` counts copies beyond the original. `items` selects the input
assignments, `filter(name("Guests"))` keeps the guest enum assignment, and
`values` reads its literal right-hand side. `clone_members` checks that exactly
one original assignment was selected and builds the fresh member mappings.

These native copy stages take `values` directly. Do not add `one` here.
In the earlier constructed-data examples, `one` extracts a value for arithmetic
or a comprehension; copying instead needs the original literal selection,
including its source text and model type. The saved `copies` value is an opaque
member mapping consumed by `append` and `clone_elements`.

For factor `2`, the mapping is:

| Original guest | Additional copy |
| --- | --- |
| `A` | `GrownGuest_1_1` |
| `B` | `GrownGuest_1_2` |

The prefix is chosen in the query. The first number identifies the additional
copy and the second is the guest's position in the original enum; both start
at one. A collision with an existing input or model/include name, or another
name added by the batch, stops the request.

`append Guests = copies;` adds the new members. The next three appends copy
records and group sets with the same mapping. Only parsed guest references and
guest keys change. The strings `"A"` and `"B"`, record field labels, gender
members and interest values keep their original spelling. Comments keep their
text too, even if their prose describes the original instance.

`scale_enum Tables = factor;` changes `Tables = T(1..2)` to `Tables = T(1..4)`.
It changes only the literal upper bound. The final append repeats the capacities
in order, giving `[2, 3,2, 3]`. The factor-2 candidate therefore has four guest
records and four tables, with these guest and group assignments:

```minizinc
Guests = {A, B,GrownGuest_1_1, GrownGuest_1_2};
same_table = [{A, B},{GrownGuest_1_1, GrownGuest_1_2}];
different_tables = [{A}, {B},{GrownGuest_1_1}, {GrownGuest_1_2}];
```

Copies retain source spacing; Zincite adds separators without formatting the
whole collection.

Every target is explicit. Zincite does not discover which collections you want
to copy. Every read sees the original file, and the final `emit;` prepares one
complete candidate after checking the whole batch. The checks include unchanged
data assignments affected by the grown enums. Omitting the `guests` or
`table_capacities` append leaves too few entries and fails validation.

### Preview, save and write

Preview the changes from the checkout root:

```sh
target/release/zincite query \
  --model crates/zincite-query/examples/fixtures/wedding.mzn \
  --param factor=2 \
  --max-work 50000000 \
  --diff \
  -f crates/zincite-query/examples/fixtures/grow-wedding.zq \
  crates/zincite-query/examples/fixtures/wedding.dzn
```

Omit `--diff` to print the complete candidate, including unchanged assignments:

```sh
target/release/zincite query \
  --model crates/zincite-query/examples/fixtures/wedding.mzn \
  --param factor=2 \
  --max-work 50000000 \
  -f crates/zincite-query/examples/fixtures/grow-wedding.zq \
  crates/zincite-query/examples/fixtures/wedding.dzn
```

Both commands leave the input untouched. Save a separate candidate by redirecting
standard output. The standalone command accepts the same options:

```sh
target/release/zincite-query \
  --model crates/zincite-query/examples/fixtures/wedding.mzn \
  --param factor=2 \
  --max-work 50000000 \
  -f crates/zincite-query/examples/fixtures/grow-wedding.zq \
  crates/zincite-query/examples/fixtures/wedding.dzn \
  > /tmp/wedding-grown.dzn
```

Use a separate output path: shell redirection opens that path before Zincite
reads its input. Check a successful candidate against the supplied declarations:

```sh
minizinc --instance-check-only \
  crates/zincite-query/examples/fixtures/wedding.mzn \
  /tmp/wedding-grown.dzn
```

To try in-place editing without changing the fixture, copy it first:

```sh
cp crates/zincite-query/examples/fixtures/wedding.dzn /tmp/wedding-to-grow.dzn
target/release/zincite query \
  --model crates/zincite-query/examples/fixtures/wedding.mzn \
  --param factor=2 \
  --max-work 50000000 \
  --write \
  -f crates/zincite-query/examples/fixtures/grow-wedding.zq \
  /tmp/wedding-to-grow.dzn
```

`--write` prints no candidate. It checks that the input still matches the read
snapshot, preserves its permissions and replaces the file atomically after
validation. It refuses stdin and symlink targets. The model and query remain
read-only inputs. Re-running the growth program reads the file's current contents;
the fixed prefix can then collide with names from a previous run.

### Supported data and limits

Member cloning requires a nonempty set of distinct explicit enum members and a
unique uninitialized enum declaration in the model. Element copying accepts
literal sets and rank-one positional or fully keyed arrays. Their elements can
contain records, tuples, sets and nested rank-one arrays. Model types establish
which references belong to the copied enum. Enum-valued elements must use the
full enum domain, directly or through a type alias; enum subset domains are
refused, including nested subsets. Repetition does not reindex explicit keys,
so repeating keyed entries can produce a duplicate-key error.

`scale_enum` accepts only a unique model-backed enum assignment of the form
`C(1..N)`, with a positive literal integer `N` and a positive integer factor.
Multiplication is checked for overflow. Dependent positional arrays are checked
against the final cardinality. Constructor-member values or explicit keys such
as `T(1)` elsewhere in the data are unsupported. The operation does not evaluate
arbitrary constructor arguments or computed bounds.

Computed data, optional types, matrices, starting-key arrays and unavailable or
ambiguous domains are refused by these copy edits. Copying formatter or lint
directives is also refused. Original bytes outside the explicit edits remain
unchanged. Other supported `create` or `replace` edits can share the batch when
their types do not depend on a changed enum; each target can appear only once.

The query uses the normal program limits: 1,000,000 work units, 100,000 aggregate
result units, 100,000 entries in one stream and nesting depth 64 by default.
The commands raise the work allowance to 50,000,000 for model loading, copying
and final validation. Counts and factors have no separate example-specific cap;
expansion must fit the active limits. The [program limits](reference.md#program-limits)
describe the overrides and accounting.

Zincite validates and reparses the complete candidate before printing or writing
it. A collision, unsupported value, coverage failure or exceeded limit produces
a located diagnostic on stderr, exits `2` and leaves stdout empty and the input
unchanged. Successful growth exits `0`.

This transformation copies written data deterministically. Data validity does
not establish satisfiability: a model can impose constraints across all guests
and tables, so copied records and references do not make independent seating
problems. Neither Zincite nor the instance check solves the model.

The [source-copy reference](reference.md#source-copy-edits) defines the operations
in detail. For lower-level Rust use, the optional
[`grow-wedding` example](../crates/zincite-query/examples/grow-wedding.rs)
shows direct literal views and source-edit APIs. Its fixed wedding shape and
separate example limits do not define the query language's limits.
