# Bounded comprehensions for reports and data

`set_value("EXPRESSION")` inserts MiniZinc expression source into an existing
assignment. `values` reads written literals, but does not evaluate a computed
right-hand side. Neither operation constructs a new value from query results.
Read-only [query programs](reference.md#query-programs) use immutable `let`
bindings for input values and calculations.
They support required scalar parameters, ordered query statements, native
reports and the bounded expression/comprehension subset below. Zincite lowers its existing CST and uses
Zirql for checked expressions, scopes, generators, array summaries, requirements
and keyed helpers. Read-only programs also accept [typed query functions](reference.md#typed-query-functions) with explicit native inputs and bounded direct or mutual recursion.
Model-backed `.dzn` edit batches use the common expression subset and evaluator;
the report-only calls below are excluded from every edit expression.

Reports accept positional and sparse keyed arrays, matrices, tuples, ordered
records and homogeneous Boolean/integer/float/nominal sets. Sources may use
existing query projections or one unique original literal assignment. Nested
arrays stay nested. Integer starting-key tails use checked successors; enum
starting-key tails are refused. `index_set`
returns established rank-one keys. `enum_member` resolves explicit declared
members through a retained read-only model; use `--model` for `.dzn` programs.
Input bindings may establish enum alignment and empty axes from written
integer intervals or explicit enums. Bounded integer axis endpoints may read unique fixed integer literal assignments;
computed dependency values and uncertain axes are refused.
Native-only reports keep their existing untagged result; pure reports use the
tagged schema in this contract.

The expression syntax follows the [MiniZinc 2.10.1 comprehension and generator
grammar](https://docs.minizinc.dev/en/2.10.1/spec.html#set-comprehensions).
MiniZinc itself permits more expressions and functions. This contract accepts
only the subset stated here; syntactic acceptance by Zincite's parser alone
does not imply that the constructor can evaluate an expression.

## Native report composition

Reports may use direct native pipeline stages and the shared `project(subject, {fields})`
and `match { from ...; bind ...; where ...; }` forms described in the
[query reference](reference.md#native-projections-and-captures). Their quoted
projection fields use the shared value reader and explicit native scalar
projections. Ordinary MiniZinc expressions, records and comprehensions retain
the CST frontend described below. Native captures retain declaration/reference
identity and their model owner; they are never constructed scalar handles.

The shared evaluator owns capture scope, dependent generator order,
exact-one bindings, field lookup and correlated projection. The adapter supplies
Zincite stages, semantic predicate leaves and scalar conversion. A match guard
keeps Unknown until shared Boolean control finishes: a final Unknown refuses
at the responsible model location, while a known true alternative can retain
a row with its located limitation. Empty capture collections retain their field
schema. Candidate admission, copies, metadata and rendering use the same
continuing request budget as other reports.

The shared `paths(referenced_declarations, to=predicate, max_depth=n)` form also
returns native captures. Source, sink and declaration steps retain the model
owner and original coordinates. The [path reference](reference.md#bounded-declaration-paths)
defines shortest witnesses, edge bounds, Unknown refusal and incompleteness.
Its traversal, native copies, field projection and rendering spend the same
program budget. The final expression emits its result and may end at EOF without `;`.
Bindings and earlier statements require terminators; edits end with `emit;`.

These forms are excluded from data expressions and preceding edit lets. The
closed MiniZinc data grammar, complete-candidate validation and atomic write
rules below continue to apply.

## Report expressions and edit grammar

Read-only requests may interleave parameters, immutable bindings and output
expressions. Names become visible in statement order. `let` binds both input
values and calculations. For a data assignment `required = [2, 3, 4];`:

```zirql
let required = items | filter(name("required")) | values | one;
sum(required)
```

`items` selects top-level input items, `filter` keeps the named assignment,
`values` reads its literal RHS, and `one` extracts exactly one projected value.
Zero or multiple matches fail. Written array keys and declared axes or nominal
identities survive the conversion; use `--model` for declaration evidence in
`.dzn` reports. Computed RHSs are refused.

Expressions outside bindings emit their results. Native selections keep source
output and native counts print as numbers; calculated values and records produce
JSON. The final expression's semicolon is optional. `report expression;`
explicitly emits JSON for a native result, including intermediate inspection
results. Use `let` for an intermediate calculation that should not print.

A data request ends in one atomic edit batch and cannot include output
expressions. Input reads and calculations may interleave before the batch;
every read uses the original input. Native data reads admit `items`, `filter`,
`head`, `range`, `values`, `names`, `count`, `unique`, `fields`, `elements` and
`keys`, followed by shared `one` when exactly one projected value is needed.
Semantic filters, `project`, `match` and paths remain report-only. Calculations
and native predicate arguments retain the closed data-expression policy below;
report-only calls are refused even in unused bindings or unchosen branches.
Each visible binding and parameter name must be unique. Parameters follow the
[required input rules](reference.md#required-query-parameters).

```ebnf
request        = (parameter | binding | output ";")* output [";"]
               | (parameter | binding)* edit-batch ;
parameter      = "param" ascii-ident ":" ("int" | "string" | "bool") ";" ;
binding        = "let" ident "=" expression ";" ;
output         = ["report"] expression ;
edit-batch     = edit+ ("emit" | "emit_document") ";" ;
edit           = ("create" | "replace") ident "=" expression ";" ;

array-comp     = "[" expression "|" generators "]" ;
indexed-comp   = "[" key ":" expression "|" generators "]" ;
set-comp       = "{" expression "|" generators "}" ;
key            = expression | "(" expression ("," expression)+ ")" ;
generators     = generator ("," generator)* ;
generator      = names "in" expression ("where" expression)?
               | binder "=" expression ("where" expression)? ;
names          = binder ("," binder)* ;
binder         = ident | "_" ;
ident          = MiniZinc identifier or quoted identifier ;
ascii-ident    = ASCII letter or "_", followed by ASCII letters, digits or "_" ;
```

The brackets, key prefix and generator forms match the pinned [array and
indexed array comprehensions](https://docs.minizinc.dev/en/2.10.1/spec.html#indexed-array-comprehensions).
A generator's `in` source must be a finite constructed or source-backed array,
a fixed set with the supported element types below, or a finite integer range.
Set iteration uses type order: `false` before `true`, numeric order for numbers
and declaration order for members. An `in` generator yields **values**;
`a in records` does not yield keys or `(key, value)` pairs. Use `index_set(records)` and
`records[i]` for rank-one keys, or `entries(records)` to read full keys and values
from any keyed rank. Multiple names before `in` each range over the
same source, with the last name varying fastest. A single-name `=` generator
evaluates its right side once for each preceding binding. Earlier generator
bindings are visible in later sources and filters; a generator's own bindings
are visible in its `where` and the head. They are not visible in that
generator's source. The rightmost generator is innermost. `_` discards a value
and cannot be referenced. Duplicate bindings, shadowing a source, `let` or
visible parameter, and forward references fail at the name. Filters require a
fixed Boolean.
An absent value cannot act as a filter; decision-dependent Booleans remain unsupported. These
scope and iteration rules follow the [specification's generator
rules](https://docs.minizinc.dev/en/2.10.1/spec.html#set-comprehensions).

Calculated `expression` values use MiniZinc 2.10.1 syntax parsed by Zincite's
existing CST, then checked against the following closed subset. Native input
pipelines use the shared reader and the bounded read stages above. Expressions may contain
signed integer, decimal float, Boolean, string and absent (`<>`)
literals; bound names; parenthesized expressions; field access (`r.field`),
tuple access (`t.1`), array access (`a[i]` or `a[i,j]`); finite integer ranges
(`lo..hi`); set, tuple, record, positional array and fully keyed array
literals; the three comprehensions above; and `if ... then ... elseif ... then
... else ... endif`. The `else` is mandatory in this subset. Records use
MiniZinc labels, for example `(weight: 7, peer: 2)`; a label is not a string or
member. The common expression calls are `index_set(a)` for a one-dimensional
array's established keys and the host helper
`enum_member(EnumName, "MemberName")` for an explicit enum member identity.
`enum_member` requires a unique model enum declaration and a
member already declared there. Its arguments are an enum identifier and a
string literal, not computed expressions. Written member references in source
literals retain their resolved identity when the model establishes one. Member
names and generated assignment names must have valid MiniZinc identifier
spelling (quoted when required). Serialization uses the declaration's member
spelling; it never replaces matching text inside strings or field labels.

Reports additionally accept `length(a)`, `sum(a)`, `forall(a)`, `exists(a)`,
`require(condition, value, "message")`, `group_by(rows, "field")`, `entries(a)`,
`zip_exact(a,b)`, `contains(text, fragment)`, `starts_with(text, prefix)` and
`ends_with(text, suffix)`, plus report ordering and optional extrema below.

String matching accepts two strings and returns a fixed Boolean. Matching is
exact and case-sensitive, without coercion, regex, case folding or Unicode
normalization. Empty search strings match, including an empty text. Both
operands evaluate in written order; known non-string operands fail preflight,
including in skipped branches. Scans and failure diagnostics use the active
request budget, and matching borrows existing text. These helpers remain
report-only, including in unused lets and skipped data-edit branches.

Each reducer takes one ordinary array expression. The array materializes fully
before reduction; generator-call spellings such as `forall(i in 1..3)(...)` are
unsupported. Length counts present
values without flattening nested arrays. Sum accepts only integers and performs
checked addition in stored order. The quantifiers require fixed Booleans and
scan every stored element. Positional and keyed arrays use the same reducers;
keyed reducers visit stored cells, including in sparse arrays. Generic axis and
key invariants remain checked. Data edits require contiguous axes and complete
rectangular coverage.

| Call | Empty array result |
| --- | --- |
| `length([])` | `0` |
| `sum([])` | `0` |
| `forall([])` | `true` |
| `exists([])` | `false` |

Require takes exactly three arguments, with a string literal as its message.
It preflights every operand, name and known type. Runtime evaluates its condition
as a fixed Boolean. True evaluates and returns the value; false fails at the
require call with the supplied message. False skips value arithmetic, access,
construction and runtime host checks, while source projection and actual binding
type discovery may still inspect or copy referenced values during preflight.
A skipped operand cannot hide unsupported syntax or a known type mismatch.

`sort(xs)` and `sort_by(rows, "field")` stably order positional scalar values
or original records. They are ascending unless the final literal option is
`descending=true`; `descending=false` is the default. Descending ties retain
input order. The selector is one static string literal. This local option uses
`=`, not a general named-argument or equality expression.

`min(xs)`/`max(xs)` return an optional scalar, and `min_by(rows, "field")`/
`max_by(rows, "field")` an optional original record. Empty input returns `<>`;
the first input wins a tie. Use `min([]) default 0` for an explicit empty policy.
Keys require present homogeneous integers, finite floats, exact strings or one
nominal type. Integers and floats do not mix; nominal members use declaration
order. Other row fields, declarations and member identities survive. A missing,
absent or incompatible field fails at the selector, including a later bad key
in extrema. Project a `lookup(row, "score") default 0` key first when intended.
Keyed arrays need explicit positional conversion, such as `[e.value | e in
entries(a)]`.

All six calls are report-only, including unused lets and skipped edit branches.
Data source/target admission and atomic edits retain their closed policy.
Sorting extracts keys once, admits O(n) scratch and uses fallible O(n log n)
comparisons. Extrema scan once and select only the winner without a whole-input
copy. Field/string/nominal inspection, scratch, comparisons and owned output
copies share the continuing request budget; refusal yields no partial output.
See the [ordering report](queries.md#sort-rows-and-select-extrema).

`group_by` takes exactly two arguments: a positional record array and one string
literal field selector. Keys must be homogeneous integers, strings or resolved
members of one enum. It returns positional `key`/`items` records in first-seen
key order; each items array retains row order and duplicates. Other row fields
may differ. Empty input returns an empty array. Missing fields fail when
executed, while known incompatible keys fail preflight even in skipped branches.
Key refusals point to the selector; non-array input and non-record rows point
to the call. Keyed arrays need an explicit positional comprehension conversion.

`entries` takes exactly one keyed array and returns positional records with
`key` followed by `value` in present-cell order. Rank-one keys are scalar integers
or resolved members; higher-rank keys are tuples in dimension order. Empty
input gives an ordinary empty positional array. The source retains its axes.

`zip_exact` takes exactly two keyed arrays whose ordered axes and present full
keys are identical. It returns a keyed array of two-element tuple cells, left
value followed by right value, retaining left axes and keys. Tuple components
may have different value types. Equal empty axes are retained. Different rank,
axis types/order, integer keys, nominal identity/metadata or key coverage causes
a refusal; equal counts are insufficient. There is no truncation, filling,
reindexing or data write. Both operands run once, left before right. Known input
kinds and axis rank/types are preflighted in every branch; runtime alignment
errors use the call range. Child errors keep their responsible ranges.

Read-only keyed operands may have noncontiguous axes and missing cells. Helpers
retain their stored keys; they do not fill holes. Data edits still require
contiguous axes and rectangular coverage. An empty indexed delta comprehension headed by
`e.key` cannot infer axes after `entries`; the existing fallback needs simple
generator key names. Report `zip_exact` directly to retain declared empty axes.
The [matrix example](queries.md#compare-keyed-matrix-cells) computes nonempty
cell deltas explicitly.

These calls are reporting operations only. An edit program refuses them in
preceding `let` bindings, including unused lets and skipped branches, and in every
`create` or `replace` RHS. Data expressions retain exactly `index_set` and
`enum_member` as their call whitelist. Sources, binding order, target checks and
the atomic candidate rules below remain the data contract.

The operators below use the [MiniZinc 2.10.1 precedence and associativity
table](https://docs.minizinc.dev/en/2.10.1/spec.html#operators). Parentheses
can remove any ambiguity. Operations reject incompatible types rather than
coercing strings, record labels, members or keys.

| Form | Accepted operands and result |
| --- | --- |
| `+x`, `-x` | Numeric sign; checked for integer overflow. |
| `not x` | Boolean negation. |
| `*`, `+`, `-` | Two integers give a checked integer; two floats give a finite float. Mixed numeric operands require exact integer-to-binary64 conversion and give a float. |
| `div`, `mod` | Two integers, with MiniZinc's truncation-toward-zero quotient and remainder sign of the dividend. |
| `/` | Two floats, or exact numeric promotion to floats; finite result only. |
| `<`, `<=`, `>`, `>=` | Two compatible numeric values, or two integer keys. Boolean result. |
| `=`, `!=` | Equal types: numeric values, Booleans, strings, resolved members of the same enum, or recursively compatible constructed values. Boolean result. |
| `in` | Scalar membership in a supported finite set or integer range, with compatible element type. |
| `/\`, `\/` | Booleans; evaluate the right side only when needed. |
| `++` | Two strings, or two positional one-dimensional arrays; arrays receive fresh `1..n` positions. No implicit concatenation of keyed arrays. |

Array and record access evaluates only an existing key or field. Tuple access
uses a positive one-based position. `if` and `elseif` evaluate conditions in
order, then only the chosen branch. A false `where` skips the head. An
unevaluated branch cannot suppress a parse, type or unsupported-form error;
those checks run before evaluation. At runtime a skipped branch does not
produce arithmetic or missing-key errors. Set construction, set generators and
set membership accept only homogeneous fixed Booleans, signed integers, finite
floats or resolved members of one enum. A constructed set removes duplicates
by typed identity and renders in type order. A string set, or a set with another
unsupported element, fails at the set expression or source literal range before report or `.dzn`
serialization. Strings remain ordinary scalars, array elements and `++`
operands. The [pinned specification](https://docs.minizinc.dev/en/2.10.1/spec.html#sets)
permits broader fixed sets, but the tested MiniZinc 2.10.1 compiler rejects
string set literals. This contract excludes them for both consumers.

Model calls, arbitrary built-ins, generator calls, interpolation, annotations,
`let` inside a MiniZinc expression, decision values,
floating ranges and every unlisted operator fail with a located explanation.

Named query functions are the exception to the arbitrary-call exclusion.
Their declarations use the common typed header and local-let structure, while
bodies lower through the same MiniZinc CST adapter as report expressions.
Functions support located tuple/record patterns in parameters and immutable
bindings, guarded `match(subject)` values and lexical `if ... { ... } else
{ ... }` blocks. MiniZinc conditionals, records and comprehensions retain their
CST lowering. The shared prepared runner owns matching, scope, Boolean guard
admission, lazy selected bodies and tail calls. Native record fields use
explicit capture views and retain their input owners and coordinates. These
forms, including unused lets and skipped branches, remain report-only. Data
requests retain their closed grammar and atomic edit contract. See the
[patterns reference](reference.md#patterns-and-pure-control-values) and
[complete examples](queries.md#destructure-a-report).

Functions do not execute model functions. They share the prepared value runner,
work/result allowances and native callbacks described in the
[function reference](reference.md#typed-query-functions). Atomic Data programs
continue to reject every function declaration and call, even when unused.

Numbers use signed 64-bit integers and finite binary64 decimal floats. Integer
literals may use MiniZinc decimal, `0x` or `0o` spelling, including
`-9223372036854775808`; hexadecimal floats are unsupported. Integer
arithmetic, range endpoints and index arithmetic fail on overflow. `div`,
`mod` and `/` fail on zero divisors; `i64::MIN div -1` and the matching `mod`
case fail on overflow. A nonzero decimal literal that rounds to zero fails,
as does any nonfinite literal or result. Conversion of an integer to a float
fails if binary64 cannot represent it exactly, as in the existing
`filter_elements` numeric comparison. Floating arithmetic also refuses a
nonzero finite mathematical result rounded to zero. No implicit string-to-number
or string-to-member conversion is allowed.

## Optional read-only values

`<>` represents absence. `occurs(value)` and `absent(value)` inspect presence;
zero, false, empty strings and empty containers remain present. Arithmetic,
ordering and filters require present operands. Optional equality checks known
underlying declarations before comparing presence.

`lookup(value, key)` reads a record string label, a one-based positional index,
or a scalar/tuple keyed-array key. A compatible missing key returns absence.
`has_key(value, key)` reports whether storage contains the key, so a stored
`<>` counts as present storage. Wrong containers, key types, ranks and enum
identities fail. With a retained record declaration, an undeclared label also
fails. Ordinary dot and bracket accesses still fail on missing data.

`value default fallback` evaluates its left operand once and selects the
fallback only for absence. Both branches receive syntax, scope, purity and
known-type checks. A fallback-only binding is not projected, scanned or copied,
and a fallback-only enum member is not resolved, until selected. Selected errors
keep their original locations and share the request budget. Parenthesize mixed
operators; `default` follows MiniZinc precedence and associates left to right.

Fixed optional model sources retain their actual TypeInst contracts, including
required and optional record labels. Omitted optional fields are permitted;
missing required fields, extra labels and incompatible values fail admission.
Zincite keeps these declarations with native constructed values; Rust callers
can inspect data through `ConstructedValue::data()` without discarding the
stored contract. Decision and computed source values remain unsupported.

Tagged reports render absence as `{"kind":"absent"}`. Records emit only stored
fields and keyed arrays only stored cells. Data requests refuse absence,
optional source/target contracts and these new forms, including unused lets and
skipped branches. External parameters remain required int/string/bool inputs.
See the [optional report](queries.md#read-optional-fields-and-sparse-cells).

## Constructed shape and result

`[value | ...]` has positions `1..n` in generation order, including after a
filter. Fully keyed `[key: value | ...]` carries one integer or resolved enum
key per dimension. A tuple key establishes its arity. Keys must be unique;
integer axes retain numeric order and enum axes retain known identity and
declaration order. Read-only arrays may omit axis keys or cells. Omission does
not shift stored keys; data targets retain the contiguous rectangular policy. Keyed sources whose member names are merely syntactic
identifiers need model facts before they can establish enum identity. The
existing literal filter's syntactic key check alone cannot prove that identity
or a target declaration's coverage.

Iteration over a multidimensional array yields its **values** in row-major
axis order. `a[i,j]` uses a multidimensional key. `a[i][j]` accesses a nested
array and may have a different length for each outer value. A nested array is
not silently flattened into a multidimensional array. Serialize two-dimensional
integer axes as a matrix when their established keys and values fit that form;
otherwise use fully keyed literals in row-major key order. More dimensions use
fully keyed literals. Report JSON keeps explicit axes and keys even when `.dzn`
uses a matrix. Empty positional arrays and sets are valid. Refuse an empty
keyed or multidimensional result when its dimensions, enum identity or target
coverage cannot be established from a declared target or explicit source axes.
An empty fixed-size target still must match its declared axes. Shape and type
checks are target-aware for edits; a report may retain a known empty axis.

Constructed values live in a native value type separate from `LiteralValue`,
whose `file`, `range` and `spelling` describe written source. A report renders
one JSON object with `complete`, `limitations` and `result`. The `result` is
one of these tagged values: `{"kind":"integer","value":7}` (similarly
`float`, `boolean`, `string`), `{"kind":"member","enum":"Guests","name":"A"}`,
`{"kind":"set","elements":[...]}`, `{"kind":"tuple","elements":[...]}`,
`{"kind":"record","fields":[{"label":"weight","value":...}]}`, or
`{"kind":"array","indexing":"positional"|"explicit","axes":[[key,...],...],"entries":[{"key":[key,...],"value":...}]}`.
Array keys use the scalar integer/member forms. Positional arrays expose their
`1..n` keys here too; every axis is ordered. A complete report has
`"complete":true,"limitations":[]`. A report derived from incomplete semantic
inspection keeps `complete:false` and its existing located limitation objects.
Newly computed values do not invent source ranges. Source-backed query items
may retain their existing file and range in a separate inspected result.

For one assignment named `counts`, the second report example renders this
complete object (shown with line breaks for readability):

```json
{
  "complete": true,
  "limitations": [],
  "result": {
    "kind": "array",
    "indexing": "explicit",
    "axes": [[{"kind": "integer", "value": 1}]],
    "entries": [{
      "key": [{"kind": "integer", "value": 1}],
      "value": {"kind": "record", "fields": [{
        "label": "name",
        "value": {"kind": "string", "value": "counts"}
      }]}
    }]
  }
}
```

## Reports and data edit requests

Both reports are implemented. They use the current `names` projection as a
source of strings; the keyed report preserves explicit positions.

```zirql
let names = filter(kind("assignment")) | names;
[(name: n, label: "data:" ++ n) | n in names]
```

```zirql
let names = filter(kind("assignment")) | names;
[i: (name: names[i]) | i in index_set(names)]
```

Native input pipelines project a `Strings` result to a positional array of
strings, `Count` to an integer, and `Literals` to a positional array of
source-backed literal values. For one named RHS, use
`items | filter(name("counts")) | values | one`; `one` checks the projection's
cardinality and extracts the value. Item, node and semantic selections need an
explicit, typed projection before use in a calculated expression;
an ambiguous native result, `Tally`, `Json`, `Document` or `Reduction` is refused. Report
construction over semantic results carries their completion status. A queried
name is a string; it never becomes an enum member or record label by spelling.

For a `.dzn` containing `counts = [3, 5];`, with `counts` and
`doubled_counts` both declared as `array[1..2] of int` in a read-only model,
these requests make different kinds of data edit:

```zirql
let old_counts = items | filter(name("counts")) | values | one;
create doubled_counts = [old_counts[i] * 2 | i in index_set(old_counts)];
emit;
```

```zirql
let old_counts = items | filter(name("counts")) | values | one;
replace counts = [old_counts[i] + 1 | i in index_set(old_counts)];
emit;
```

The first adds `doubled_counts = [6, 10];`; the second replaces `counts` with
`[4, 6]`. Neither is equivalent to `set_value("[old_counts[i] * 2 | ...]")`,
which only inserts expression text. `create` requires an absent assignment
name and a unique compatible model declaration. `replace` requires exactly one
existing assignment. Duplicate targets fail; a request cannot create and
replace the same name. All bindings read the original input,
never a preceding edit in the same request. Creation appends a complete item
after the last assignment, before trailing detached comments, using the input's
line ending. An edit never changes model or include files. Without unique
model ownership, type and axis obligations for a new or changed assignment
remain unknown and the edit is refused.

An edit program constructs one atomic batch. It checks supported target types,
written finite integer scalar domains and array shapes using read-only model
declarations and the coherent candidate assignments after all edits. Thus the
doubled example requires `entities` length 4, `groups` length 2 and
`capacities` shape `4 × 2`, while input bindings still read the original
`2`, `1` and `2 × 2` values. Resolve only supported literal scalar assignments
and bounded declaration-axis expressions. A computed, ambiguous or unresolved
axis dependency refuses the whole batch at its source range; this check does
not establish full MiniZinc type validity or satisfiability.

Replacement changes only the original RHS range. An RHS with internal comments
is refused until the caller chooses their placement; generated values inherit
no comments. Existing comments outside replaced ranges, whitespace and all
other source bytes stay in place. Created items have no inherited comments;
creation fails if its placement would attach a directive to a different item.
The entire batch uses original byte coordinates, rejects overlaps and directive
damage, reparses the complete `.dzn` candidate, and uses the existing diff/write
preconditions. There is no partial output or replacement on an error.

## A complete doubled instance

Here doubling creates a second copy of each entity **and** updates its peer,
group, matrix row and group membership. Multiplying `entity_count` or the
weights by two would leave references pointing at the wrong entities.

The public model and input data are:

```minizinc title="doubled.mzn"
int: entity_count;
int: group_count;
array[1..entity_count] of record(int: weight, int: peer, int: group): entities;
array[1..entity_count, 1..2] of int: capacities;
array[1..group_count] of set of int: groups;
solve satisfy;
```

```minizinc title="doubled.dzn"
entity_count = 2;
group_count = 1;
entities = [(weight: 7, peer: 2, group: 1), (weight: 11, peer: 1, group: 1)];
capacities = [| 3, 4 | 5, 6 |];
groups = [{1, 2}];
```

The request names the two maps. For copy `c` in `0..1`, an old entity `i`
becomes `entity_map[i,c] = i + 2*c`, and an old group `g` becomes
`group_map[g,c] = g + c`. Each input name binds an original literal value.
The five replacements form one candidate.

```zirql title="double.zq"
let old_entities = items | filter(name("entities")) | values | one;
let old_capacities = items | filter(name("capacities")) | values | one;
let old_groups = items | filter(name("groups")) | values | one;
let entity_map = [(i,c): i + 2*c | i in 1..2, c in 0..1];
let group_map = [(g,c): g + c | g in 1..1, c in 0..1];
replace entity_count = 4;
replace group_count = 2;
replace entities = [
    (weight: old_entities[i].weight,
     peer: entity_map[old_entities[i].peer,c],
     group: group_map[old_entities[i].group,c])
  | c in 0..1, i in 1..2
];
replace capacities = [
    (entity_map[i,c],j): old_capacities[i,j]
  | c in 0..1, i in 1..2, j in 1..2
];
replace groups = [
    group_map[g,c]: {entity_map[i,c] | i in old_groups[g]}
  | c in 0..1, g in 1..1
];
emit;
```

| Old key | Copy 0 | Copy 1 |
| --- | ---: | ---: |
| Entity 1 | 1 | 3 |
| Entity 2 | 2 | 4 |
| Group 1 | 1 | 2 |

The resulting complete `.dzn` is:

```minizinc
entity_count = 4;
group_count = 2;
entities = [
    (weight: 7, peer: 2, group: 1),
    (weight: 11, peer: 1, group: 1),
    (weight: 7, peer: 4, group: 2),
    (weight: 11, peer: 3, group: 2)
];
capacities = [| 3, 4 | 5, 6 | 3, 4 | 5, 6 |];
groups = [1: {1, 2}, 2: {3, 4}];
```

The four entity records retain weights `7, 11, 7, 11`; peer references are
`2, 1, 4, 3`, and group references are `1, 1, 2, 2`. Capacity keys cover
`1..4 × 1..2`, with each old two-column row copied unchanged. Group keys
cover `1..2`, with memberships `{1,2}` and `{3,4}`. If a source also had a
matrix indexed by **two** entity axes, the caller would have to state whether
cross-copy pairs exist and what values they take. The constructor must not
infer that policy from the one-axis copies above.

Integer positions are not enum identities. Suppose the read-only model
declares `enum Guests = {A, B, A_copy, B_copy};`. This report binding names
both sides of a separate member mapping:

```zirql
let guest_map = [
    enum_member(Guests, "A"): enum_member(Guests, "A_copy"),
    enum_member(Guests, "B"): enum_member(Guests, "B_copy")
];
```

Changing a string `"A"` or an unrelated record label `A` would be wrong.
This bounded constructor does not invent enum declarations or names. An
absent or ambiguous member declaration refuses the mapping.

## Limits, diagnostics and implementation seam

One shared budget covers input reads, calculated bindings, generator
candidates **before** filters, expression visits, nested values and keys,
every copied byte, edit construction and final rendering. The recommended
defaults reuse `Limits`: at most 64 nested expression/collection levels,
1,000,000 work units and 100,000 aggregate produced entries across the
request. A query source's model scan keeps its separate pre-allocation
admission; raw loaded tokens do not count as produced report entries. Check
products and byte lengths with overflow-safe arithmetic before allocating.
Each repeated generator expansion and repeated serialization is charged again.
On exhaustion, return a located limit error; never truncate a report or edit.

Reducers pay full operand materialization, generator candidates and copies before
scanning. Sum, forall and exists charge one work unit per inspected element;
length charges one for the present-value count and checked `i64` conversion.
Each summary scalar adds one aggregate result unit. Empty summaries still pay
expression/preflight work and their scalar unit. Require preflight charges its
literal message bytes; false diagnostic copying charges bytes before allocation.
True require moves the admitted child with no additional result or copy charge.
Emission snapshots and rendering charge again. Arithmetic errors retain their
child ranges; requirement failures retain their call range and limit failures
the active operation range.

Grouping evaluates rows once and charges row/depth scans, field/key comparisons,
nominal metadata checks, new containers and slots, and copied group keys/labels.
Rows move into items without another deep copy. Inferred group facts retain
possible row schemas, including schema-bearing empty projections, without
claiming group membership or count. Later snapshots and rendering spend the
same allowance. Completion and limitations remain with the report adapter.

Entries and exact pairing charge ordered axis/full-key comparisons, nominal
metadata checks, type facts, new containers and slots, copied labels and depth
scans. Evaluated cell values move into records or tuples without another deep
copy; pairing retains admitted left axes and keys. Later snapshots and rendering
spend the same allowance. Completion and limitations remain with the existing
report adapter.

Native projections prepare their located bodies and embedded primitive arguments
once per invocation. The CST frontend lowers those arguments into the same value
tree; selected enum members still resolve through the current ModelFacts owner.
Written branches retain scope/type/purity validation, and an unselected default
fallback leaves its data and member resolution untouched. Native operations run
fresh for the current subject and keep their source coordinates and completion
status.

A selected CST input projection can retain one immutable owner in its current
input slot. Repeated reads inspect that owner; they charge reference/slot work
without deep copied bytes or recursively produced entries. New selected data
still receives admission. Owned fields, explicit copies, emission snapshots,
data serialization and rendering retain their materialization charges. Preparation
and scope metadata are charged before growth; dropping a frame never refunds
spending. The work/result/nesting defaults are unchanged. Lower repeated analysis
and copy charges can let a request near an earlier threshold complete or move
its located limit refusal. DATA grammar, target admission, candidate reparse and
atomic writes retain their existing rules.

Parse errors, unsupported expressions, wrong types, missing fields or keys,
duplicate keys/targets and arithmetic failures identify the host query byte
range or the original source file/range responsible. For a generated value,
report the expression range. An uncertain literal, unresolved enum identity,
unknown dependency or unsupported computed source value cannot be guessed into
an edit. A semantic report may use known rows while retaining `complete:false`
and located limitations. A filter with an unknown truth value fails at that
condition even under `not`, so negation never turns uncertainty into a match.
Incomplete candidates may be previewed only where the existing best-effort
contract permits them; in-place write requires completeness. Any actual error
buffers away the whole report or candidate.

Zincite lowers its existing CST and projects native query/literal results into
Zirql's bounded evaluator:

```rust
zirql::evaluate_value(expression, bindings, shared_budget)
    -> Result<ConstructedValue, QueryError>
```

The report adapter supplies typed query results and renders the native value.
The data adapter supplies literal RHS values, checks model ownership and target
shape, serializes constructed MiniZinc literals, then calls the existing atomic
edit preparation and complete-candidate reparse. Both adapters share the common subset, evaluator and budget. Reporting also
admits summaries, require, grouping and keyed helpers; data lowering excludes
those calls. Edit expressions are
validated at parse time and lowered again with declared target evidence during
evaluation; this additional work spends the same request budget. Zincite has one CST frontend and uses the existing
Zirql dependency for shared evaluation. It does not run a model, compiler or solver.

The user approved this exact bounded data contract on 2026-10-13. The library
and both query commands implement it. Use `--model` to supply read-only target
declarations, `--diff` to inspect changes, and `--write` only for an explicit
regular data file. Model and include files are never edited. A candidate's type
and axis checks cover this subset; they do not prove full MiniZinc validity or
satisfiability.
