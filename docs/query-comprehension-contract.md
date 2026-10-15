# Query expressions and data construction

A query program can calculate values from MiniZinc data and either return a
JSON report or construct a new `.dzn` file. It reads literal assignments,
binds their values with `let`, and uses expressions and comprehensions to build
the result. It does not evaluate the MiniZinc model or run a solver.

This page defines the supported expressions, result formats and edit rules.
Start with [query reports](queries.md) or [data edits](data-edits.md) for a
walkthrough. The [doubled-instance example](#a-complete-doubled-instance) below
shows how to copy records while updating their indices and references together.

Zincite uses [Zirql](https://zayenz.se/zirql/) to evaluate the queries. Its
expressions follow MiniZinc 2.10.1 syntax where described here, but support only
the operations listed on this page. A MiniZinc expression can parse correctly
and still be unsupported in a query.

## Combine source selections in a report

Source selections and semantic queries can feed a report directly. Use
`project(subject, {fields})` to calculate fields for each selected item, or
`match { from ...; bind ...; where ...; }` to relate declarations to their uses.
The [projection examples](queries.md#correlate-declarations-with-their-uses)
show both forms. Selections keep their declaration identity and source
locations throughout the report.

A guard that remains unknown fails at the model location responsible. A known
true alternative can still keep a row, with any analysis limitations attached.
Empty matches keep their field names so later projections can be checked.

`paths(referenced_declarations, to=predicate, max_depth=n)` finds connections
between declarations. Each result contains a source, a sink and the declarations
along one shortest path between them. The [path reference](reference.md#bounded-declaration-paths)
defines the depth bound, traversal order and incomplete-result behaviour.

These operations are available in read-only reports. Data-edit programs support
the narrower set of input stages and expressions below.

## Common brace forms in read-only reports

Read-only reports also accept `{key: value}` dictionaries and
`{key: value for name in source if condition}` comprehensions. Keys are
expressions that must return present strings. Each key is checked for a
duplicate before its value runs. The dictionary keeps successful generation
order and uses the same record data and declared schemas as quoted static
fields. `d[key]` and `d.field` fail on a missing stored field; `lookup(d,key)`
returns absence for a valid missing key.

Bare sets accept `{3, 1, 3}` and `{n mod 3 for n in 1..5}`. They keep `3, 1`
and `1, 2, 0` respectively. Empty `{}` is a dictionary in this report syntax;
`set<int>()`, `set<bool>()` and `set<float>()` supply typed empty scalar sets.
Nominal identities and empty nominal sets remain supplied by the host. String
sets remain unsupported. Arrays, tuples, grouping and function/control blocks
compose with these values; a final collection inside a block has its own braces.

Natural MiniZinc records and `|` comprehensions retain the forms below. The
common dictionary and typed empty forms are read-only report syntax. Data edits
retain the closed MiniZinc subset and serialization checks below.

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
every read uses the original input. Native data reads support `items`, `filter`,
`head`, `range`, `values`, `names`, `count`, `unique`, `fields`, `elements` and
`keys`, followed by shared `one` when exactly one projected value is needed.
Semantic filters, `project`, `match` and paths remain report-only. Calculations
and native predicate arguments retain the closed data-expression policy below;
report-only calls are refused even in unused bindings or unchosen branches.
Each visible binding and parameter name must be unique. Parameters follow the
[required input rules](reference.md#required-query-parameters). The grammar below
covers basic report statements and data edits; read-only [patterns and
functions](reference.md#patterns-and-pure-control-values) have additional syntax.

```text title="Basic grammar"
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
Sets iterate in first-insertion order. Explicit `sort` uses `false` before
`true`, numeric order for numbers and declaration order for members. An `in` generator yields **values**;
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

Calculated values support the following MiniZinc 2.10.1 expressions:
signed integer, decimal float, Boolean, string and absent (`<>`)
literals; bound names; parenthesized expressions; field access (`r.field`),
tuple access (`t.1`), array access (`a[i]` or `a[i,j]`); finite integer ranges
(`lo..hi`); set, tuple, record, positional array and fully keyed array
literals; the three comprehensions above; and `if ... then ... elseif ... then
... else ... endif`. The `else` is mandatory in this subset. Records use
MiniZinc labels, for example `(weight: 7, peer: 2)`; a label is not a string or
member. Both reports and data edits accept `index_set(a)` for a one-dimensional
array's keys and `enum_member(EnumName, "MemberName")` for a declared enum member.
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
operands evaluate in written order. Known non-string operands are rejected
before evaluation, including in skipped branches. String scans count towards
the program's work limit. These helpers are report-only, including in unused
bindings and skipped data-edit branches.

Each reducer takes one ordinary array expression. The array materializes fully
before reduction; generator-call spellings such as `forall(i in 1..3)(...)` are
unsupported. Length counts present values without flattening nested arrays. Sum accepts only integers and performs
checked addition in stored order. The quantifiers require fixed Booleans and
scan every stored element. Positional and keyed arrays use the same reducers;
keyed reducers visit stored cells, including in sparse arrays. Array indices
must still be valid. Data edits require contiguous axes and complete rectangular
coverage.

| Call | Empty array result |
| --- | --- |
| `length([])` | `0` |
| `sum([])` | `0` |
| `forall([])` | `true` |
| `exists([])` | `false` |

`require(condition, value, "message")` returns `value` when the condition is
true. Otherwise, it stops the report with the supplied literal message. A false
condition skips the value's arithmetic and array accesses. Syntax, names and
known types are checked before evaluation, so the skipped value cannot hide an
unsupported operation or a known type mismatch. Those checks may inspect input
values even when the condition will be false.

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

All six calls are report-only, including in unused bindings and skipped edit
branches. Sorting extracts keys once and needs O(n) temporary storage and
O(n log n) comparisons. Extrema scan the input once and select the winning
value. This work, temporary storage and output copying count towards the program
limits. A limit error produces no partial output.
See the [ordering report](queries.md#sort-rows-and-select-extrema).

`group_by` takes exactly two arguments: a positional record array and one string
literal field selector. Keys must be homogeneous integers, strings or resolved
members of one enum. It returns positional `key`/`items` records in first-seen
key order; each items array retains row order and duplicates. Other row fields
may differ. Empty input returns an empty array. Missing fields fail when
executed, while known incompatible keys fail before evaluation, even in skipped
branches.
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
kinds, ranks and index types are checked in every branch.
Alignment errors point to the call; errors within an operand point to that
operand.

Read-only keyed operands may have noncontiguous axes and missing cells. Helpers
retain their stored keys; they do not fill holes. Data edits still require
contiguous axes and rectangular coverage. An empty indexed delta comprehension headed by
`e.key` cannot infer axes after `entries`; empty-axis inference needs simple
generator key names. Report `zip_exact` directly to retain declared empty axes.
The [matrix example](queries.md#compare-keyed-matrix-cells) computes nonempty
cell deltas explicitly.

These calls are reporting operations only. An edit program refuses them in
preceding `let` bindings, including unused lets and skipped branches, and in every
`create` or `replace` right-hand side. The only calls allowed in data expressions
are `index_set` and `enum_member`.
The edit rules below apply to the whole program, including its bindings.

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
by typed identity and renders in first-insertion order. A string set, or a set with another
unsupported element, fails at the set expression or source literal range before report or `.dzn`
serialization. Strings remain ordinary scalars, array elements and `++`
operands. The [pinned specification](https://docs.minizinc.dev/en/2.10.1/spec.html#sets)
permits broader fixed sets, but the tested MiniZinc 2.10.1 compiler rejects
string set literals. This contract excludes them for both consumers.

Model calls, arbitrary built-ins, generator calls, interpolation, annotations,
`let` inside a MiniZinc expression, decision values,
floating ranges and every unlisted operator fail with a located explanation.

Named query functions are the exception to the arbitrary-call exclusion.
They declare typed parameters and a result type, and can use local `let`
bindings, conditionals and comprehensions.
Function parameters and immutable bindings can destructure tuples and records.
Guarded `match(subject)` expressions and `if ... { ... } else { ... }` blocks
choose which value to calculate. Only the chosen body runs, although every body
is checked for valid names, types and allowed operations. Native fields keep
their declaration identity and source coordinates.

Reports also support explicit-state `loop`, `break` and `continue`, named
`fold`/`scan` steps, typed queues and native identity sets. These calls compose
inside MiniZinc records, calls and comprehensions. The same scope and type checks
apply to nested inputs, and native values keep their owners and source locations.
These forms are report-only, including in unused bindings and skipped branches. See the
[patterns reference](reference.md#patterns-and-pure-control-values) and
[complete examples](queries.md#destructure-a-report).

Query functions do not execute model functions. Their calls share the program's
work and result limits; see the [function reference](reference.md#typed-query-functions).
Data-edit programs reject function declarations and calls, even when unused.

Numbers use signed 64-bit integers and finite binary64 decimal floats. Integer
literals may use MiniZinc decimal, `0x` or `0o` spelling, including
`-9223372036854775808`; hexadecimal floats are unsupported. Integer
arithmetic, range endpoints and index arithmetic fail on overflow. `div`,
`mod` and `/` fail on zero divisors; `i64::MIN div -1` and the matching `mod`
case fail on overflow. A nonzero decimal literal that rounds to zero fails,
as does any nonfinite literal or result. Conversion of an integer to a float
fails if binary64 cannot represent it exactly. The same rule applies to
`filter_elements` numeric comparisons. Floating arithmetic also refuses a
nonzero finite mathematical result rounded to zero. No implicit string-to-number
or string-to-member conversion is allowed.

### Constructed pipelines

Completed positional arrays and MiniZinc comprehensions accept
`filter(starts_with("prefix"))`, `count`, `sort`, `unique` and `tally` suffixes.
For example, `[x | x in [3,1,3]] | sort | unique`
returns `[1,3]`. The generator pipe belongs to the comprehension; the suffix
pipes consume its completed result. Group saved values and function calls,
as in `(rows) | sort` or `(make_rows()) | tally`. A binding already known to
hold a constructed value also accepts the suffix directly. Ordinary native
name and call pipelines retain their existing routing.

`filter(starts_with("a"))` retains strings beginning with `a`, in input order.
Matching is exact and case-sensitive; an empty prefix matches every string.
Non-string elements fail, including an incompatible declared element type on
empty input. `count` returns the number of present positional entries as an
integer, including zero for an empty array. A filter can precede another stage,
for example `(rows) | filter(starts_with("a")) | count`.

`sort` is stable ascending scalar ordering with the same homogeneous key rules
as `sort(xs)`. `unique` retains the first equal scalar, preserving input order.
`tally` accepts only strings and returns a dictionary of integer counts in
first-seen key order. It is terminal: another stage after it is an error. Untyped empty
arrays are accepted; an established incompatible element contract is still
refused when empty. Keyed arrays, records and native handles require explicit
conversion or projection before these operations. A keyed array can become a
positional one through `[e.value | e in entries(a)]`.

Suffixes lower to Zirql's existing value nodes and use its evaluator and request
budget. They are report-only, including unused lets and skipped branches in
data-edit requests. A failure points to the responsible suffix and produces no
partial CLI output. [The saved-selection example](queries.md#continue-a-calculated-array)
shows native inputs and calculated arrays in one report.

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

When a model declares a record's type, that declaration determines which fields
are required or optional. Optional fields may be omitted. Missing required
fields, extra labels and incompatible values are errors. Rust callers can use
`ConstructedValue::data()` to inspect a value while keeping its declaration
information. Decision values and computed source expressions are unsupported.

Tagged reports render absence as `{"kind":"absent"}`. Records emit only stored
fields and keyed arrays only stored cells. Data requests refuse absence,
optional source or target declarations, and these operations, including unused lets and
skipped branches. External parameters remain required int/string/bool inputs.
See the [optional report](queries.md#read-optional-fields-and-sparse-cells).

## Constructed shape and result

`[value | ...]` has positions `1..n` in generation order, including after a
filter. Fully keyed `[key: value | ...]` carries one integer or resolved enum
key per dimension. A tuple key establishes its arity. Keys must be unique;
integer axes retain numeric order and enum axes retain known identity and
declaration order. Read-only arrays may omit axis keys or cells. Omission does
not shift stored keys; data edits require contiguous axes and complete
rectangular coverage. A member name in a keyed source needs a matching model declaration to establish its enum
identity. Matching the spelling alone is insufficient.

A starting integer key can determine the following keys by checked addition.
Enum starting keys cannot be used to infer a tail. Array axes can come from
written integer intervals or explicit enums. Integer interval endpoints may
refer to unique fixed integer literals in the input; computed or uncertain
bounds are unsupported.

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
a scalar or collection value: `{"kind":"integer","value":7}` (similarly
`float`, `boolean`, `string`), `{"kind":"member","enum":"Guests","name":"A"}`,
`{"kind":"set","elements":[...]}`, `{"kind":"tuple","elements":[...]}`,
an ordered dictionary object such as `{"weight":{"kind":"integer","value":7}}`, or
`{"kind":"array","indexing":"positional"|"explicit","axes":[[key,...],...],"entries":[{"key":[key,...],"value":...}]}`.
Dictionary keys preserve insertion/current order recursively; empty dictionaries
render as `{}`. Native `map_by(key_query,value_query)` uses the same ordered
storage while retaining native payloads. It evaluates key then value against
the same singleton and rejects a duplicate before its value. `sort_by(value)`
uses present homogeneous integer, finite-float, string or coherent nominal
keys; native counts use checked signed conversion. Sorting keeps current-order
ties. Dictionary `reverse`, `head` and `tail` preserve kind and retained order,
including empty, zero and oversized bounds. Native tally keeps its stored
first-seen keys and counts through these operations. Both selectors and known
unsupported input/key/sort kinds and effects are checked even for empty inputs.

Array keys use the scalar integer/member forms. Positional arrays expose their
`1..n` keys here too; every axis is ordered. A complete report has
`"complete":true,"limitations":[]`. A report derived from incomplete semantic
inspection keeps `complete:false` and its existing located limitation objects.
Newly computed values do not invent source ranges. Source-backed query items
may retain their existing file and range in a separate inspected result.


## Reports and data edit requests

These two reports read assignment names. The first returns a list of records;
the second keeps an explicit integer key for each record.

```zirql
let names = filter(kind("assignment")) | names;
[(name: n, label: "data:" ++ n) | n in names]
```

```zirql
let names = filter(kind("assignment")) | names;
[i: (name: names[i]) | i in index_set(names)]
```

With just `counts = [3, 5];` in the input, the second report returns one record
containing its name. Here is the complete JSON output, with line breaks added
for readability:

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
      "value": {"name": {"kind": "string", "value": "counts"}}
    }]
  }
}
```


Native input pipelines project a `Strings` result to a positional array of
strings, `Count` to an integer, and `Literals` to a positional array of
source-backed literal values. For one named RHS, use
`items | filter(name("counts")) | values | one`; `one` checks the projection's
cardinality and extracts the value. Item, node and semantic selections need an
explicit value projection before use in a calculation. Histograms and rendered
JSON or data documents cannot be used as calculation inputs. Reports derived
from semantic queries keep their completeness status and limitations. A
projected name is a string; its spelling does not make it an enum member or a
record label.

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
line ending. An edit never changes model or include files. If the model does not
uniquely identify the target declaration and its type and indices, Zincite cannot
validate the assignment and refuses the edit.

An edit program constructs one atomic batch. It checks supported target types,
written finite integer domains and array shapes against the model declarations
and the combined result of all edits. The doubled example requires `entities`
length 4, `groups` length 2 and
`capacities` shape `4 × 2`, while input bindings still read the original
`2`, `1` and `2 × 2` values. Array bounds can depend on supported integer literal
assignments. A computed, ambiguous or unresolved bound stops the whole batch at
the relevant source location. These checks do not establish full MiniZinc type validity or
satisfiability.

Replacement changes only the original right-hand side. A right-hand side
containing comments cannot be replaced automatically; move or remove those
comments explicitly before making the edit. Comments outside replaced ranges,
whitespace and all other source bytes stay in place. Created items have no
inherited comments; creation fails if its placement would attach a directive to a different item.
Zincite rejects overlapping edits and changes that would damage a directive,
then reparses the complete `.dzn` candidate before previewing or writing it.
An error leaves stdout empty and the original file unchanged.

## A complete doubled instance

The input below has two entities in one group. Each entity stores a weight,
the index of its peer and the index of its group. The two entities refer to
each other. Each also has a two-column capacity row.

We want two independent copies of this instance: four entities in two groups.
The copied entities need new indices, and their peer and group references must
point into their own copy. Changing the counts alone would leave arrays and
references inconsistent.

Save the model and data as these two files:

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

The query uses two index maps. Copy 0 keeps the original indices. Copy 1 maps
entities 1 and 2 to 3 and 4, and group 1 to group 2. The peer of copied entity
3 is therefore 4, rather than original entity 2.

Save the program as `double.zq`. Its first three bindings read the original
records, capacity matrix and group memberships. The replacements update the
counts and all three collections in one batch.

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

With the [MiniZinc standard library configured](queries.md#inspect-semantic-facts),
run:

```sh
zincite query --max-work 5000000 --model doubled.mzn -f double.zq doubled.dzn
```

This example raises the work allowance because loading the standard library
and checking several dependent assignments can exceed the default budget.
The command prints the complete candidate without changing `doubled.dzn`:

```minizinc title="Standard output" wrap
entity_count = 4;
group_count = 2;
entities = [(weight: 7, peer: 2, group: 1), (weight: 11, peer: 1, group: 1), (weight: 7, peer: 4, group: 2), (weight: 11, peer: 3, group: 2)];
capacities = [| 3, 4 | 5, 6 | 3, 4 | 5, 6 |];
groups = [1: {1, 2}, 2: {3, 4}];
```

Add `--diff` to inspect the changes or `--write` to apply them. The
[data-edit guide](data-edits.md#construct-new-data) also shows how to redirect
the candidate into a separate file.

The weights and capacity rows are copied unchanged. Peer indices become
`2, 1, 4, 3`, and group indices become `1, 1, 2, 2`. The group memberships
are `{1, 2}` and `{3, 4}`, so each pair stays together.

A matrix indexed by entities on both axes would need another decision: what
should its cells contain for pairs from different copies? That relationship
must be written into the query; copying a single entity axis does not define it.

Enum members need an explicit mapping too. Suppose the model
declares `enum Guests = {A, B, A_copy, B_copy};`. This report binding names
both sides of a separate member mapping:

```zirql
let guest_map = [
    enum_member(Guests, "A"): enum_member(Guests, "A_copy"),
    enum_member(Guests, "B"): enum_member(Guests, "B_copy")
];
```

The map relates actual enum members. A string `"A"` or a record field named
`A` is a different value and is unaffected. Every mapped member must already
be declared in the model; a missing or ambiguous declaration is an error.

<a id="limits-diagnostics-and-implementation-seam"></a>

## Work limits and errors

One program budget covers reading input, calculations, generator candidates,
intermediate values, copying and rendering. Candidates count before filters,
so a comprehension with a small result can still do substantial work. The
default limits are 64 nested expression or collection levels, 1,000,000 work
units and 100,000 aggregate result entries. See [program limits](reference.md#program-limits)
for CLI overrides and the separate stream limit.

Repeating an expansion or serialization spends more of the same budget. Reusing
an immutable input value avoids copying all of its contents, but creating new
values, snapshotting outputs and rendering them still consume work and result
entries. Model scanning consumes work; raw model tokens do not count as report
entries. A limit error stops the request and produces no truncated report or
partially written data file.

For library callers, the accounting rules are:

- Reducers first calculate their complete operand array. `sum`, `forall` and
  `exists` spend one work unit per inspected element. `length` spends one for
  counting present values and converting the count to an integer. Each summary
  value uses one result entry, including for an empty input.
- `require` checks its message and operands before evaluation. A failed
  requirement spends work on copying its diagnostic message. A successful one
  returns its child value without another copy charge.
- Grouping, `entries` and `zip_exact` count field and key comparisons, enum
  metadata checks, temporary containers and copied keys or labels. Moving an
  already evaluated row or cell into its result does not deep-copy that value.
- Native projections prepare their bodies and primitive arguments once per
  invocation. Operations still run for each selected subject. Reusing an input
  value spends reference and slot work; new input values and changed contracts
  must be checked again.
- Scope metadata and storage growth consume budget before allocation. Leaving
  a scope does not refund work. Output snapshots and each rendering spend the
  remaining budget, including repeated library calls to render the same result.

Errors point to the responsible query expression or original model/data
location. These include invalid syntax, unsupported operations, incompatible
types, missing keys, duplicate targets and arithmetic errors. A semantic report
can return known results with `complete: false` and located limitations. An
unknown filter condition remains unknown under negation and cannot become a
match merely through `not`.

An edit needs enough information to validate every target. Unknown dependencies,
unresolved enum identities and unsupported computed source values stop the
batch. Some incomplete reductions can be previewed as described in the
[enum-reduction reference](reference.md#enum-reduction); in-place writing always
requires a complete candidate. An actual evaluation or rendering error leaves
stdout empty.

Zincite parses the MiniZinc syntax and supplies source selections, model facts
and literal values. Zirql evaluates the shared expressions, bindings and
comprehensions. Zincite then renders a report or validates and serializes the
complete edited data file. The report and edit forms share that evaluator and
budget, while data edits allow fewer operations. Neither form runs a MiniZinc
compiler or solver.

Use `--model` to supply target declarations, `--diff` to inspect changes and
`--write` to update an explicit regular data file. Model and include files are
read-only. The [Rust reference](reference.md#program-invocation-and-rust-results)
describes `Program`, its result types and candidate handling.

## Native ordering and sets

Read-only native ordering uses the same scalar key rules as constructed ordering.
The [ordering walkthrough](queries.md#order-native-selections) shows the
selection forms. `sort`, `min`, `max`, `min_all` and `max_all` each accept an
optional `*_by(selector)` form. Selectors use one incoming subject and the
surrounding lexical bindings. Explicit input stages retain their original-input
meaning. Native counts enter the signed key domain through checked conversion.
Missing, absent, Boolean or mixed integer/float keys fail. Nominal keys retain
host identities and coherent declaration metadata. Every key is admitted before
sorting or selecting extrema.

Sorts keep incoming ties; single extrema keep the first tie and all-ties extrema
retain every tied subject in input order. Empty native extrema produce an empty
selection with the same native kind, element contract and owner. Constructed
extrema remain optional and return `<>` when empty. Known unsupported input or
selector kinds and effects fail before iteration, including unused branches.

Native `union` keeps first occurrence from left then right. `intersect` and
`except` deduplicate in left order. The operands execute left then right against
the same incoming input. Only established identity-bearing selections are
admitted; no identity comes from rendered text. All handles and owners in both
operands are validated, even when they are discarded or the result is empty.
Native and constructed tally dictionaries retain first-seen keys through saved,
nested and JSON results. Ordering, identity work, copying and temporary storage
use the active request budget. Data-edit input and expression permissions remain
the closed subset described above.

## Native existence and fixed points

Native `where_exists(relation)` and `fixpoint(body)` are pure report operations.
Their bodies use the current runner, lexical scope and continuing request
budget. Preparation checks known kinds, names and effects even when the input
is empty. Existing data-expression permissions remain unchanged.

Existence keeps original subjects in input order, retaining duplicates. Its
relation runs against each singleton and must return an admitted native
selection. Complete empty results reject a subject; uncertain adjacency and
final unknown predicates refuse. A saved partial selection retains its
responsible error through copies, aliases, ordering and set operations, even
when an operation discards all rows. Unrelated report limitations do not refuse
a complete observation. Singular nodes and declarations normalize to their
possibly empty selection kinds at the existence boundary.

Fixed points replace the selection until ordered identities and duplicate
multiplicity are unchanged. The body keeps the exact native kind, element
contract and source owner, including for empty results. Explicit `input union`
provides accumulation. Stable empty selections succeed; repeated non-fixed
selections refuse with one bounded checkpoint. Native copying, comparison and
nested evaluation spend the active budget. See the
[walkthrough](queries.md#filter-connections-and-accumulate-declarations) and
[reference](reference.md#native-existence-and-fixed-points).
