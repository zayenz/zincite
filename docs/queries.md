# Inspect MiniZinc source

Use `zincite query` to select source and inspect written structure. The
[query reference](reference.md#query-library) defines the pipeline stages,
selection order, comment attachment and result formats.

## Select items and calls

Create a small model and count its constraints:

```sh
printf 'int: capacity = 20;\nconstraint capacity > 0;\nsolve satisfy;\n' > model.mzn
zincite query 'filter(kind("constraint")) | count' model.mzn
zincite-query 'filter(kind("constraint"))' model.mzn
zincite query 'filter(kind("constraint")) | expressions | call_names | tally | json' model.mzn
```

The count is `1`. The standalone and root commands accept the same options.
Syntax inspection does not need a standard library. For more substantial
examples, use the [feature-extraction cookbook](query-cookbook.md), which creates
its own inputs for counts, histograms, declaration types and literal shapes.

## Reuse results in a report

Save this as `report.zq`:

```text
source selected = query("filter(kind(\"constraint\"))");
source n = selected | query("count");
source histogram = selected | query("expressions | call_names | tally");
report (count: n, histogram: histogram, selected: selected);
```

Run `zincite query -f report.zq model.mzn` or
`zincite-query --program-file report.zq model.mzn`. The JSON envelope contains
`complete`, `limitations` and a record with the native count, histogram and
selection. The selected nodes keep their file, byte range and exact source text.
No model facts are needed for this syntax report. Use `query("declarations")`
in the source binding to select semantic declaration identities instead.

Statements run in order with immutable earlier bindings. Each starts from the
same input selection unless it names another native result. A later failure
leaves stdout empty. [Query programs](reference.md#query-programs) describes
emitters, the library result and aggregate limits.

Read-only programs also support `let` bindings, positional comprehensions and
scalar expressions over query projections or a written literal assignment. The
reference describes the accepted values, checked arithmetic and result schema.
Reports also support keyed arrays, established enum-member identities and
multidimensional access, with explicit axes and rectangular-shape checks.
For new assignments and computed replacements, use a model-backed
[data edit program](data-edits.md#construct-new-data). It uses the common bounded
subset, with `index_set` and `enum_member` as its only expression calls, and
produces one atomic document candidate. The report calls below are excluded
from edit RHSs and preceding lets, including skipped branches and unused lets.

## Compare demand and capacity

For a short report, create `data.dzn` with `demand = [2,3,4];` and run either
command:

```sh
zincite-query --program 'source d = literal("demand"); report sum(d);' data.dzn
zincite query --program 'source d = literal("demand"); report sum(d);' data.dzn
```

Both return a complete report with the tagged integer `9`. Save that program as
`sum.zq` and run `--program-file sum.zq` for the same result. Library callers use
`Program::parse`, evaluate against the data input, then call `render` on the
result. Inline programs need no parameter declaration or named entry point.

Totals alone can miss an overloaded position. For this data:

```minizinc
demand = [2, 3, 4];
capacity = [6, 8, 5];
```

Save the following as `capacity.zq`. The factor is a local literal binding.

```text
source demand = literal("demand");
source capacity = literal("capacity");
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
report (
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

`length`, checked integer `sum`, Boolean `forall` and `exists` take fully
materialized arrays. They retain empty identities `0`, `0`, `true` and `false`.
Every comprehension head runs before reduction. `require` preflights both
operands and accepts a literal message. False fails at the call while skipping
the value's runtime arithmetic, access and construction. A later failure leaves
stdout empty. These reports use literal data and shared Zirql evaluation;
they do not run a MiniZinc model or solver. The
[comprehension contract](query-comprehension-contract.md) defines types, eager
arrays, requirements and request charges.

## Inspect semantic facts

To follow a declaration's uses in that model:

```sh
zincite query 'filter(name("capacity")) | declarations | uses | json' model.mzn
zincite-query 'references | declarations | types | json' model.mzn
```

Semantic queries use model context. Without a configured standard library,
even this small model reports incomplete builtin context and exits 1 while
returning available results. Supply `--stdlib-dir DIR` or `MZN_STDLIB_DIR`
for the library root containing `std/stdlib.mzn`, and `-I DIR` for project
includes. Stdin semantic inspection needs an explicit `--model`
whose retained root bytes match the input exactly; `--stdin-filepath` alone
provides no model context. See [semantic inspection](reference.md#semantic-inspection)
for the root, identity, ordering and traversal contracts.

Semantic JSON reports `complete`, located `limitations` and `result`. Keep the
completeness flag and [exit status](cli.md#exit-codes) with the result. An unknown
fact does not become known by negating a filter. The [support guide](support.md)
explains how to distinguish missing proof from input failure.

To change instance values or reduce an enum, use the [data-edit guide](data-edits.md).
The [query acceptance report](../scripts/query-acceptance.md) retains measured
commands and remaining resource limits for its checked revision.
