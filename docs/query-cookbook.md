# MiniZinc feature extraction

Use these recipes to compare the source structure of models: how many
constraints are written, which calls they use, and what kinds of decision
variables they declare. These counts describe the source, so a `forall`
constraint counts once regardless of its index set. They do not measure the
size of the flattened model or predict solver runtime.

The recipes are read-only. They use `zincite` on `PATH`; the declaration-type
recipe also needs MiniZinc, and the literal-shape summaries use Python 3.

## Create the inputs

Save these two files in a new directory:

```minizinc title="staffing.mzn"
include "globals.mzn";
annotation staffing_rule;
set of int: Days = 1 .. 3;
array[Days] of int: required :: staffing_rule = [2, 3, 4];
array[Days] of int: available = [3, 4, 5];
array[Days] of var 0 .. max(available): staff :: staffing_rule;
constraint :: "Meet demand"
forall (d in Days) (
    staff[d] >= required[d]
);
constraint :: "Respect availability"
forall (d in Days) (
    staff[d] <= available[d]
);
solve minimize sum(staff);
```

```minizinc title="data.dzn"
weights = [1, 2, 3];
costs = [|
      1, 2, 3
    | 4, 5, 6
|];
```

The staffing model follows the [introductory walkthrough](first-model.md).
Here the data is in the model, and the `required` and `staff` declarations
carry a `staffing_rule` annotation. Both constraints have string labels. The
separate `data.dzn` file is for inspecting literal array and matrix shapes.

## Count constraints

```sh
zincite query 'filter(kind("constraint")) | count' staffing.mzn
```

Output:

```text
2
```

The initial selection contains each top-level item in `staffing.mzn` once.
Syntax queries inspect only the supplied file: they do not load
`globals.mzn`. Each `forall` counts as one written constraint, rather than
one constraint per day. Two separate items with identical source text would
still count twice.

## Count calls inside constraints

```sh
zincite query 'filter(kind("constraint")) | expressions | call_names | tally' staffing.mzn
```

Output:

```json
{"forall":2}
```

`expressions` visits nested expressions, and `call_names` selects ordinary
and generator-call names. The `max` in the variable domain and the `sum` in
the objective are outside the selected constraints, so they do not appear.
Operators such as `>=` are not call names. Remove the initial
`filter(kind("constraint")) |` to include calls elsewhere in the file:
`{"max":1,"forall":2,"sum":1}`. Keys follow the first occurrence of each
call name in the source.

Navigation visits each selected root independently. If your selection contains
both an expression and its parent, expanding `subtree` can visit the same call
twice. Put `unique` before `call_names` to remove repeated nodes. Putting it
after `call_names` would instead remove repeated names and lose the counts.

## Count annotations

```sh
zincite query 'subtree | annotation_names | tally' staffing.mzn
```

Output:

```json
{"staffing_rule":2}
```

This counts the two uses of `:: staffing_rule`. The declaration
`annotation staffing_rule` is not a use. String constraint labels do not have
names to tally. As with calls, use `unique` before `annotation_names` if your
starting selection overlaps.

## Inspect decision declaration types

This recipe uses model facts. Read the standard-library path from MiniZinc's
configuration and pass it to Zincite:

```sh
stdlib_dir=$(
  minizinc --config-dirs |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["mznStdlibDir"])'
)
zincite query --stdlib-dir "$stdlib_dir" \
  'filter(kind("declaration")) | declarations | filter(instantiation("var")) | types' \
  staffing.mzn
```

Output:

```text
array
```

`staff` is the only decision declaration in the file. The `var` predicate also
matches an array whose elements are decision variables. Text output shows its
outer type, `array`; add `| json` to inspect its element and index types, source
location and completeness fields.

The loader reads included files to establish model facts, but their declarations
are not added to the selection. The query still starts from declarations written
in `staffing.mzn`.

This example exits 0. For other models, unavailable facts can leave a result
incomplete. Exit 1 emits the available rows and notes on stderr; JSON sets
`complete` to false. A missing row is then not proof that a declaration is a
parameter. Check the exit status before treating the result as a complete
inventory. Input or query errors exit 2 with empty stdout.

## Keep calculated rows

A predicate and a filtered comprehension select the same ordered rows:

```sh
zincite query --program '
let rows=[(weight:2),(weight:5),(weight:3)];
rows | filter(row => row.weight > 2)
' \
  data.dzn
zincite query --program \
  '[row for row in [(weight:2),(weight:5),(weight:3)] if row.weight > 2]' \
  data.dzn
```

The implicit body is `filter(_.weight > 2)`. The MiniZinc-style comprehension
`[row | row in rows where row.weight > 2]` has the same meaning. These queries
calculate from their written values; the input file is unchanged.

## Inspect literal array and matrix shapes

These commands inspect `data.dzn` without a model or standard library.
`values | json` retains literal structure, spelling and ranges. The Python
commands summarize that structure without evaluating MiniZinc expressions.

For the array:

```sh
zincite query 'filter(name("weights")) | values | json' data.dzn |
  python3 -c '
import json, sys
value = json.load(sys.stdin)[0]
print(value["kind"], value["indexing"], len(value["entries"]))
'
```

Output:

```text
array positional 3
```

For the matrix:

```sh
zincite query 'filter(name("costs")) | values | json' data.dzn |
  python3 -c '
import json, sys
value = json.load(sys.stdin)[0]
print(value["kind"], len(value["rows"]), [len(row["cells"]) for row in value["rows"]])
'
```

Output:

```text
matrix 2 [3, 3]
```

The matrix has two written rows, each with three cells. These are literal
shapes: `values` rejects computed expressions such as comprehensions instead
of evaluating them. Repeated values count as separate entries. Explicit array
keys and matrix axis keys, when present, are also available in the JSON.

For scripts, capture the query output and check its exit status before
processing it, or enable your shell's pipeline failure handling.
The [query reference](reference.md#query-library) defines the full JSON formats
and query stages.

To calculate with the values themselves, [bind a named assignment with
`let`](queries.md#read-a-named-assignment). That example sums a literal array
inside the query and combines it with another assignment in one report.
