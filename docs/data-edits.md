# Edit a data instance

Query transformations produce a complete data candidate. Preview it before an
explicit write: the edit changes instance meaning, and successful parsing checks
syntax only. The [query reference](reference.md#assignment-edits) defines byte
preservation, comment handling and candidate validation.

## Replace an assignment expression

Create a small data file:

```sh
printf 'capacity = 20;\nweights = [1, -2, 3];\nunused = 0;\n' > data.dzn
zincite query --diff 'filter(name("capacity")) | set_value("30")' data.dzn
zincite-query 'filter(name("capacity")) | set_value("10 + 20")' data.dzn
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

The reference describes [literal inspection and filtering](reference.md#query-library),
including supported comparisons and collection shapes. Computed expressions
are not arbitrary evaluated values. Removal includes attached comments; separated
section comments remain.

## Reduce enum membership

An enum with explicit member-keyed data can be reduced without a model:

```sh
printf 'Guests = {A, B, C};\n' > guests.dzn
zincite query --diff 'reduce_enum("Guests", keep("A", "C"))' guests.dzn
```

Supply `--model model.mzn` when positional array alignment needs declarations.
Read the [enum reduction contract](reference.md#enum-reduction) before reducing
dependent arrays, records or nested sets. Incomplete candidates retain located
diagnostics and exit 1; `--write` refuses them. A smaller enum alone does not
prove that all dependent data has been reduced.

Write restrictions and failure statuses are in the [command reference](cli.md).
The [support guide](support.md) describes the semantic boundaries.
