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
