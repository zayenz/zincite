# Bounded comprehensions for reports and data

`set_value("EXPRESSION")` inserts MiniZinc expression source into an existing
assignment. `values` reads written literals, but does not evaluate a computed
right-hand side. Neither operation constructs a new value from query results.
Read-only [query programs](reference.md#query-programs) support immutable `source`
and `let` bindings, ordered query statements, native reports and the bounded
expression/comprehension subset below. Zincite lowers its existing CST and uses
Zirql for checked expressions, scopes and generators. Model-backed `.dzn` edit batches use the same expression rules and evaluator.

Reports accept positional and fully keyed arrays, matrices, tuples, ordered
records and homogeneous Boolean/integer/float/nominal sets. Sources may use
existing query projections or one unique original literal assignment. Nested
arrays stay nested. Integer starting-key tails use checked successors; enum
starting-key tails are refused. `index_set`
returns established rank-one keys. `enum_member` resolves explicit declared
members through a retained read-only model; use `--model` for `.dzn` programs.
Source declarations may establish enum alignment and empty axes from written
integer intervals or explicit enums. Bounded integer axis endpoints may read unique fixed integer literal assignments;
computed dependency values and uncertain axes are refused.
Native-only reports keep their existing untagged result; pure reports use the
tagged schema in this contract.

The expression syntax follows the [MiniZinc 2.10.1 comprehension and generator
grammar](https://docs.minizinc.dev/en/2.10.1/spec.html#set-comprehensions).
MiniZinc itself permits more expressions and functions. This contract accepts
only the subset stated here; syntactic acceptance by Zincite's parser alone
does not imply that the constructor can evaluate an expression.

## Report expressions and edit grammar

One request has named sources, optional immutable bindings, then one report or
one atomic edit batch. The `query` string is an existing Zincite query pipeline;
`literal` names one existing assignment in the input `.dzn`. These are host
statements, not expression calls. Host strings use the current query string
escapes (`\"`, `\\`, `\n`, `\r`, `\t`); other escapes fail. Each source name and
`let` name must be unique.

```ebnf
request        = source* binding* (report | edit-batch) ;
source         = "source" ident "=" ("query" "(" string ")"
               | "literal" "(" string ")") ";" ;
binding        = "let" ident "=" expression ";" ;
report         = "report" expression ";" ;
edit-batch     = edit+ "emit_document" ";" ;
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
```

The brackets, key prefix and generator forms match the pinned [array and
indexed array comprehensions](https://docs.minizinc.dev/en/2.10.1/spec.html#indexed-array-comprehensions).
A generator's `in` source must be a finite constructed or source-backed array,
a fixed set with the supported element types below, or a finite integer range.
Set iteration uses type order: `false` before `true`, numeric order for numbers
and declaration order for members. An `in` generator yields **values**;
`a in records` does not yield keys or `(key, value)` pairs. Use `index_set(records)` and
`records[i]` when keys matter. Multiple names before `in` each range over the
same source, with the last name varying fastest. A single-name `=` generator
evaluates its right side once for each preceding binding. Earlier generator
bindings are visible in later sources and filters; a generator's own bindings
are visible in its `where` and the head. They are not visible in that
generator's source. The rightmost generator is innermost. `_` discards a value
and cannot be referenced. Duplicate bindings, shadowing a source or `let`,
and forward references fail at the name. Filters require a fixed Boolean.
Neither missing values nor decision-dependent Booleans are admitted. These
scope and iteration rules follow the [specification's generator
rules](https://docs.minizinc.dev/en/2.10.1/spec.html#set-comprehensions).

`expression` is a MiniZinc 2.10.1 expression parsed by Zincite's existing CST,
then checked against the following closed subset. Expressions may contain
signed integer, decimal float, Boolean and string
literals; bound names; parenthesized expressions; field access (`r.field`),
tuple access (`t.1`), array access (`a[i]` or `a[i,j]`); finite integer ranges
(`lo..hi`); set, tuple, record, positional array and fully keyed array
literals; the three comprehensions above; and `if ... then ... elseif ... then
... else ... endif`. The `else` is mandatory in this subset. Records use
MiniZinc labels, for example `(weight: 7, peer: 2)`; a label is not a string or
member. The only expression calls are `index_set(a)` for a one-dimensional
array's established keys and the host helper
`enum_member(EnumName, "MemberName")` for an explicit enum member identity.
`enum_member` requires a unique model enum declaration and a
member already declared there. Its arguments are an enum identifier and a
string literal, not computed expressions. Written member references in source
literals retain their resolved identity when the model establishes one. Member
names and generated assignment names must have valid MiniZinc identifier
spelling (quoted when required). Serialization uses the declaration's member
spelling; it never replaces matching text inside strings or field labels.

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
`let` inside an expression, decision values, optional values, recursion,
floating ranges and every unlisted operator fail with a located explanation.

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

## Constructed shape and result

`[value | ...]` has positions `1..n` in generation order, including after a
filter. Fully keyed `[key: value | ...]` carries one integer or resolved enum
key per dimension. A tuple key establishes its arity. Keys must be unique;
each integer axis must be contiguous, each enum axis must have a known enum
identity and a contiguous interval in its declared order, and every tuple-key
product must have exactly one value at every combination. A keyed array that loses a cell fails rather than
silently shifting keys. Keyed sources whose member names are merely syntactic
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

```text
source names = query("filter(kind(\"assignment\")) | names");
report [(name: n, label: "data:" ++ n) | n in names];
```

```text
source names = query("filter(kind(\"assignment\")) | names");
report [i: (name: names[i]) | i in index_set(names)];
```

`query` converts a current `Strings` result to a positional array of strings,
`Count` to an integer, and `Literals` to a positional array of source-backed
literal values. `literal` returns the one named RHS value. Item,
node and semantic selections need an explicit, typed projection before use;
an ambiguous native result, `Tally`, `Json`, `Document` or `Reduction` is refused. Report
construction over semantic results carries their completion status. A queried
name is a string; it never becomes an enum member or record label by spelling.

For a `.dzn` containing `counts = [3, 5];`, with `counts` and
`doubled_counts` both declared as `array[1..2] of int` in a read-only model,
these requests make different kinds of data edit:

```text
source old_counts = literal("counts");
create doubled_counts = [old_counts[i] * 2 | i in index_set(old_counts)];
emit_document;
```

```text
source old_counts = literal("counts");
replace counts = [old_counts[i] + 1 | i in index_set(old_counts)];
emit_document;
```

The first adds `doubled_counts = [6, 10];`; the second replaces `counts` with
`[4, 6]`. Neither is equivalent to `set_value("[old_counts[i] * 2 | ...]")`,
which only inserts expression text. `create` requires an absent assignment
name and a unique compatible model declaration. `replace` requires exactly one
existing assignment. Duplicate targets fail; a request cannot create and
replace the same name. All sources and `let` bindings read the original input,
never a preceding edit in the same request. Creation appends a complete item
after the last assignment, before trailing detached comments, using the input's
line ending. An edit never changes model or include files. Without unique
model ownership, type and axis obligations for a new or changed assignment
remain unknown and the edit is refused.

An edit program constructs one atomic batch. It checks supported target types,
written finite integer scalar domains and array shapes using read-only model
declarations and the coherent candidate assignments after all edits. Thus the
doubled example requires `entities` length 4, `groups` length 2 and
`capacities` shape `4 × 2`, while source bindings still read the original
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

```minizinc
int: entity_count;
int: group_count;
array[1..entity_count] of record(int: weight, int: peer, int: group): entities;
array[1..entity_count, 1..2] of int: capacities;
array[1..group_count] of set of int: groups;
solve satisfy;
```

```minizinc
entity_count = 2;
group_count = 1;
entities = [(weight: 7, peer: 2, group: 1), (weight: 11, peer: 1, group: 1)];
capacities = [| 3, 4 | 5, 6 |];
groups = [{1, 2}];
```

The request names the two maps. For copy `c` in `0..1`, an old entity `i`
becomes `entity_map[i,c] = i + 2*c`, and an old group `g` becomes
`group_map[g,c] = g + c`. Each source name binds an original literal value.
The five replacements form one candidate.

```text
source old_entities = literal("entities");
source old_capacities = literal("capacities");
source old_groups = literal("groups");
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
emit_document;
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

```text
let guest_map = [
    enum_member(Guests, "A"): enum_member(Guests, "A_copy"),
    enum_member(Guests, "B"): enum_member(Guests, "B_copy")
];
```

Changing a string `"A"` or an unrelated record label `A` would be wrong.
This bounded constructor does not invent enum declarations or names. An
absent or ambiguous member declaration refuses the mapping.

## Limits, diagnostics and implementation seam

One shared budget covers query source bindings, `let` evaluation, generator
candidates **before** filters, expression visits, nested values and keys,
every copied byte, edit construction and final rendering. The recommended
defaults reuse `Limits`: at most 64 nested expression/collection levels,
1,000,000 work units and 100,000 aggregate produced entries across the
request. A query source's model scan keeps its separate pre-allocation
admission; raw loaded tokens do not count as produced report entries. Check
products and byte lengths with overflow-safe arithmetic before allocating.
Each repeated generator expansion and repeated serialization is charged again.
On exhaustion, return a located limit error; never truncate a report or edit.

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
edit preparation and complete-candidate reparse. Both adapters use the same expression rules and budget. Edit expressions are
validated at parse time and lowered again with declared target evidence during
evaluation; this additional work spends the same request budget. Zincite has one CST frontend and uses the existing
Zirql dependency for shared evaluation. It does not run a model, compiler or solver.

The user approved this exact bounded data contract on 2026-10-13. The library
and both query commands implement it. Use `--model` to supply read-only target
declarations, `--diff` to inspect changes, and `--write` only for an explicit
regular data file. Model and include files are never edited. A candidate's type
and axis checks cover this subset; they do not prove full MiniZinc validity or
satisfiability.
