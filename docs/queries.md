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
Program-based data edits remain future work and require their existing approval
gate.

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
