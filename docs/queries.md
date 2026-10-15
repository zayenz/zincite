# Inspect MiniZinc source

Use `zincite query` when you have a specific question about a model or its
data. You can select constraints, count written calls, follow declaration
uses or compute a report from literal data. Queries do not run a solver.

The [staffing walkthrough](first-model.md#querying-staff-shortages)
starts with a practical question: which day has too few staff, even though
the total availability looks sufficient? The examples below develop source
inspection and data reports in more detail.

Zincite uses [Zirql](https://zayenz.se/zirql/) for shared query evaluation.
Zincite supplies its own MiniZinc-oriented syntax and operations, including
source selections, model facts and literal data. Use this guide for commands
you can run in Zincite; the Zirql site explains the shared language and its
Rust embedding interfaces.

## Select items and calls

Save this small model as `model.mzn`. It shares a capacity across three
allocations:

```minizinc
int: capacity = 20;
array[1 .. 3] of var 0 .. capacity: allocated;
constraint sum(allocated) <= capacity;
solve satisfy;
```

Count its constraints, print them, or count the calls written inside them:

```sh
zincite query 'filter(kind("constraint")) | count' model.mzn
zincite query 'filter(kind("constraint"))' model.mzn
zincite query 'filter(kind("constraint")) | expressions | call_names | tally' model.mzn
```

The first command prints `1`. The second prints the constraint's source, and
the third prints `{"sum":1}`. Stages are separated by `|`; each works on the
previous stage's result. The standalone `zincite-query` command accepts the
same options. Syntax inspection needs no standard library.

The [feature-extraction cookbook](query-cookbook.md) develops these counts
and histograms and adds declaration types and literal shapes. The
[query reference](reference.md#query-library) defines stages, selection order
and result formats.

## Reuse results in a report

Save this as `report.zq`:

```zirql
let selected = filter(kind("constraint"));
let n = selected | count;
let histogram = selected | expressions | call_names | tally;
(count: n, histogram: histogram, selected: selected);
```

Run `zincite query -f report.zq model.mzn` or
`zincite-query --program-file report.zq model.mzn`. The JSON envelope contains
`complete`, `limitations` and a record with the native count, histogram and
selection. The selected nodes keep their file, byte range and exact source text.
No model facts are needed for this syntax report. Use `declarations`
in the input binding to select semantic declaration identities instead.

Use `let` for input values and calculations, including native query pipelines.
The final expression emits its result and may omit its semicolon; `report` is
optional. Statements run in order with immutable earlier bindings. Each starts from the
same input selection unless it names another native result. A later failure
leaves stdout empty. [Query programs](reference.md#query-programs) describes
emitters, the library result and aggregate limits.

The following examples build records and arrays from literal data. Their JSON
uses explicit type tags and array keys, so the tables in this guide show the
values rather than the full serialization. The
[result schema](query-comprehension-contract.md#constructed-shape-and-result)
describes how to read those reports from another program.

All examples here are read-only. To change a data file, use the
[data-edit guide](data-edits.md); editing supports a smaller set of expressions.

## Compare demand and capacity

Start by totalling a demand array. Create `data.dzn` with
`demand = [2, 3, 4];` and run:

```sh
zincite query --program 'let d = items | filter(name("demand")) | values | one; sum(d);' data.dzn
```

The result contains the integer `9`. To reuse the query, save the text inside
the quotes as `sum.zq` and run `zincite query -f sum.zq data.dzn`.

Now suppose demand may double. We want to compare the new demand with capacity
at each position, as well as in total. Replace `data.dzn` with:

```minizinc
demand = [2, 3, 4];
capacity = [6, 8, 5];
```

Save the following as `capacity.zq`. The factor is a local literal binding.

```zirql
let demand = items | filter(name("demand")) | values | one;
let capacity = items | filter(name("capacity")) | values | one;
let factor = 2;
let positive_factor = require(factor > 0, factor,
                              "factor must be positive");
let aligned_capacity = require(
  index_set(demand) = index_set(capacity),
  capacity,
  "demand and capacity must have the same index set"
);
let scaled = require(
  forall([d >= 0 | d in demand]) /\
  forall([c >= 0 | c in aligned_capacity]),
  [positive_factor * d | d in demand],
  "demand and capacity must be nonnegative"
);
let rows = [
  (index: i,
   demand: scaled[i],
   capacity: aligned_capacity[i],
   headroom: aligned_capacity[i] - scaled[i])
  | i in index_set(demand)
];
let total_demand = sum(scaled);
let total_capacity = sum(aligned_capacity);
(
  rows: rows,
  total_demand: total_demand,
  total_capacity: total_capacity,
  totals_fit: total_demand <= total_capacity,
  cells_fit: forall([r.demand <= r.capacity | r in rows]),
  violations: [r | r in rows where r.demand > r.capacity]
);
```

Run `zincite query --program-file capacity.zq data.dzn` or pass the same text
through `--program`. The program returns these rows:

| Index | Scaled demand | Capacity | Headroom |
| --- | --- | --- | --- |
| 1 | 4 | 6 | 2 |
| 2 | 6 | 8 | 2 |
| 3 | 8 | 5 | -3 |

The totals are `18` and `19`, so `totals_fit` is true. `cells_fit` is false and
`violations` contains only row 3. This negative finding is successful report
data: `complete` is true, `limitations` is empty and the command exits 0.

The `require` calls check the assumptions before calculating the rows: the
factor must be positive, the arrays must have the same indices, and values
must be nonnegative. A failed requirement stops the report with its message.

`sum` adds the scaled demand; `forall` checks that every row fits. These calls
evaluate query values, not the MiniZinc model. The
[expression reference](query-comprehension-contract.md#report-expressions-and-edit-grammar)
covers the supported operations and their error cases.

## Compare keyed matrix cells

To compare two matrices, match cells by their indices before subtracting
the values. Save this as `matrix.dzn`:

```minizinc
before = [|3, 4 | 5, 6|];
after = [|4, 6 | 8, 10|];
```

Save the query as `matrix-delta.zq`:

```zirql
let a = items | filter(name("before")) | values | one;
let b = items | filter(name("after")) | values | one;
let pairs = zip_exact(a, b);
[e.key: e.value.2 - e.value.1 | e in entries(pairs)];
```

```sh
zincite query --program-file matrix-delta.zq matrix.dzn
```

`zip_exact` requires both arrays to have the same ordered axes and stored keys.
It pairs the before and after values at each cell. `entries` exposes each key
and pair, so the comprehension can subtract the first value from the second
and keep the original key.

The result has axes `1..2` by `1..2` and these values:

| Row | Column 1 | Column 2 |
| --- | --- | --- |
| 1 | 1 | 2 |
| 2 | 3 | 4 |

Different axes or missing keys on only one side produce an error rather than
pairing unrelated cells. For empty arrays, report `zip_exact(a, b)` directly
if you need to retain the declared axes: this comprehension's `e.key` head
cannot infer them from no entries. These helpers are available in reports,
not data edits.

## Group machine demands

A total for each machine is more useful than a count of all jobs. Save these
jobs as `jobs.dzn`:

```minizinc
jobs = [(machine: "b", demand: 3), (machine: "a", demand: 5),
        (machine: "b", demand: 7), (machine: "b", demand: 3)];
```

Save this as `machine-totals.zq`:

```zirql
let rows = items | filter(name("jobs")) | values | one;
[
    (machine: g.key,
     count: length(g.items),
     total: sum([r.demand | r in g.items]))
    | g in group_by(rows, "machine")
];
```

```sh
zincite query --program-file machine-totals.zq jobs.dzn
```

`group_by` gives each group a `key` and its original `items`. The query counts
the jobs and sums their demands:

| Machine | Jobs | Total demand |
| --- | --- | --- |
| b | 3 | 13 |
| a | 1 | 5 |

Groups follow first occurrence, so `b` comes before `a`. Both jobs with demand
3 remain in the `b` group. Grouping preserves duplicates; it does not assume
that equal records describe the same job.

The input here is a positional array. To group a keyed array, first make its
values positional with `[r | r in keyed_rows]`. Group keys must have compatible
types, and every row must contain the selected field.

## Inspect semantic facts

The previous examples inspect syntax or literal values. Finding the uses of a
declaration needs more context: Zincite must resolve names in the model and
its included files.

The semantic examples below use an installed MiniZinc standard library. Set
`MZN_STDLIB_DIR` to the directory containing `std/stdlib.mzn`. If MiniZinc and
Python 3 are on your path, you can read it from the installation:

```sh
export MZN_STDLIB_DIR=$(minizinc --config-dirs | python3 -c 'import json,sys; print(json.load(sys.stdin)["mznStdlibDir"])')
```

Alternatively, pass `--stdlib-dir DIR` on each command. Use `-I DIR` for project
include directories.

Return to `model.mzn`, the capacity-and-allocations model at the start of this
guide. Find the written uses of `capacity`:

```sh
zincite query 'filter(name("capacity")) | declarations | uses | json' model.mzn
```

`filter` selects its declaration item, `declarations` identifies the declared
value, and `uses` finds references to that value. The result includes its use
in the array domain and in the constraint, with their file locations.

Semantic JSON includes `complete`, `limitations` and `result`. Without a
configured library, even a small model may produce available results with
`complete: false` and exit 1. Read the limitations before treating the result
as a complete inventory. Errors such as a missing include exit 2.

For stdin, semantic inspection also needs `--model` pointing to a file with
exactly the same root bytes. `--stdin-filepath` supplies a name and settings
location, but does not load that model. The [semantic reference](reference.md#semantic-inspection)
describes selection and context in detail.

## Correlate declarations with their uses

To report a name and a use count for each declaration in `model.mzn`, save
this as `use-counts.zq`:

```zirql
subtree | declarations | unique | project(d, {
    "name": d | names | one,
    "uses": d | uses | count
});
```

```sh
zincite query --program-file use-counts.zq model.mzn
```

`project` evaluates both fields for the same declaration `d`. `names` reads its
name, and `one` requires exactly one value. `uses` finds that declaration's
references. `unique` removes repeated visits to the same declaration; two
local declarations with the same name would remain distinct.

For individual uses, save this as `uses.zq`:

```zirql
let matches = match {
    from d in (subtree | declarations | unique);
    from u in (d | uses);
};
matches | project(m, {
    "name": m.d | names | one,
    "text": m.u | text | one
});
```

```sh
zincite query --program-file uses.zq model.mzn
```

The first `from` chooses a declaration. The second visits its uses. Each result
row keeps both objects, so the report can read the declaration's name and the
use's exact source text. In this model, `capacity` has two uses and `allocated`
has one. An unused declaration contributes no row to this second report.

These are source relationships. The query does not count how often an
expression runs or how many variables MiniZinc creates when flattening.
See [projections and captures](reference.md#native-projections-and-captures)
for additional clauses and the library result types.

## Explain declaration connections

A path query can show how one declaration refers to another through
intermediate declarations. Save this as `paths.mzn`:

```minizinc
int: base_capacity = 10;
int: usable_capacity = base_capacity;
int: daily_limit = usable_capacity;
solve satisfy;
```

Save the query as `paths.zq`:

```zirql
subtree | declarations | filter(name("daily_limit"))
| paths(referenced_declarations, to=name("base_capacity"), max_depth=2)
| json
```

```sh
zincite query --program-file paths.zq paths.mzn
```

The result contains the path `daily_limit`, `usable_capacity`, `base_capacity`,
with the original declarations and locations. The depth counts edges: a limit
of two reaches `base_capacity`, while a limit of one does not.

The query returns one shortest path per source/target pair within the bound.
An empty result means no path was established within that depth, not that the
declarations are unrelated at every depth. Unresolved references remain
limitations. These paths follow written references; they do not describe
compiler-generated dataflow. See [bounded paths](reference.md#bounded-declaration-paths)
for ordering and result fields.

## Read optional fields and sparse cells

Reports can distinguish a stored absent value (`<>`) from a field or cell
that was not stored at all. This is useful when inspecting incomplete data.
Save the declarations as `optional-report.mzn`:

```minizinc
array[1 .. 2, 1 .. 2] of opt int: cells;
record(int: id, opt int: missing, opt int: stored): info;
solve satisfy;
```

Save this partial data as `optional-report.dzn`:

```minizinc
cells = [(1, 1): 10, (1, 2): <>, (2, 1): 30];
info = (id: 4, stored: <>);
```

The declarations tell the query which fields and axes are expected. The data
omits `info.missing` and cell `(2, 2)`; it is an inspection example, not a
complete instance to pass to a solver. Save this as `optional-report.zq`:

```zirql
let cells = items | filter(name("cells")) | values | one;
let info = items | filter(name("info")) | values | one;
(
    id: lookup(info, "id") default 0,
    stored_field: has_key(info, "stored"),
    missing_field: has_key(info, "missing"),
    stored_cell: has_key(cells, (1, 2)),
    missing_cell: has_key(cells, (2, 2)),
    fallback: lookup(cells, (2, 2)) default 8
);
```

```sh
zincite query --model optional-report.mzn -f optional-report.zq optional-report.dzn
```

The report gives `id = 4`, `stored_field = true`, `missing_field = false`,
`stored_cell = true`, `missing_cell = false` and `fallback = 8`.
`has_key` checks whether storage exists, even when its value is `<>`.
`lookup` returns absence for a compatible missing key, and `default` supplies
a value only when its left operand is absent.

Strict access such as `cells[2, 2]` would fail. A label outside the declared
record schema also fails. A default does not catch type errors, division by
zero or resource limits. These operations are report-only; data edits require
nonoptional targets and complete rectangular arrays.

## Sort rows and select extrema

Continue with `jobs.dzn` from the machine-grouping example. To put the busiest
machine first, save this as `rank-machines.zq`:

```zirql
let jobs = items | filter(name("jobs")) | values | one;
let totals = [
    (machine: g.key, demand: sum([job.demand | job in g.items]))
    | g in group_by(jobs, "machine")
];
(
    ranked: sort_by(totals, "demand", descending=true),
    busiest: max_by(totals, "demand")
);
```

```sh
zincite query --program-file rank-machines.zq jobs.dzn
```

`ranked` contains machine `b` with demand 13, followed by `a` with demand 5.
`busiest` is the `b` record. Sorting preserves input order for equal keys;
`min_by` and `max_by` choose the first record on a tie.

Use `sort` for an array of scalars and `sort_by` for records. Keys must be
present and have compatible types. To supply a missing score deliberately,
construct a field such as `score: lookup(row, "score") default 0` before
sorting. Empty extrema return `<>`, so `min([]) default 0` supplies zero.
Sorting and extrema are available in reports, not data edits.

## Count written calls with a function

A named function can express a source traversal that you want to reuse or
adapt. Save this input as `calls.mzn`:

```minizinc
int: first_total = sum([1, 2]);
int: second_total = sum(i in 1..2)(i);
int: largest = max([3, 4]);
solve satisfy;
```

Save this query as `calls.zq`:

```zirql
fn count_calls(node: Node, target: string) -> int {
    let here = if (written_call_name(node) default "") = target then 1 else 0 endif;
    here + sum([count_calls(child, target) | child in direct_children(node)])
}
let roots = items;
(sum_calls: sum([count_calls(root, "sum") | root in roots]));
```

```sh
zincite query --program-file calls.zq calls.mzn
```

The result contains `sum_calls = 2`. Both the ordinary array call and the
generator call count; `max` does not. For each node, the function checks its
written call name and adds the counts from its direct children. A node with
no call name contributes zero through the explicit default.

For this count alone, the `call_names | tally` pipeline is shorter. The
function form gives you control over the traversal when adapting the query
to a more specific source pattern. It inspects syntax without resolving or
executing model calls.

Function arguments and results are checked against the declared types.
Recursion is limited by depth and total work. See the
[function reference](reference.md#typed-query-functions) for scope, signatures
and limits.

## Combine functions, defaults and ordering

Return to the staffing problem, this time with a missing availability value.
For this report, we deliberately treat missing availability as zero. The goal
is to rank the days by the staff still needed. Save `rank-shortages.zq`:

```zirql
fn shortage(required: int, available: int) -> int {
    max([0, required - available])
}
let days = [
    (day: 1, required: 2, available: 3),
    (day: 2, required: 3, available: <>),
    (day: 3, required: 4, available: 3)
];
let shortages = [
    (day: row.day, missing: shortage(row.required, row.available default 0))
    | row in days
];
sort_by(shortages, "missing", descending=true);
```

This program supplies its own data. Run it with empty stdin:

```sh
zincite query --program-file rank-shortages.zq < /dev/null
```

The result orders day 2 (three missing staff), day 3 (one), then day 1 (zero).
`shortage` prevents surplus availability from becoming a negative shortage.
The default is an explicit reporting decision, not a claim that the unknown
availability really is zero. If missing data should stop the analysis, use a
requirement instead.

Functions cannot capture top-level bindings, so the required and available
counts are passed as arguments. The report leaves the original staffing files
unchanged.
