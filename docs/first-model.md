# Format, lint and inspect a staffing model

A model can be neatly formatted and still describe an impossible instance.
This small staffing example shows what Zincite can help with at each step:
layout, feedback on the source, and a query that finds a shortage in the data.

[Install Zincite](getting-started.md) first, then create the files below in a
new directory. The Zincite commands in this walkthrough need no MiniZinc
installation or solver.

## A three-day staffing problem

We need a certain number of staff on each day, with at most ten available.
The decision is how many to use. People available on one day cannot cover a
shortage on another.

Save this deliberately compact model as `staffing.mzn`:

```minizinc title="staffing.mzn"
include "globals.mzn";

set of int: Days=1..3;
array[Days] of 0..10: required;
array[Days] of 0..10: available;
array[Days] of var 0..max(available): staff;

% Staff cannot be moved from one day to another.
constraint forall(d in Days)(staff[d]>=required[d]);
constraint forall(d in Days)(staff[d]<=available[d]);
solve minimize sum(staff);
```

The first constraint covers the demand; the second limits the staff to those
available. The objective minimises the total number of staff-days used.
`Days` is an ordered integer index set, so day 3 means the third day.

Save the instance as `staffing.dzn`:

```minizinc title="staffing.dzn"
required = [2, 3, 4];
available = [3, 4, 3];
```

There are nine staff-days of demand and ten available. That sounds sufficient,
but look at the last day.

## Formatting the model

Preview the formatting:

```sh
zincite fmt staffing.mzn
```

With the default settings, Zincite prints:

```minizinc title="staffing.mzn (formatted)"
include "globals.mzn";

set of int: Days = 1 .. 3;
array[Days] of 0 .. 10: required;
array[Days] of 0 .. 10: available;
array[Days] of var 0 .. max(available): staff;

% Staff cannot be moved from one day to another.
constraint forall (d in Days) (
    staff[d] >= required[d]
);
constraint forall (d in Days) (
    staff[d] <= available[d]
);
solve minimize sum(staff);
```

The `forall` bodies are easier to pick out, and the comment and names are
unchanged. The file itself has not been written. To apply the layout and then
check it:

```sh
zincite fmt --write staffing.mzn staffing.dzn
zincite fmt --check staffing.mzn staffing.dzn
```

The check exits 0 when the files are formatted. An `.editorconfig` can change
indentation and line width; see the [formatting guide](formatting.md).

## Linting constraint labels

Run the two local style checks:

```sh
zincite lint --isolated --rules naming,missing-constraint-label staffing.mzn
```

Zincite reports two `missing-constraint-label` warnings, each with its source
location, and exits 1. Both have this message:

```text
consider adding a string label to explain this constraint's modelling intent
```

The names already follow the conventions: `Days` names a set, while `required`,
`available` and `staff` name values. The missing labels are advice about making
the constraints understandable. The formatter cannot supply that explanation.

Replace the two constraints and their preceding comment with:

```minizinc title="staffing.mzn (labelled constraints)"
% Demand must be met on each day, even when the total availability is sufficient.
constraint :: "Cover each day's demand"
forall (d in Days) (
    staff[d] >= required[d]
);

% Availability on another day cannot cover today's shortage.
constraint :: "Use only available staff"
forall (d in Days) (
    staff[d] <= available[d]
);
```

Run the same lint command again. It produces no warnings and exits 0. This
only establishes that these two checks pass; the data still has a shortage.

`--isolated` ignores project lint settings, and `--rules` selects exactly the
checks used here. Ordinary `zincite lint staffing.mzn` does not run them. It
checks for likely mistakes such as incompatible indices, shadowed names, partial
expressions, vacuous constraints and unused declarations. Those checks use model
context; the [linting guide](linting.md) explains library configuration, how to
add the naming and label checks to every run, and the other rule presets.

## Selecting constraints

Count the constraint items written in the model:

```sh
zincite query 'filter(kind("constraint")) | count' staffing.mzn
```

This prints `2`. Each `forall` is one written constraint, regardless of how many
days it covers. A query about the source does not count the constraints a
MiniZinc compiler would generate.

To extract those constraints with their labels and attached comments:

```sh
zincite query 'filter(kind("constraint"))' staffing.mzn
```

The pipeline starts with the file's top-level items. `filter` keeps the
constraint items, and `count` counts the selection. Without `count`, the
command prints the selected source.

## Querying staff shortages

Inspecting the instance answers a different question. We want the days where
the required staff exceeds the available staff, along with the size of each
shortage.

Save this query as `shortages.zq`:

```zirql title="shortages.zq"
let required = items | filter(name("required")) | values | one;
let available = items | filter(name("available")) | values | one;
[
    (day: d, missing: required[d] - available[d])
    | d in index_set(required)
    where required[d] > available[d]
]
```

Run it against the data file:

```sh
zincite query --program-file shortages.zq staffing.dzn
```

The first line starts with `items`, the top-level items in `staffing.dzn`.
`filter(name("required"))` keeps the assignment named `required`, and `values`
reads its written RHS. `one` requires exactly one match and extracts the array
`[2, 3, 4]`. The `let` statement binds that array to the query name `required`.
The name inside the predicate selects the assignment; the name after `let`
is how the rest of the query refers to the value. `let` binds both input values
and calculations, and a bound name cannot be reassigned. For example,
`let total_required = sum(required);` would calculate the demand total.

The second line reads `available` in the same way. `required[d]` and
`available[d]` then select the values for day `d`. The comprehension visits
the indices in `required`, keeps days with a shortage, and constructs a record
for each one. `(day: d, missing: ...)` names the record's two fields. Inside the
array brackets, `|` introduces the generator and `where` keeps only indices
whose condition is true. For these three-element arrays, the index set is `1..3`.
`values` reads literals written in the data file; it does not evaluate a
MiniZinc expression such as `required = [n * 2 | n in 1..3];`.

The two bindings end with semicolons. The final array expression emits a JSON
report and can end without a semicolon.
The command prints the report with `complete: true`, an empty `limitations`
list and one result row. The row's values are:

| Day | Missing staff |
| --- | --- |
| 3 | 1 |

The JSON uses explicit type tags and array keys; the table above shows the
values rather than the full serialization. The command exits 0 because the
query succeeded. Finding a shortage is the requested result.

Day 3 requires four people and has only three. That directly conflicts with
the two constraints, even though the totals looked sufficient. The query
evaluates the literal data and the comparison we wrote; it does not solve the
model or discover its constraints automatically. For this simple model the
check is enough to expose the problem. A fuller roster could have other
restrictions, such as qualifications or weekly hours.

This query assumes the arrays have the same index set, as they do in the
example. The [demand and capacity report](queries.md#compare-demand-and-capacity)
adds an explicit alignment check and compares totals with individual positions.
The [input-pipeline explanation](queries.md#read-a-named-assignment) also shows
how to interleave reads and calculations, and what `one` checks.

## Further reading

Once the first example works, set up [format on save](format-on-save.md), then
choose [lint rules](linting.md) that address the concerns in your model.
The [query guide](queries.md) covers reports and declaration uses; the
[cookbook](query-cookbook.md) has counts and histograms you can adapt to larger
models. [Data edits](data-edits.md) can help make smaller instances for
investigation.

Zincite's [expression reference](query-comprehension-contract.md) covers array
access, comprehension scope and report output. For further examples of the
shared calculations, see [Zirql's value guide](https://zayenz.se/zirql/guides/values/).
