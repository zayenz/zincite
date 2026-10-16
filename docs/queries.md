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

Saved repository examples use the [v0.1.0 checkout](getting-started.md#build-from-a-checkout).
Run their repository-relative paths from the checkout root. Examples that give
file contents can be run in a new directory with the installed commands.

## Select items and calls

Save this small model as `model.mzn`. It shares a capacity across three
allocations:

```minizinc title="model.mzn"
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

## Keep a suffix, reverse it and check its size

Native selections support `tail(n)`, `reverse` and pass-through cardinality checks:

```zirql
let selected = items;
let n = selected | count;
let expected = n - 1;
let message = "selection size changed";
selected | tail(expected) | reverse | check(expected, message) | names
```

`tail` keeps a suffix in its current order; `reverse` then reverses those rows,
including duplicate occurrences. Zero keeps no rows and an oversized bound keeps
all rows. Saved `selected` retains its original rows for later use. These stages
keep native identities and original source ranges. Reordered item output carries
its attached comments, with unsafe formatter directive pairs still refused.

Bare `check` requires a nonempty stream. `check(n)` requires exactly `n` rows;
`check(message)` and `check(n, message)` add a failure message. Success passes the
stream through unchanged; failure reports the expected and actual counts. An
empty stream passes `check(0)` and fails bare `check`. `check()` is invalid.
Program counts and messages can refer to parameters, bindings and local scopes.
Positional queries use literal counts and double-quoted messages:

```sh
zincite query 'items | tail(2) | reverse | check(2, "need two items") | names' data.dzn
```

A check counts returned rows, including when a semantic result has limitations.
It does not prove complete domain knowledge or count the cells inside a literal
array. Use `elements` to select those cells. The Boolean `require` expression
checks a condition separately. These controls apply to native streams; they do
not add constructed-array operations or new data-edit stages.

## Reuse results in a report

Save this as `report.zq`:

```zirql title="report.zq"
let selected = items | filter(kind("constraint"));
let n = selected | count;
let histogram = selected | expressions | call_names | tally;
(count: n, histogram: histogram, selected: selected)
```

Run `zincite query -f report.zq model.mzn` or
`zincite-query --program-file report.zq model.mzn`. The JSON output has
`complete`, `limitations` and `result` fields. Here, `result` contains the count,
histogram and selection. The selected nodes keep their file, byte range and exact source text.
No model facts are needed for this syntax report. Use `declarations`
in the input binding to select semantic declaration identities instead.

`let` evaluates its right-hand side once and gives the result an immutable
name. `selected | count` and `selected | expressions` reuse that selection.
A pipeline beginning with `items` starts from the original input again.

An expression outside a binding emits its result. Here, the final record
produces JSON automatically. Ending with just `selected` would print the selected
source; `report selected` would request its JSON representation instead.
Earlier output expressions also emit, so use `let` for intermediate values you
want to keep out of the output.

Bindings and earlier statements need semicolons. The final expression may end
without one. Statements run in written order, and a later failure leaves stdout
empty. [Query programs](reference.md#query-programs) describes output formats,
the library result and aggregate limits.

The following examples build records and arrays from literal data. Their JSON
uses explicit type tags and array keys, so the tables in this guide show the
values rather than the full serialization. The reference calls pipeline
selections, counts and histograms native results to distinguish them from these
calculated values. The
[result schema](query-comprehension-contract.md#constructed-shape-and-result)
describes how to read those reports from another program.

All examples here are read-only. To change a data file, use the
[data-edit guide](data-edits.md); editing supports a smaller set of expressions.

## Calculate with shared values

A report can calculate entirely from values in the query. Save this as
`counts.zq`:

```zirql title="counts.zq"
let samples = [
  {"label": "pear", "count": 2},
  {"label": "apple", "count": 1},
  {"label": "ash", "count": 2},
  {"label": "birch", "count": 3}
];
let selected = [row for row in samples if row.count > 1];
sort_by({row.label: row.count for row in selected}, value, descending=true)
```

```sh
zincite query --program-file counts.zq model.mzn
```

```json
{"complete":true,"limitations":[],"result":{"birch":{"kind":"integer","value":3},"pear":{"kind":"integer","value":2},"ash":{"kind":"integer","value":2}}}
```

`selected` contains pear, ash and birch in their input order. The dictionary uses
their labels as string keys and their counts as values. `descending=true` puts
the largest count first. Pear and ash both have count 2, so their incoming order
stays pear, then ash. Sorting retains the dictionary kind.

Zincite accepts both `[row for row in samples if row.count > 1]` and
`[row | row in samples where row.count > 1]`. Both use Zirql's reader and
evaluator, with the same scope, order and limits. The [Zirql language guide](https://zayenz.se/zirql/guides/language-reference/)
and [Zirium guide](https://zayenz.se/zirium/guides/query-language/) describe the
shared language. Zincite retains its typed integer payloads and result envelope;
the CSV and Zirium renderers print ordinary JSON integers.

This program does not inspect `model.mzn`. To supply data from MiniZinc source,
select native items and project their `names` or literal `values`, as in the
next examples. Model evaluation, declarations and source edits retain their
MiniZinc-specific rules.

Use `--program` for an inline complete program or `--program-file` for a file.
The positional query argument reads the smaller native pipeline syntax and does
not accept `let` declarations.

## Select values with a predicate

`filter(row => row.count > 1)` names the current element. The equivalent
`filter(_.count > 1)` uses its implicit argument. Both keep the same rows as
`[row for row in samples if row.count > 1]`, in the same order. Bodies can use
visible immutable bindings and pure query functions.

Tuple and list patterns work in bindings, function parameters, predicates,
comprehensions and value-match arms:

```zirql
let [limit, extra, ..] = [2, 1, 99];
fn above((value, bound): tuple<int,int>) -> bool { value > bound }
let pairs = [(1, limit), (4, limit), (3, limit + extra)];
[pair for pair in pairs if above(pair)]
```

A tuple pattern requires exactly its written number of positions. `[a, b]`
requires a two-element positional list; `[a, b, ..]` requires at least two and
discards the rest. Sets and keyed arrays do not become lists for matching. A
binding, parameter, predicate or generator mismatch is a located error. A value
`match` tries its next arm instead. `_` remains a wildcard in patterns; inside
a predicate body it reads the whole current subject, even when a pattern names
its parts. Nested predicates bind `_` anew and retain explicitly named outer
captures.

Native `filter`, `paths(..., to=...)` and native `match` guards accept the same
named and implicit bodies. Native values keep their existing projections:
`items | filter(item => length(item | names) > 0)` tests each source item.
A path target receives one candidate declaration. A native match guard receives
the original query input, with earlier captures available under their own names.
These rules do not add fields to MiniZinc nodes or declarations.

## Continue a calculated array

A saved selection can feed several calculations before the report is rendered.
For a data file, this program sorts the assignment names and retains the
selection alongside its count:

```zirql title="names.zq"
let selected = items | filter(kind("assignment"));
let labels = selected | names;
let n = selected | count;
let rows = [label | label in labels];
let ordered = rows | sort | unique;
(names: ordered, count: n, selected: selected)
```

Run `zincite query -f names.zq data.dzn`, or pass the same source with
`--program`. The comprehension uses MiniZinc's generator pipe inside brackets.
The outer pipes continue the completed array through Zirql's shared collection
operations. `sort` orders homogeneous scalars; `unique` keeps the first equal
value. For string frequencies, replace the last binding with
`let ordered = rows | tally;`. Its result is a dictionary with string keys in first-seen order
and integer counts.

To count only names beginning with `a`, continue the calculated array:

```zirql
let selected = items | filter(kind("assignment"));
let labels = selected | names;
let rows = [label | label in labels];
let matching = rows | filter(starts_with("a"));
matching | count
```

The filter uses exact, case-sensitive prefixes. `count` counts the retained
strings, including duplicates. The same suffixes work on a saved function
result: bind `let labels = make_labels();`, then continue `labels | sort`.

A saved calculated value can feed a pipeline directly: `rows | sort | unique`.
Group a function call when continuing its result, as in `(make_rows()) | tally`.
Native pipelines such as `selected | names` keep their domain stages.
`names`, `count` and `values` provide explicit calculation inputs from native
results; no text or JSON conversion is involved. The same expressions work
with an unrelated function declaration and inside supported pure functions.

These suffixes are read-only report operations. Keyed arrays need an explicit
positional conversion, such as `[e.value | e in entries(a)] | sort`. See the
[constructed pipeline contract](query-comprehension-contract.md#constructed-pipelines)
for empty inputs, supported types and failures.

## Read a named assignment

Create `data.dzn` with demand and capacity arrays:

```minizinc title="data.dzn"
demand = [2, 3, 4];
capacity = [6, 8, 5];
```

Save this as `totals.zq`:

```zirql title="totals.zq"
let demand = items | filter(name("demand")) | values | one;
let total_demand = sum(demand);
let capacity = items | filter(name("capacity")) | values | one;
(total_demand: total_demand, total_capacity: sum(capacity))
```

```sh
zincite query -f totals.zq data.dzn
```

The result contains `total_demand = 9` and `total_capacity = 19`. The input
pipeline reads the demand in four steps:

| Stage | Result |
| --- | --- |
| `items` | The two assignments in `data.dzn`. |
| `filter(name("demand"))` | The assignment named `demand`. |
| `values` | A sequence containing its literal array `[2, 3, 4]`. |
| `one` | The array itself, extracted from that sequence. |

`one` checks the number of projected values, not the number of array elements.
Zero or multiple matches fail. `values` reads written literals; it cannot
evaluate a MiniZinc right-hand side such as `[n * 2 | n in 1..3]`.

Input reads and calculations use the same `let` syntax and can appear in the
order you need them. Here, `total_demand` is calculated before `capacity` is
read. The quoted name chooses an assignment in the file; the binding name is
how the query refers to its value. For example, `let required = items |
filter(name("demand")) | values | one;` would read the same demand array.

## Compare demand and capacity

Using the same `data.dzn`, suppose demand may double. We want to compare the new
demand with capacity at each position, as well as in total. Save the following
as `capacity.zq`. The factor is a local literal binding.

```zirql title="capacity.zq"
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
)
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

## Use a query as a checker

To reject insufficient capacity, put the condition inside `require`. Save this
program as `capacity-check.zq`:

```zirql title="capacity-check.zq"
let demand = items | filter(name("demand")) | values | one;
let capacity = items | filter(name("capacity")) | values | one;
let aligned_capacity = require(
  index_set(demand) = index_set(capacity), capacity,
  "demand and capacity must have the same index set"
);
require(
  forall([demand[i] >= 0 /\ aligned_capacity[i] >= demand[i]
          | i in index_set(demand)]),
  true,
  "each capacity must cover its nonnegative demand"
)
```

For `demand = [2, 3, 4]; capacity = [6, 8, 5];`, every cell fits and
the program returns true. Changing the final capacity to `3` fails with
`each capacity must cover its nonnegative demand` and exit status 2.
Mismatched index sets fail before cell access.

Both query commands accept the file or its text:

```sh
zincite query --program-file capacity-check.zq data.dzn
zincite-query --program-file capacity-check.zq data.dzn
zincite query --program "$(cat capacity-check.zq)" data.dzn
zincite-query --program "$(cat capacity-check.zq)" data.dzn
```

Use the exit status in Bash. Redirect stdout to suppress the successful report;
diagnostics still go to stderr, and a failed requirement selects the else branch.

```sh
if zincite query --program-file capacity-check.zq data.dzn >/dev/null; then
    echo "capacity check passed"
else
    echo "capacity check failed" >&2
    exit 1
fi
```

A plain `false` expression is a successful report value. A checker must use
`require` to turn a failed condition into an error. Unknown conditions and
evaluation errors also fail; they cannot satisfy a requirement. As with other
query failures, a later failed requirement leaves stdout empty even when an
earlier statement requested output.

Library callers use the same program:

```rust,ignore
use zincite_query::{Input, Program, ProgramLimits};
use zincite_syntax::FileMode;

let input = Input::parse(
    b"demand=[2,3,4]; capacity=[6,8,5];".to_vec(),
    FileMode::Data,
)?;
let limits = ProgramLimits::default();
let checker = Program::parse(include_str!("capacity-check.zq"), limits)?;
let checked = checker.evaluate(&input, limits)?;
assert!(checked.is_complete());
```

## Construct indexed arrays

Use integer or resolved enum keys when a cell's address should survive a filter.
A tuple supplies row and column keys. Save this as `indexed-arrays.zq`, or use
[the saved example](../crates/zincite-query/tests/fixtures/indexed-arrays.zq):

```zirql title="indexed-arrays.zq"
let sparse = [(5,8):58, (2,1):21, (2,8):28];
let generated = [(i,j):i*10+j | i in [5,2], j in [8,1] where i=2 \/ j=8];
let empty = [(i,j):1 | i in 1..0, j in {8,1}];
let unused = [i:i | i in [5,2,9] where false];
(sparse:sparse, same:sparse=generated, cells:entries(sparse), empty:empty, unused:unused)
```

Run it against the `model.mzn` above:

```sh
zincite query --program-file indexed-arrays.zq model.mzn
zincite-query --program-file indexed-arrays.zq model.mzn
```

Both commands return the same complete tagged report. The `same` field is true.
The row keys are `2, 5` and the column keys are `1, 8`; these ordered domains
are the array's axes. Stored cells follow row-major order:

| Row | Column | Value |
| --- | --- | --- |
| 2 | 1 | 21 |
| 2 | 8 | 28 |
| 5 | 8 | 58 |

There is no cell at `(5, 1)`. Reports retain this sparse shape, while constructed
data edits still require contiguous keys and rectangular coverage. Numeric keys
are inferred in numeric order; resolved enum keys use declaration order and keep
their model identity. `entries` returns only present cells. `index_set` returns
the full keys of a rank-one array, including declared keys without a cell.

The `empty` result retains two dimensions: a typed empty integer row domain and
column keys `8, 1`. The `unused` result has no cells but keeps keys `5, 2, 9`.
For these natural MiniZinc comprehensions, simple generator key names provide
a lazy zero-cell fallback from the surrounding source values. It runs only
when no cells were produced. Model-backed typed empty nominal sources retain
their enum identity too. Rust callers can supply eager declared domains through
[the typed value API](query-comprehension-contract.md#constructed-shape-and-result)
when nonempty results must also retain unused keys or a chosen domain order.

## Compare keyed matrix cells

To compare two matrices, match cells by their indices before subtracting
the values. Save this as `matrix.dzn`:

```minizinc title="matrix.dzn"
before = [|3, 4 | 5, 6|];
after = [|4, 6 | 8, 10|];
```

Save the query as `matrix-delta.zq`:

```zirql title="matrix-delta.zq"
let a = items | filter(name("before")) | values | one;
let b = items | filter(name("after")) | values | one;
let pairs = zip_exact(a, b);
[e.key: e.value.2 - e.value.1 | e in entries(pairs)]
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

```minizinc title="jobs.dzn"
jobs = [(machine: "b", demand: 3), (machine: "a", demand: 5),
        (machine: "b", demand: 7), (machine: "b", demand: 3)];
```

Save this as `machine-totals.zq`:

```zirql title="machine-totals.zq"
let rows = items | filter(name("jobs")) | values | one;
[
    (machine: g.key,
     count: length(g.items),
     total: sum([r.demand | r in g.items]))
    | g in group_by(rows, "machine")
]
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

```zirql title="use-counts.zq"
subtree | declarations | unique | project(d, {
    "name": d | names | one,
    "uses": d | uses | count
})
```

```sh
zincite query --program-file use-counts.zq model.mzn
```

`project` evaluates both fields for the same declaration `d`. `names` reads its
name, and `one` requires exactly one value. `uses` finds that declaration's
references. `unique` removes repeated visits to the same declaration; two
local declarations with the same name would remain distinct.

For individual uses, save this as `uses.zq`:

```zirql title="uses.zq"
let matches = match {
    from d in (subtree | declarations | unique);
    from u in (d | uses);
};
matches | project(m, {
    "name": m.d | names | one,
    "text": m.u | text | one
})
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

```minizinc title="paths.mzn"
int: base_capacity = 10;
int: usable_capacity = base_capacity;
int: daily_limit = usable_capacity;
solve satisfy;
```

Save the query as `paths.zq`:

```zirql title="paths.zq"
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

## Filter connections and accumulate declarations

`where_exists(query)` keeps each original subject when its singleton-relative
query returns a nonempty native selection. It preserves the incoming order and
duplicates. For this model, saved as `connections.mzn`:

```minizinc title="connections.mzn"
int: A = if true then B else C endif;
int: B = D;
int: C = B;
int: D = A;
int: E = 0;
solve satisfy;
```

This query keeps `A` and `C`, which refer directly to `B`:

```sh
zincite query --program 'declarations | where_exists(referenced_declarations | filter(name("B"))) | names' connections.mzn
```

The relation can read saved bindings and function parameters. A complete empty
relation rejects the subject. An unresolved endpoint or a final unknown filter
condition refuses the nested observation at its responsible source location,
including when an earlier query already recorded the limitation. Saving a
partial result does not make it complete. Other report limitations remain
attached to successful complete observations.

`fixpoint(query)` repeatedly replaces the current selection. Accumulation uses
an explicit union:

```sh
zincite query --program 'let seed = declarations | filter(name("A")); seed | fixpoint(input union referenced_declarations) | names' connections.mzn
```

This returns `A`, `B`, `C`, `D` in that order. The body must keep the exact native
kind and owner. Equality compares identities in order, including duplicates.
An unchanged or stable empty selection succeeds; an alternating selection such
as `declarations | fixpoint(reverse)` fails as a cycle. The runner retains one
checkpoint and uses the continuing request budget.

Both bodies are pure report queries. Unsupported result kinds and effects fail
during preparation, even on empty input. Filtering a single `Node` or
`Declaration` returns the corresponding selection, which can be empty. Fixed
points preserve the exact singleton contract. These operators do not extend the
data-edit expression allowlist.

## Read optional fields and sparse cells

Reports can distinguish a stored absent value (`<>`) from a field or cell
that was not stored at all. This is useful when inspecting incomplete data.
Save the declarations as `optional-report.mzn`:

```minizinc title="optional-report.mzn"
array[1 .. 2, 1 .. 2] of opt int: cells;
record(int: id, opt int: missing, opt int: stored): info;
solve satisfy;
```

Save this partial data as `optional-report.dzn`:

```minizinc title="optional-report.dzn"
cells = [(1, 1): 10, (1, 2): <>, (2, 1): 30];
info = (id: 4, stored: <>);
```

The declarations tell the query which fields and axes are expected. The data
omits `info.missing` and cell `(2, 2)`; it is an inspection example, not a
complete instance to pass to a solver. Save this as `optional-report.zq`:

```zirql title="optional-report.zq"
let cells = items | filter(name("cells")) | values | one;
let info = items | filter(name("info")) | values | one;
(
    id: lookup(info, "id") default 0,
    stored_field: has_key(info, "stored"),
    missing_field: has_key(info, "missing"),
    stored_cell: has_key(cells, (1, 2)),
    missing_cell: has_key(cells, (2, 2)),
    fallback: lookup(cells, (2, 2)) default 8
)
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

```zirql title="rank-machines.zq"
let jobs = items | filter(name("jobs")) | values | one;
let totals = [
    (machine: g.key, demand: sum([job.demand | job in g.items]))
    | g in group_by(jobs, "machine")
];
(
    ranked: sort_by(totals, "demand", descending=true),
    busiest: max_by(totals, "demand")
)
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

## Order native selections

Keep the selected source items while ordering them by a scalar projection. For
`int: A = 1; int: B = 1; int: C = 2;`:

```sh
zincite-query --program 'items | sort_by(expressions | tail(1) | values) | names' model.mzn
```

The names are `A`, `B`, `C`; `A` stays before `B` because their keys tie.
Replace `sort_by` with `min_by` to retain `A`, or `min_all_by` to retain both
`A` and `B`. `max_by` and `max_all_by` retain `C`. The selected nodes keep their
original file and byte ranges. A selector must return one present integer,
finite float, string or nominal member. Nominal keys use declaration order.
Counts are checked before conversion to signed keys. Empty native extrema keep
an empty selection of the same kind; constructed `min([])` still returns `<>`.

Set expressions keep operand order. For declarations whose written references
yield `C, A, C` and `B, A`, this program produces names `C, A, B`:

```zirql
let left = items | filter(name("left")) | references | declarations;
let right = items | filter(name("right")) | references | declarations;
(left union right) | names
```

`left intersect right` yields `A`; `left except right` yields `C`. These forms
use native identities and remove repeats without moving their first occurrence.
Native selections, strings and semantic rows support the algebra; projected
literal values, counts and dictionaries need their own explicit operations.
All ordering and set forms are read-only. Unsupported kinds and impure selectors
fail before execution, including empty inputs and unused branches. Tally
objects keep their keys in first-seen order too: `z, a, z` produces
`{"z":2,"a":1}`.

## Rank a native dictionary

`map_by` retains one payload for each selected item under a string key. Save
this model as `dictionary.mzn`:

```minizinc title="dictionary.mzn"
int: A = 1;
int: B = 2;
int: C = 3;
int: reads = B + B + A;
solve satisfy;
```

This report keeps the two most used declarations:

```sh
zincite query --program 'let ds = declarations | filter(name("A") or name("B") or name("C")); ds | map_by(names, uses | count) | sort_by(value) | reverse | head(2)' dictionary.mzn
```

```json
{"complete":true,"limitations":[],"result":{"B":2,"A":1}}
```

Each key selector runs before its value selector, against the same original
singleton. Both can read outer bindings; explicit sources such as `items`
still read the original input. A duplicate key fails before its value runs.
Payloads retain their native handles and source ranges until rendering.

Dictionaries retain insertion order through bindings and nesting.
`sort_by(value)` orders present homogeneous integers, finite floats, strings or
members of one nominal type. Boolean, absent, mixed numeric and nested values
cannot supply sort keys. Sorting keeps the current order within ties, so
`reverse | sort_by(value)` keeps equal values in their reversed order.
`head(n)` and `tail(n)` retain dictionary kind, including zero, empty and
oversized bounds. Native `tally` dictionaries support the same sorting and bounds
without recounting their entries.

Constructed dictionary fields keep their existing tagged values inside an
ordered JSON object. For example:

```sh
zincite query --program 'let made = (z: (inner: 3), a: 1); made' dictionary.mzn
```

```json
{"complete":true,"limitations":[],"result":{"z":{"inner":{"kind":"integer","value":3}},"a":{"kind":"integer","value":1}}}
```

Arrays, scalar values and nominal members retain their
[constructed tags](query-comprehension-contract.md#constructed-shape-and-result).
Native count payloads in the first report render as ordinary numbers.

## Count written calls with a function

A named function can express a source traversal that you want to reuse or
adapt. Save this input as `calls.mzn`:

```minizinc title="calls.mzn"
int: first_total = sum([1, 2]);
int: second_total = sum(i in 1..2)(i);
int: largest = max([3, 4]);
solve satisfy;
```

Save this query as `calls.zq`:

```zirql title="calls.zq"
fn count_calls(node: Node, target: string) -> int {
    let here = if (written_call_name(node) default "") = target then 1 else 0 endif;
    here + sum([count_calls(child, target) | child in direct_children(node)])
}
let roots = items;
(sum_calls: sum([count_calls(root, "sum") | root in roots]))
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

## Walk declaration references

A custom worklist can follow declaration references while skipping duplicate
routes and cycles. Save this source as `worklist.mzn`:

```minizinc title="worklist.mzn"
int: A = if true then B else C endif;
int: B = D;
int: C = B;
int: D = A;
int: E = 0;
solve satisfy;
```

This static graph is A → [B, C], B → D, C → B, D → A, with disconnected E.
The conditional contributes both written references. The query inspects those
source relationships; it does not evaluate the cyclic MiniZinc definitions.
Save this program as `worklist.zq`:

```zirql title="worklist.zq"
fn reaches((start, target): tuple<Declaration, Declaration>) -> bool {
    loop (pending = queue([start]), seen = identity_set<Declaration>()) {
        match (pop_front(pending)) {
            present((current, remaining)) => {
                if current in seen {
                    continue (remaining, seen);
                } else if same_declaration(current, target) {
                    break true;
                } else {
                    let {targets, ..} = reference_info(current);
                    continue (push_back_all(remaining, targets), insert(seen, current));
                }
            },
            _ => break false
        }
    }
}
let starts = declarations | filter(name("A"));
let targets = declarations | filter(name("D"));
let others = declarations | filter(name("E"));
(zero_edges: one([reaches((start, start)) | start in starts]),
 reachable: one([reaches((start, target)) | start in starts, target in targets]),
 disconnected: one([reaches((start, disconnected)) | start in starts, disconnected in others]));
```

```sh
zincite query --program-file worklist.zq worklist.mzn
zincite-query --program-file worklist.zq worklist.mzn
```

With a configured standard library, both commands return a complete report with
`zero_edges = true`, `reachable = true` and `disconnected = false`. Native
comprehension elements supply the explicit Declaration arguments. Queue order
preserves target order, and the identity set prevents repeated expansion.
`reference_info` refuses uncertain adjacency at its source location; an unknown
edge cannot become a false reachability result.

## Retain reducer states

A named reducer can destructure its state parameter. Save this as `fold-scan.zq`
and run it against `worklist.mzn` with either query command:

```zirql title="fold-scan.zq"
fn step((count, total): tuple<int, int>, item: int) -> tuple<int, int> {
    (count + 1, total + item)
}
(folded: fold([2, 3, 5], (0, 0), step),
 scanned: scan([2, 3, 5], (0, 0), step),
 empty_fold: fold([], (0, 0), step),
 empty_scan: scan([], (0, 0), step));
```

`folded` is `(3, 10)`, and `scanned` contains `(1, 2)`, `(2, 5)` and `(3, 10)`.
The empty fold returns `(0, 0)`; the empty scan has no states. Steps run in input
order and must preserve the state contract. The same rules apply to retained
native queue/set states. Those states can be inspected in the library, while
rendering a queue or identity set is a located error.

Rust callers construct these same nodes with `Program::from_values`. For example,
this typed loop returns 3 without parsing query text:

```rust
use zincite_query::{Program, ProgramLimits, ProgramValue, Query};
use zirql::{ConstructedValue as V, Located, ValueBinaryOperator as B,
            ValueExpressionKind as K};
fn e(value: K<zirql::Expression<Query, (), ()>>) -> ProgramValue {
    Located::new(value, 0..1)
}
let n = || e(K::Name("n".into()));
let integer = |v| e(K::Literal(V::Integer(v)));
let program = Program::from_values(vec![], vec![], e(K::Loop {
    states: vec![(Located::new("n".into(), 0..1), integer(0))],
    body: Box::new(e(K::Conditional {
        branches: vec![(e(K::Binary {
            operator: B::Less, left: Box::new(n()), right: Box::new(integer(3)),
        }), e(K::Continue(vec![e(K::Binary {
            operator: B::Add, left: Box::new(n()), right: Box::new(integer(1)),
        })])))],
        otherwise: Box::new(e(K::Break(Box::new(n())))),
    })),
}), ProgramLimits::default())?;
```

`K::Fold { items, initial, step }` and `K::Scan { items, initial, step }` take
boxed value nodes plus a located function name. `ProgramFunction` carries the
named step and its located parameter patterns and declarations. Native query
leaves use `zirql::Expression<Query, (), ()>`, just as other typed reports.
Evaluate with the original `Input` and retained model facts, then inspect
`result.emissions()[0].value()` before rendering. Tuple/sequence `values()`
borrow native members; computational `len()` inspects retained queues and sets.
The library and CLI share preparation, native ownership checks and allowances.

## Destructure a report

Record and tuple patterns let a report name the parts of a structured value
without a separate binding for every field. This example adds the numbers in
a pair, keeps its label and includes a source declaration in the result.
Use this `calls.mzn` for both examples below:

```minizinc title="calls.mzn"
int: first_total = sum([1, 2]);
int: second_total = sum(i in 1..2)(i);
int: largest = max([3, 4]);
solve satisfy;
```

The `describe` function takes the pair apart in its parameter pattern. The
following `let` takes the result apart and renames `label` to `title`:

```zirql title="destructured-report.zq"
fn describe({pair: (a, b), label}: record{pair: tuple<int, int>, label: string}) -> record{total: int, label: string} {
    (total: a + b, label: label)
}
let {total, label: title} = describe((pair: (3, 4), label: "sum"));
fn keep(nodes: sequence<Node>) -> record{selected: sequence<Node>} { (selected: nodes) }
let roots = items | head(1);
let {selected: root} = keep(roots);
report (total: total, title: title, root: root);
```

```sh
zincite query --program-file destructured-report.zq calls.mzn
zincite-query --program-file destructured-report.zq calls.mzn
```

The report has `total = 7`, `title = "sum"` and a `root` selection containing
`first_total`. Passing the selection through `keep` preserves its original file
and source range. In the JSON result, `root` contains that source information;
`total` and `title` use the tagged values described in the
[value reference](reference.md#values-and-report-expressions).

## Choose an optional value with guarded arms

Use a guarded `match` when a report needs different calculations for absent,
negative and positive values. This program uses the same `calls.mzn`:

```zirql title="optional-match.zq"
let {value: candidate, ..} = (value: 7, ignored: "kept out");
report match(candidate) {
    absent => "missing",
    present(value) if value < 0 => "negative",
    present(value) if value > 0 => {
        let (first, second) = (value, 2);
        if first > second {
            let result = first + second;
            result
        } else { 0 }
    },
    _ => 0,
};
```

```sh
zincite query --program-file optional-match.zq calls.mzn
zincite-query --program-file optional-match.zq calls.mzn
```

Both commands report the tagged integer `9`. The record pattern binds
`candidate` to `7`; its final `..` ignores `ignored`. The positive arm then adds
`2`. With `<>` in place of `7`, the first arm would report `"missing"`.

`match` evaluates its subject once, tries arms in order and runs the first body
whose pattern and guard succeed. `present(value)` binds a present value, while
`absent` matches `<>`. The names `first`, `second` and `result` are local to
their blocks. A final unguarded `_` arm is required; here it handles zero.

Both saved programs also work with `--program TEXT`. For the complete pattern
rules and Rust API, see the [pattern reference](reference.md#patterns-and-pure-control-values).
Matching a value is separate from finding source structures with the
[`match { from ...; bind ...; }` capture syntax](#correlate-declarations-with-their-uses).

## Combine functions, defaults and ordering

Return to the staffing problem, this time with a missing availability value.
For this report, we deliberately treat missing availability as zero. The goal
is to rank the days by the staff still needed. Save `rank-shortages.zq`:

```zirql title="rank-shortages.zq"
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
sort_by(shortages, "missing", descending=true)
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

## Build a dictionary from string keys

Bare braces construct a set or a string dictionary in a read-only report.
A colon separates each dictionary key expression from its value. This query
needs no model facts and works with the existing `data.dzn` fixture:

```sh
zincite query --program 'let words=["pear","apple"]; {w: {n mod 3 for n in 1..5} for w in words}' data.dzn
```

The dictionary keeps `pear`, then `apple`; each value is a set iterating as
`1, 2, 0`. Zincite renders that set with its existing `kind` and `elements`
fields inside the complete report envelope. Saving a dictionary retains its
keys, values and insertion order. `{w: 1}` reads the bound string `w`;
`{"w": 1}` uses the literal key `w`.

Each key must be a present string. A duplicate key fails before evaluating
its value. `d["pear"]` is strict and fails if the stored entry is missing;
`lookup(d, "pear")` returns absence for a valid missing entry. A present absent
value remains distinct from a missing entry.

`{3, 1, 3}` keeps `3, 1`, and `sort({3, 1, 3})` keeps the set kind in order
`1, 3`. Empty `{}` is a dictionary. Use `set<int>()`, `set<bool>()` or
`set<float>()` for typed empty scalar sets. Nominal values and empty nominal
sets retain their host-supplied type identity. Strings are not set elements.
Natural MiniZinc sets, `(field: value)` records and `|` comprehensions remain
available. The common dictionary and typed empty syntax is report-only; the
[data-edit expression rules](query-comprehension-contract.md#report-expressions-and-edit-grammar)
retain their existing closed subset.
