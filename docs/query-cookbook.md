# MiniZinc feature extraction

These recipes count written constraints, calls and annotations, inspect decision
declaration types, and measure literal array and matrix shapes. They use the
current query stages on small inputs you can recreate. None of the commands
flattens a model or runs a solver. All queries here are read-only.

Each pair of commands produces the same output, including a final newline.
Run the setup in a shell with Zincite's two command forms on `PATH`. Python 3 is
needed for the literal shape summaries and the standard-library path lookup.

## Create the inputs

Use a temporary directory so the commands do not replace existing files:

```sh
recipe_dir=$(mktemp -d)
cd "$recipe_dir"
cat > model.mzn <<'MZN'
include "included.mzn";
annotation tag;
int: limit = 3;
var 0..limit: x :: tag;
array[1..2] of var bool: flags :: tag;
constraint :: "nonnegative" x >= 0;
constraint forall(i in 1..2)(bool2int(flags[i]) <= x);
constraint abs(x) = abs(x);
solve satisfy;
MZN
cat > included.mzn <<'MZN'
var int: included_x;
constraint included_x >= 0;
MZN
cat > data.dzn <<'DZN'
weights = [1, 2, 3];
costs = [|1, 2, 3|4, 5, 6|];
DZN
```

`model.mzn` is the root model; `included.mzn` owns its own declaration and
constraint. Syntax queries inspect only the supplied file and never load its
includes. The semantic recipe loads includes for facts but keeps its selection
in the root. `data.dzn` is an independent assignment-only input.

## Count root constraints

```sh
zincite query 'filter(kind("constraint")) | count' model.mzn
zincite-query 'filter(kind("constraint")) | count' model.mzn
```

Each command prints:

```text
3
```

The initial selection contains each top-level item in `model.mzn` once. The
included constraint contributes nothing; query `included.mzn` separately to
count it. Equal constraint text in different items would count separately.
The `forall` item counts as one written constraint, regardless of its generator
domain. This count does not describe flattened constraints or execution frequency.

## Count call names in root constraints

```sh
zincite query 'filter(kind("constraint")) | expressions | call_names | tally' model.mzn
zincite-query 'filter(kind("constraint")) | expressions | call_names | tally' model.mzn
```

Each command prints:

```json
{"abs":2,"bool2int":1,"forall":1}
```

The selection starts at distinct root constraint items. `expressions` visits
nested expressions, and `call_names` keeps ordinary and generator-call heads.
`tally` counts both written `abs` calls, even though their names match. It counts
`bool2int` once in the generator body. It does not expand that body for each
index, include operator symbols such as `>=`, or inspect included definitions.

Navigation expands every selected root independently. If you select overlapping
expressions and then expand `subtree`, the same call node may appear more than
once. Put `unique` before `call_names` to count each selected node once in that
case. Putting `unique` after `call_names` instead collapses equal names, losing
the occurrence counts.

## Count root annotation names

```sh
zincite query 'subtree | annotation_names | tally' model.mzn
zincite-query 'subtree | annotation_names | tally' model.mzn
```

Each command prints:

```json
{"tag":2}
```

`subtree` starts from the root's distinct top-level items. The histogram counts
the two written uses of `:: tag`; declaring `annotation tag` is not an annotation
use. The string constraint label has no identifier head to tally. Annotations in
included files are outside this selection. Equal names stay as separate
occurrences until `tally` groups them. For overlapping selections, put `unique`
before `annotation_names` to remove repeated nodes. These are source annotations,
not counts of annotations after flattening or of times a solver applies them.

## Inspect root decision declaration types

This recipe needs the installed MiniZinc standard library. Read its path from
MiniZinc's configuration, then pass it explicitly to both query forms:

```sh
stdlib_dir=$(minizinc --config-dirs | python3 -c 'import json,sys; print(json.load(sys.stdin)["mznStdlibDir"])')
zincite query --stdlib-dir "$stdlib_dir" 'filter(kind("declaration")) | declarations | filter(instantiation("var")) | types' model.mzn
zincite-query --stdlib-dir "$stdlib_dir" 'filter(kind("declaration")) | declarations | filter(instantiation("var")) | types' model.mzn
```

Each query prints:

```text
int
array
```

The file input supplies the semantic root. `declarations` selects identities
owned by the already selected root declaration items. The loader also retains
`included.mzn` and standard-library sources, but loading them does not add their
declarations to this selection. There are no overlapping roots here, so `x` and
`flags` each appear once; equal types on other declarations would produce
additional rows. `unique` before `types` removes repeated declaration identities,
not distinct declarations with the same type.

The `var` predicate selects the scalar `x` and the array `flags`, whose elements
are decision Booleans. Text output gives their outer type kinds in source order.
It does not report domain sizes or count the individual variables created by
flattening. Add `| json` to retain the structured element and index types and the
owning file and ranges, together with `complete` and `limitations`.

Both example queries exit 0. For other models, missing library configuration or
unknown semantic facts can leave a result incomplete. Exit 1 still emits the
available rows and located notes on stderr; JSON sets `complete` to false.
Unknown facts remain unknown under `not`, and semantic filters keep only proven
matches. A missing row is therefore not evidence that a declaration is `par`.
Check the exit status, or keep the JSON completeness fields, before treating a
report as a complete inventory. Actual model, dependency or query errors exit 2
with empty stdout.

## Inspect literal array and matrix shapes

These commands inspect `data.dzn` only; they need no model or standard library.
`values | json` retains literal structure, spelling and ranges. Python below
summarizes that structure without evaluating MiniZinc expressions.

For the array:

```sh
zincite query 'filter(name("weights")) | values | json' data.dzn | python3 -c 'import json,sys; v=json.load(sys.stdin)[0]; print(v["kind"], v["indexing"], len(v["entries"]))'
zincite-query 'filter(name("weights")) | values | json' data.dzn | python3 -c 'import json,sys; v=json.load(sys.stdin)[0]; print(v["kind"], v["indexing"], len(v["entries"]))'
```

Each pipeline prints:

```text
array positional 3
```

For the matrix:

```sh
zincite query 'filter(name("costs")) | values | json' data.dzn | python3 -c 'import json,sys; v=json.load(sys.stdin)[0]; print(v["kind"], len(v["rows"]), [len(row["cells"]) for row in v["rows"]])'
zincite-query 'filter(name("costs")) | values | json' data.dzn | python3 -c 'import json,sys; v=json.load(sys.stdin)[0]; print(v["kind"], len(v["rows"]), [len(row["cells"]) for row in v["rows"]])'
```

Each pipeline prints:

```text
matrix 2 [3, 3]
```

Each filter selects one assignment and each summary consumes its one literal.
Repeated cell values count separately. The matrix summary reports two written
rows and three cells in each row, keeping the row widths explicit rather than
assuming rectangularity. JSON also retains explicit array keys and matrix axis
keys when present. These shapes describe literal entries, not evaluated index
sets, computed arrays or flattened variable arrays. Includes and model
declarations contribute nothing. `values` rejects calls, comprehensions and
arithmetic instead of inferring a shape. These syntax recipes have no semantic
completeness report.
For scripting, capture the query output and check its status before processing
it, or enable your shell's pipeline failure handling.

For the full JSON formats and stage contracts, see the
[query reference](reference.md#query-library).
