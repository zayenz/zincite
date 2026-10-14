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
multidimensional access, with explicit axes and retained sparse cells. Data
edits require contiguous axes and complete rectangular coverage.
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

## Compare keyed matrix cells

`entries(a)` exposes each present keyed cell as a `key`/`value` record. Matrix
keys are two-element tuples. For a short report, run either command:

```sh
zincite-query --program 'report entries([|3,4|5,6|]);' crates/zincite-query/tests/fixtures/keyed-comparison.dzn
zincite query --program 'report entries([|3,4|5,6|]);' crates/zincite-query/tests/fixtures/keyed-comparison.dzn
```

The positional report contains keys `(1,1)`, `(1,2)`, `(2,1)`, `(2,2)` and values
`3,4,5,6` in row-major order. `zip_exact(a,b)` pairs cells only when both keyed
arrays have identical ordered axes and present keys. It retains those axes and
keys, with the left and right values in each two-element tuple cell.

The [comparison fixture](../crates/zincite-query/tests/fixtures/keyed-comparison.dzn)
has these matrices:

```minizinc
before = [|3,4|5,6|];
after = [|4,6|8,10|];
```

Its [saved program](../crates/zincite-query/tests/fixtures/keyed-comparison.zq)
computes each cell's delta explicitly:

```text
source a = literal("before");
source b = literal("after");
let pairs = zip_exact(a, b);
report [e.key: e.value.2 - e.value.1 | e in entries(pairs)];
```

Run `zincite query --program-file
crates/zincite-query/tests/fixtures/keyed-comparison.zq
crates/zincite-query/tests/fixtures/keyed-comparison.dzn`, or pass the same
program through `zincite-query --program`. Both return a complete explicit array
with the original two axes `1..2` and deltas `1,2,3,4`. Library callers use
`Program::parse` and `evaluate`, inspect `ReportValue::Constructed` for native
axes, keys and tuple cells, then render the tagged report. Input bytes stay
unchanged. Different axes or key coverage cause a located refusal; a later
failure leaves stdout empty.

The helpers retain enum-member identity and declared empty axes. Empty
`zip_exact` returns its typed keyed array; `entries` returns an empty positional
array. An empty delta comprehension headed by `e.key` cannot infer its result
axes: the existing empty-axis fallback needs simple generator key names. Report
the paired array directly to retain known empty axes.

Read-only keyed arrays may have noncontiguous axes and missing cells.
`entries` returns stored cells; `zip_exact` requires identical ordered axes and
stored keys. Data edits retain contiguous axes and complete rectangular coverage.
Both helpers are report-only and are refused in data-edit expressions and
preceding edit lets. See the [expression contract](query-comprehension-contract.md#report-expressions-and-edit-grammar)
for type, identity and budget rules.

## Group machine demands

The [small data fixture](../crates/zincite-query/tests/fixtures/machine-groups.dzn)
contains four positional records with machine keys `b, a, b, b` and demands
`3, 5, 7, 3`. Grouping keeps both copies of the last `b` row:

```sh
zincite-query --program 'source rows = literal("jobs"); report [(machine:g.key, count:length(g.items), total:sum([r.demand | r in g.items])) | g in group_by(rows, "machine")];' crates/zincite-query/tests/fixtures/machine-groups.dzn
zincite query --program-file crates/zincite-query/tests/fixtures/machine-groups.zq crates/zincite-query/tests/fixtures/machine-groups.dzn
```

Both return `b` with count `3` and total `13`, followed by `a` with count `1`
and total `5`. The [saved program](../crates/zincite-query/tests/fixtures/machine-groups.zq)
uses the same expression as the inline command. The report is complete, with
no limitations, and uses the existing tagged array/record envelope.
The input bytes remain unchanged.

Library callers inspect native data before rendering:

```rust
use zincite_query::{Input, Program, ProgramLimits, ReportValue};
use zincite_syntax::FileMode;

let limits = ProgramLimits::default();
let input = Input::parse(std::fs::read("machine-groups.dzn")?, FileMode::Data)?;
let program = Program::parse(&std::fs::read_to_string("machine-groups.zq")?, limits)?;
let mut result = program.evaluate(&input, limits)?;
let ReportValue::Constructed(summary) = result.emissions()[0].value() else {
    unreachable!("constructed machine summary")
};
// summary is an ordered native Zirql array of records.
let bytes = result.render()?;
```

Without a model, this plain literal source stays positional. A keyed source
needs an explicit conversion such as `[r | r in keyed_rows]` before grouping.
Keys keep their integer, string or enum-member identity. The same grouping
evaluator serves reports built from native projections; their existing
completion status and located limitations pass through. Grouping is excluded
from data-edit expressions and preceding edit lets. See the
[expression contract](query-comprehension-contract.md#report-expressions-and-edit-grammar)
for selector, type and budget rules.

## Correlate declarations with their uses

A projection evaluates its fields against one native subject at a time. This
short program reports each declaration's own name and use count:

```sh
zincite-query --program 'report subtree | declarations | unique | project(d, {"name": d | names | one, "uses": d | uses | count});' model.mzn
zincite query --program 'report subtree | declarations | unique | project(d, {"name": d | names | one, "uses": d | uses | count});' model.mzn
```

`unique` removes repeated declaration handles. Two declarations named
`capacity` in different scopes remain separate rows, with their own counts.
`names` explicitly projects a string; `one` requires exactly one projected
value. A missing or multiple value fails the request.

To retain each declaration and its uses together, save this as `uses.zq`:

```text
let matches = match {
  from d in (subtree | declarations | unique);
  from u in (d | uses);
};
report matches | project(m, {
  "name": m.d | names | one,
  "text": m.u | text | one
});
```

Run `zincite query --program-file uses.zq model.mzn`, or pass the same text
through `--program`. Both commands use the same parser and evaluator. The
[self-contained fixture](../crates/zincite-query/tests/fixtures/native-composition.mzn)
has two declarations with the same name and three uses, including a quoted
`'capacity'` occurrence. Its
[companion program](../crates/zincite-query/tests/fixtures/native-composition.zq)
also reports the native captures. The checks supply an empty local
`std/stdlib.mzn`; they need no MiniZinc installation or solver.

Each `from` follows source order. Later clauses see earlier captures; the
rightmost generator varies fastest. `bind name = query;` requires exactly one
native subject. `where type(d, "int");` and
`where instantiation(d, "par");` use existing semantic facts. Shared
`not`, `and` and `or` preserve Unknown through short-circuit evaluation. An
Unknown required guard fails at its responsible model location. An Unknown
leaf followed by a true `or` alternative can produce a known match, retaining
its limitation and `complete:false`.

Library callers inspect `ReportValue::Captures` through `CaptureMatches::rows`.
Each row has ordered native fields; `ReportValue::fields` borrows a singleton
capture's fields. The `ProgramResult` retains the same `ModelFacts` owner and
source coordinates. Captures and projections share the request's remaining
work, result and nesting allowance, including copies and rendering. Empty
selections still validate field names, scope, types and purity. These forms
are available only in reports; data edits refuse them in preceding lets and
edit operands before preparing a candidate.

## Explain declaration connections

For `paths.mzn` containing `int: c = 1; int: b = c; int: a = b; solve satisfy;`,
this short program retains one witness with native declarations `[a, b, c]`:

```sh
zincite-query --program 'subtree | declarations | filter(name("a")) | paths(referenced_declarations, to=name("c"), max_depth=2) | json' paths.mzn
```

Save the same expression as `paths.zq` and run either command form:

```sh
zincite-query --program-file paths.zq paths.mzn
zincite query --program-file paths.zq paths.mzn
```

The repository has [the model](../crates/zincite-query/tests/fixtures/bounded-paths.mzn)
and [saved program](../crates/zincite-query/tests/fixtures/bounded-paths.zq).
The single expression needs no binding, declaration or final semicolon. A bound
of one gives no witness for `c`; a matching source gives one step at depth zero.
Source and sink remain native singleton declaration streams, and steps contain
both endpoints. Select `p.steps | names` inside `project(p, {...})` to construct
a scalar report from those handles.

Library callers use the same program and retain the model owner:

```rust
let limits = zincite_query::ProgramLimits::default();
let program = zincite_query::Program::parse(program_source, limits)?;
let result = program.evaluate_with_model(&input, Some(&model), limits)?;
let zincite_query::ReportValue::Captures(paths) = result.emissions()[0].value()
    else { unreachable!("native path result") };
let native_fields = &paths.rows()[0].fields;
```

`native_fields` contains `source`, `sink` and `steps` with their DeclarationIds,
file/range coordinates and retained model facts before JSON rendering.
`referenced_declarations` follows proven written references within each owning
region. It does not execute a model or prove a dataflow dependency. First-reference
order chooses between shortest alternatives, and repeated source handles do not
duplicate a source/sink pair.

The explicit edge bound and request limits apply together. Use `--max-work` and
`--max-results` for a smaller allowance, or supply `ProgramLimits` in Rust.
Exhaustion refuses the complete request. No witness within this bound does not
prove global disconnection. Unknown targets refuse; unresolved edges and missing
standard-library context retain completeness limitations and status 1, as in
[semantic inspection](reference.md#semantic-inspection). Source bytes stay unchanged.

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

## Read optional fields and sparse cells

The development query API supports absence and lazy defaults in read-only
reports. These fixture files keep a fixed model schema and sparse stored cells:

```minizinc
% optional-report.mzn
array[1..2,1..2] of opt int: cells;
record(int: id, opt int: missing, opt int: stored): info;
solve satisfy;
```

```minizinc
% optional-report.dzn
cells=[(1,1):10,(1,2):<>,(2,1):30];
info=(id:4,stored:<>);
```

```text
source cells=literal("cells");
source info=literal("info");
report (id: lookup(info,"id") default 0,
        omitted: absent(lookup(info,"missing")),
        stored: has_key(info,"stored"), missing: has_key(info,"missing"),
        stored_cell: has_key(cells,(1,2)), missing_cell: has_key(cells,(2,2)),
        fallback: lookup(cells,(2,2)) default 8,
        skip: 7 default (1 div 0), info: info, cells: cells);
```

Save the program as `optional-report.zq` and run:

```sh
zincite-query --model optional-report.mzn --program-file optional-report.zq optional-report.dzn
```

`stored` and `stored_cell` are true; `missing` and `missing_cell` are false.
The fallback is 8 and `skip` is 7. The tagged output retains the stored absent
field/cell and omits missing storage. A label outside `info`'s declared schema
fails, as does strict `cells[2,2]`. The complete reusable control in
`crates/zincite-query/tests/fixtures/optional-report.zq` also includes a native
count and checks that false, zero and empty arrays remain present.

The Rust `Program::parse` and `evaluate_with_model` entry points accept the same
program. Constructed results keep their declaration annotations; use `.data()`
when matching the stored data. These values do not expand data editing or
external parameter types. Data edits still require nonoptional targets and
complete rectangular arrays.
