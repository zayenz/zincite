# Syntax and library reference

Zincite targets MiniZinc 2.10.1 model (`.mzn`) and data (`.dzn`) files.
For installation and command usage, see the [README](../README.md).

## Syntax support

This describes supported syntax families, not complete compiler compatibility.
Known corpus gaps include form-feed whitespace, escaped single quotes in strings,
deprecated declarations without the `function` keyword, anonymous enum
constructors, standalone equality items and callable annotation capture. See the
[corpus checkpoint](../scripts/formatter-corpus-checkpoint.md) for examples,
validation results and unresolved performance limits.

The parser supports:

- `bool`, `int`, `float`, `string` and `ann` declarations, with optional `var`
  or `par` and `opt`, annotations and an optional initializer; `any` declarations
  are also accepted.
- Named and expression domains, including scalar ranges with numeric-expression
  bounds.
- Ordinary `set of`, `array[...] of` and `list of` declarations, including
  multiple array dimensions and nested arrays/lists.
- Set cardinalities such as `var set(n) of 1..10`, and index-dependent arrays such
  as `array[i in 1..n] of var 1..(i+2)`, including mixed index types and nested arrays.
- Set and array literals, including empty literals and trailing commas. Indexed
  arrays support a key on each entry or a starting key followed by bare values;
  keys may use parenthesized index tuples.
- Two-dimensional literals with optional column and row indices, including empty
  forms and written row delimiters.
- Set, array and indexed array comprehensions, including scalar or tuple keys,
  multiple-name `in` generators, single-name assignment generators and an optional
  attached `where` filter on each generator. Quoted and anonymous bindings and
  trailing generator commas are supported.
- Array indexing after atoms, calls and parentheses, including repeated indexing,
  multiple indices and range slices with either, both or neither bound.
- Assignments to ordinary or quoted identifiers.
- Atom values: identifiers, quoted identifiers, integer/float/string literals,
  `true`, `false`, `infinity`, anonymous `_` and absent `<>`.
- Interpolated strings with one or more `\(expression)` segments, including nested
  interpolated strings. Embedded expressions use the currently supported expression
  families; literal segments, escapes and interpolation delimiters retain their
  exact spelling while embedded expression layout is formatted.
- All unary and binary operators from the pinned specification, including
  backtick operators, Unicode spellings and half-open or one-sided ranges.
- Parenthesized expressions and ordinary or quoted calls, including nested calls,
  named arguments, empty argument lists and trailing commas.
- Generator calls such as `forall(i in Indices)(i > 0)` and `sum(i in Indices)(i)`,
  including supported nested expressions, annotations and array access.
- Conditional expressions with ordered `if`/`elseif` branches and an optional
  `else`, including nested bodies and numeric-expression positions.
- `let` expressions with local declarations using the supported types, local
  constraints and greedy bodies. Blocks contain at least one item; both semicolon
  and comma separators and a trailing separator are retained. Local declaration
  names and byte ranges are exposed by the same declaration nodes and direct
  token leaves as top-level names.
- Function, predicate, test and annotation declarations with optional parameter
  lists and bodies. Parameters support the existing scalar/collection types,
  generic type-inst variables, `any`-qualified variables and named defaults.
  MiniZinc 2.10.1 also accepts type-only unnamed parameters and annotations after
  named parameters, before an optional default. Zincite supports these compiler
  extensions to the narrower printed parameter grammar. A written colon still
  requires a name; unnamed defaults and annotations before the name are rejected.
  Function, predicate and test declarations support declaration annotations; the
  pinned grammar does not permit declaration annotations on annotation declarations.
  Set cardinalities and dependent array index bindings are excluded from parameter
  types where the parameter grammar requires it. Array index types themselves use
  the ordinary type grammar. Return types use that same ordinary grammar.
- Enum declarations, including deferred definitions, explicit member lists,
  anonymous `_(expression)` and named constructor forms, and combinations with
  `++`. Enum/type declaration names, member names and constructor names have
  distinct node kinds with direct name tokens and precise byte ranges. Constructor
  arguments and ordinary uses of these names remain expression syntax; their
  references are not resolved or classified as declarations.
- Type aliases with declaration annotations and the currently supported target
  types. Shared type-inst concatenation supports declarations, aliases, callable
  return/parameter types, array indices and local types. Its right operand is a
  base type, as required by the pinned grammar; parenthesized expression
  concatenation remains expression syntax. Targets are neither resolved nor
  evaluated.
- Declaration and expression annotations, including annotations on local let
  declarations. The bare reserved `output` token is accepted immediately after
  `::`, matching MiniZinc 2.10.1; ordinary expressions, parenthesized annotations,
  calls and field access using that reserved spelling remain rejected. Other
  annotation literals use ordinary identifier and call syntax.
- Constraints with an expression and an optional direct `:: "label"`, including
  interpolated labels.
- Include items with a string literal path, output items with an optional string,
  call or parenthesized section annotation, and `solve satisfy`, `solve minimize`
  and `solve maximize` with solve annotations. Output bodies and optimization
  objectives use the supported expression families; expression annotations stay
  attached to those expressions.

Items use semicolon separators; the last semicolon is optional on input.
Comments may occur between tokens. Formatting preserves comment, literal and
operator spelling and explicit parentheses. It puts a labelled constraint's
expression on the following line and retains explicitly multiline comma-separated
lists with one entry per line and a trailing comma. A multiline nested entry does
not expand its enclosing list. Generator calls use spaces before their header and
body parentheses. `forall` bodies always use block indentation, including quoted
`'forall'` calls. Other short bodies stay compact; explicitly expanded bodies stay
expanded. Generator headers exceeding 120 columns or written with line breaks
use one generator per line, with each `where` filter indented beneath its generator.
Expanded headers have a trailing comma; bodies contain a single expression without
an added comma. Ordinary expressions, annotations, lists and item continuations
wrap using their actual line prefix and following delimiters. Broken binary
operators end the preceding line; continuation indentation stays consistent
through a chain. Parentheses, precedence and annotation order are preserved.
Indivisible identifiers, literals and preserved comments may exceed the width
without a warning.
Conditional branch bodies and `let` blocks/bodies use block indentation;
local item order, separators, parentheses and annotation attachment are retained.
Generic type-inst syntax is shared by local, top-level and callable declarations.
No local name resolution or checks of branch types or declaration initializers are
performed. Adjacent include groups sort with their attached comments, retaining
written paths and group boundaries; included files are never loaded.
Matrices align columns and wrap rows at shared column boundaries when needed,
retaining their logical row boundaries, indices and comments. It normalizes
editable spacing, defaults to LF with a final newline, and preserves blank-line
groups while collapsing excess blank layout lines.

Tuple and record types, literals and chained field/tuple access are supported,
including nested types, `var`/`par`/`opt` qualifiers, array fields, aliases, callable
signatures and local declarations. Field order and spelling are preserved.
The CST distinguishes record type field declarations, literal field labels and
access references; parsing does not resolve them. Types require at least one
field and accept a trailing comma. Tuple literals require their first comma,
including `(value,)`; this follows the pinned grammar despite its prose excluding
unary tuples. Record literals also accept `(field: value)` without a comma,
matching MiniZinc 2.10.1 beyond the printed record production. They retain a
record node and field-label nodes rather than becoming parenthesized expressions.
The structured fixture and its formatted output pass MiniZinc 2.10.1 model checks.
`variant_record` and `case` are reserved keywords without productions in the
pinned grammar; they remain unsupported and produce diagnostics. Unsupported
input produces diagnostics. Range-type bounds follow the numeric-expression
grammar with the compiler-supported `infinity` bound, including signed bounds.
Their parentheses may contain numeric operators, identifiers and calls; call
arguments and annotations use the general expression grammar. Global and local
declaration initializers accept both `=` and the compiler-supported `==` spelling,
retaining the written token. Other declarations such as type aliases still require
`=`. Local annotation declarations remain rejected, as in MiniZinc 2.10.1.

Parsing checks syntax only: it does not resolve includes or names, require a
solve item, infer types or check semantic validity. For example, accepting a
named argument does not establish that a callable has that parameter, and
accepting a range expression does not establish that its bounds have valid types.
The enum/alias fixture and its formatted output pass MiniZinc 2.10.1 model checks.
Type-inst concatenation examples establish syntax and format stability only:
MiniZinc rejects an enum-target `type Joined = Left ++ Right` with a type error.
Zincite retains the pinned concatenation syntax without imposing that semantic
restriction on parsing.
Written indices and cardinalities are retained without evaluation, domain inference,
contiguity checks or rectangularity checks. The pinned grammar permits an empty
index tuple `()` as a key, although the installed MiniZinc 2.10.1 compiler rejects
it. That compiler also crashes on a mixed binder/plain-index declaration during
`--model-check-only`; Zincite accepts these forms from the specification.

The comparison with the pinned [full grammar](https://docs.minizinc.dev/en/2.10.1/spec.html#full-grammar)
covers its item families, ordinary and parameter type-inst forms, general and
numeric expressions, collection/structured literals and access, control and
call expressions, interpolation and annotations. All planned families have
shared parser/formatter paths; the integration fixture combines them, rather
than treating each family as an isolated syntax subset. This is family coverage,
not a claim of complete compiler compatibility: the grammar/compiler differences
and syntax-only exclusions above still apply. `variant_record` and `case` have
no productions in this grammar. JSON data and solver output formats are outside
the `.mzn`/`.dzn` target.

The model/data pair and cross-family integration fixture, both original and
formatted, pass MiniZinc 2.10.1 `--model-check-only`. Data assignment expressions
use the same expression grammar as models, without evaluation or type checking.
The specification forbids user-defined operation calls in data files; deciding
whether a call is user-defined requires model name resolution, so syntax-only
data parsing retains calls and does not establish that restriction. Parsing
never loads includes or requires a solve item.

## Syntax library

`zincite_syntax::lex` owns UTF-8 source text and exposes read-only source,
tokens and lexical diagnostics. Token ranges cover every byte exactly once,
including comments, whitespace and erroneous input. Slice the source with a
token's half-open byte range to read its original spelling.

The lexer recognizes the lexical forms in the
[MiniZinc 2.10.1 specification](https://docs.minizinc.dev/en/2.10.1/spec.html),
including quoted names, numerals, strings and Unicode operators. It
reports malformed lexical input and resumes at a reliable boundary. Interpolated
strings retain exact literal/delimiter chunks and embedded expression tokens in
the same token buffer, including nested strings and written escapes. Lexing does
not validate model/data grammar, resolve includes or check semantic validity.

`zincite_syntax::parse` owns source text, tokens, a concrete syntax tree and
combined lexical/parser diagnostics. The tree exposes ordered children, item
and expression kinds, and half-open byte ranges. Its token leaves cover the
whole source, including malformed input in error nodes. Parsing resumes after
an erroneous item so callers can inspect subsequent items.

`SyntaxNode::child_nodes` borrows direct child nodes in source order. Declaration
types, collection entries, array index types and access expressions have distinct
node kinds in this same tree. Indexed entries expose their key and value; matrix
nodes expose column indices and rows in written order. Array index bindings retain
their name tokens and index type, and set cardinalities retain their expression.
Comprehensions expose their head and ordered generator list; indexed heads reuse
the key/value entry nodes. Each generator retains its written binding names and
`in` or `=` token, its source/value expression and its optional `where` filter.
Generator calls expose that same list followed by a parenthesized body.
Function, predicate, test and annotation declarations expose their name as a direct
token. Their optional `ParameterList` contains ordered `Parameter` nodes with a
type child, optional direct binding name, ordered `Annotation` children and an
optional default expression. Unnamed parameters contain only their type; annotations
after a default belong to that expression. Name tokens and parameter nodes retain
precise byte ranges. Generic variables have a distinct
`TypeInstVariable` kind and retain their qualifiers and written `$T`/`$$Index`
spelling. These views borrow the existing tree.
Interpolated strings expose their embedded expressions as ordered child nodes,
with literal chunks retained as token leaves in the same tree.
Include nodes expose their path literal as a child. Output nodes expose an optional
`Annotation` child followed by the output expression. `Solve` identifies a satisfy
item; `SolveMinimize` and `SolveMaximize` expose direct solve annotations followed
by the objective expression. Each node retains its precise source range.

## Query library

`zincite-query` selects top-level items from one model or assignment-only data
file and explicitly edits data assignments and enum membership. Its reusable
library and both commands use the same fixed pipeline:

| Stage | Result |
| --- | --- |
| `items` | Reset to all top-level items in written order |
| `filter(predicate)` | Keep matching items in their existing order |
| `head(n)` | Keep the first `n` items of the computed selection |
| `count` | Return a native count; render as decimal text followed by LF |
| `expressions` | Select expression nodes in each selected subtree |
| `children` | Replace nodes with immediate CST child nodes |
| `subtree` | Expand each node to itself and all descendants |
| `range(start,end)` | Keep selected nodes fully contained in the half-open input byte range |
| `references` | Select written reference occurrences contained in selected regions |
| `declarations` | Select declarations owned by selected nodes or proven reference targets |
| `uses` | Select written occurrences referring to selected declaration identities |
| `transitive_references(n)` | Expand declaration references for at most `n` waves |
| `types` | Inspect native structured declaration/expression types |
| `instantiations` | Inspect independently available `par`/`var` facts and unknown reasons |
| `names` | Project directly retained identifier identities |
| `call_names` | Project call and generator-call heads on selected nodes |
| `annotation_names` | Project heads of selected annotation nodes |
| `text` | Project exact original node text as UTF-8 strings |
| `unique` | Keep the first occurrence of each node or string |
| `tally` | Return a native string-to-count histogram |
| `values` | Inspect recursive literal values with original spelling and ranges |
| `fields` | Select record field nodes, keeping labels separate from values |
| `elements` | Select written collection values, excluding array keys |
| `keys` | Select written array keys and matrix axis keys |
| `json` | Render nodes, literals, strings, a count or histogram as JSON |
| `emit` | Render the selected original source bytes |
| `set_value("EXPRESSION")` | Replace selected assignment right-hand sides in data mode |
| `filter_elements(comparison)` | Explicitly filter literal set, array or matrix elements in data mode |
| `remove` | Remove selected data assignments and their attached comments/directives |
| `reduce_enum("Guests", keep("A", "B"))` | Keep explicit enum members and reduce supported dependent arrays, sets and group accesses |
| `emit_document` | Render the reparsed edit candidate |

Stages are separated by `|`. The initial selection is `items`, including for an
empty query. Source or projected-string emission is implicit. `emit` and `json` are terminal.
Only `json` may follow `count` or `tally`. A pipeline may contain one transformation stage,
followed only by its optional `emit_document`; document emission is implicit for
edits. `emit_document` requires a transformation. `head` does not avoid parsing
or evaluating the earlier selection. No input items produces an empty selection or count 0;
the untouched selection of a comment-only document still emits its original bytes.

Predicates are `kind("assignment")` and `name("capacity")`, combined with
parentheses and `not`, `and`, `or` in that precedence order. Query strings are
double quoted and accept `\"`, `\\`, `\n`, `\r` and `\t` escapes. Other escapes
and literal control characters are errors; Unicode characters may be written
directly. Whitespace between query tokens is ignored. Query comments, program
files and bindings are not implemented.

The accepted kind strings are `assignment`, `declaration`, `enum`, `type_alias`,
`function`, `predicate`, `test`, `annotation`, `constraint`, `include`, `output`
and `solve`. All three solve modes have kind `solve`. Unknown kinds are query
errors. `name` compares the direct declared/assigned identifier, ignoring single
quote delimiters on source names and on the predicate argument. For example,
`name("capacity")` and `name("'capacity'")` both select `'capacity' = 2;`.
Comparison is case sensitive and does not normalize Unicode. Constraints,
includes, output and solve items have no item name; names used inside expressions
are not item names. Syntax queries never load includes, resolve bindings,
evaluate data or require an installed standard library.

Navigation operates on the current selection. `expressions` includes literal and
identifier atoms, calls, operator/access/annotated expressions, conditionals,
let expressions, interpolated strings, tuple/record/set/array/matrix literals
and comprehensions. Structural wrappers such as annotations, generators and
branches remain available through `children` and `subtree`. Navigation expands
each selected root independently in written preorder; overlapping subtrees keep
duplicates. `unique` removes duplicate CST nodes by identity and equal projected
strings, retaining the first occurrence. These are counts of written source
constructs, not execution frequency or flattened constraints.

`range(start,end)` filters the current node stream by full containment. Bounds
use original input bytes, including a leading BOM; reversed, overflowing,
out-of-input or split UTF-8 bounds fail. A valid empty range or unmatched
selection succeeds with an empty stream. Expression fragments need not form a
standalone MiniZinc model.

Node projections inspect selected nodes, without an implicit subtree expansion.
`names` emits direct identifier tokens in written order, ignoring quote
delimiters; this includes declarations, references and field/binding labels.
`call_names` includes ordinary and generator calls, anonymous `_` constructors
and retained inverse-constructor heads. `annotation_names` takes the head of an
Annotation node, including bare reserved `output`. Projections skip nodes with
no applicable name and preserve duplicates. Use `subtree` before a projection
to include nested occurrences. String streams emit one value and LF per entry.
`tally` counts equal strings and emits a JSON object with lexical key order;
an empty string stream yields `{}`.

Item filters keep the kind aliases above. Navigated node filters and JSON use
original CST kinds in snake_case, such as `call_expression`,
`annotation_declaration`, `annotation`, `solve_minimize` and `solve_maximize`.
`name` on navigated nodes compares the first direct identifier identity.
`head` and `count` accept node, literal or string streams. `unique` accepts node or string streams. Navigation,
filtering and projections require nodes; `tally` requires projected strings.
`items` resets a node selection. Invalid stage types produce located query
errors even for empty streams. Assignment transformations require an item selection.
`filter_elements` also accepts original collection nodes; literal and string streams
cannot become editing targets.

`json` emits node streams as arrays of objects with `kind`, `names`,
`call_names`, `annotation_names`, `file`, `range: {start,end}` and `text` fields.
Text covers the exact node range, excluding separately attached item comments.
Strings emit JSON arrays, counts emit numbers and histograms emit objects.
JSON output ends in LF. This is an inspection format, not round-trippable
MiniZinc data. CLI file identity uses the supplied file path or
`--stdin-filepath`, defaulting to `<stdin>`. Library callers use
`Input::parse_named`; `Input::parse` defaults to `<input>`.

`text` and node JSON reject selected spans containing opaque invalid UTF-8
comment bytes with an input diagnostic. Ordinary source emission continues to
preserve those bytes exactly. Inspection JSON does not load includes or resolve
names.

The untouched initial selection and any final reset with `items` emit the exact
document, including a leading UTF-8 BOM, section comments and all whitespace.
`filter` and `head` produce fragment selections, even if every item survives.
Fragments emit original item bytes and attached comments in written order:

- A comment after an item on the same line belongs to that item, including a
  block comment that starts on that line. The following first line ending and
  surrounding same-line whitespace are retained.
- A standalone preceding comment block attaches to the following item when
  no blank line separates them. Its indentation and line endings are retained.
  Separated section comments stay out of fragments. Fragments omit the BOM.
- Standalone `% zincite-fmt: skip` and `% zincite-lint: ignore ...` markers
  attach to their next item, including intervening whitespace.
  `% zincite-fmt: off` attaches to the first following item and `% zincite-fmt: on` to the last
  preceding item. Their retained spans include the intervening trivia.
  Fragment selections fail with a source diagnostic if marker attachments
  overlap, split a next-item marker from its target or leave paired formatter
  markers unbalanced. A count does not emit fragments and needs no such check.

Source rendering inserts no separators and changes no line endings. It preserves
opaque non-UTF-8 comment bytes; code and literals must be UTF-8. Input parsing
rejects malformed/unsupported syntax and non-assignment data items. It establishes
syntax only, not compiler acceptance or semantic validity of data expressions.

Assignment edits require data-file mode, even when a model-mode input contains
only assignments. `set_value` checks that its argument contains one valid
expression, including when no assignment is selected. It replaces only the RHS
node range, preserving identifier spelling, surrounding comments and whitespace.
Comments inside that range cause a located error because their replacement
position is unspecified. `remove` deletes attached comments with each assignment,
keeps separated section comments, and removes next-item directives with their
original targets. Surviving paired formatter directives must remain balanced and
standalone between complete items. Query edits preserve opaque lint directive
comments without interpreting rule names.

Edits use original byte coordinates, validate ranges/copies and reject overlaps
as one atomic batch. The complete candidate is reparsed as data before output;
no later query stage uses the original CST against the changed bytes. An empty
edit selection returns the complete original document. These operations change
instance meaning: parsing does not establish compiler type validity, semantic
equivalence or satisfiability.

`values`, `fields`, `elements` and `keys` resolve selected assignments to their
single RHS and resolve a record field node to its value. Parentheses may wrap
literals; numeric signs may wrap numbers. `values` rejects calls, operators,
ranges, comprehensions, interpolation and other computed forms at their source
ranges. Node projections require a matching literal container and can expose its
computed child nodes for syntax inspection. Bare and quoted identifiers are inspected as member references without
resolving declarations. Inspection preserves numeric and string spelling even
when the comparison routines do not support that spelling.

`fields` selects `RecordLiteralField` nodes. `elements` selects direct set,
tuple or array values; matrix cells follow row order. Indexed array keys are
excluded from `elements`. `keys` selects only written keys: positional arrays
produce an empty stream; a starting-key array exposes its one written key.
Matrix column keys precede row keys. These stages return ordinary node streams,
so existing navigation, name/text projections and JSON remain available. They do
not expand nested collections implicitly.

`values` returns `QueryResult::Literals(LiteralSelection)` with recursive
`LiteralValue` entries. Each value exposes its file, original byte range,
original spelling bytes and `LiteralKind`. Records expose separate labels
(identity, range and spelling) and values. Arrays expose key/value entries and
`ArrayIndexing::{Positional, Explicit, StartingKey}`. Matrices expose column
keys, row keys and cells separately. Sets and tuples retain ordered elements.
`values` accepts `head`, `count`, `json` and `emit`; implicit/explicit source
emission concatenates the original value spans. Node navigation and editing
require original node streams rather than these inspection values.

Literal JSON uses the same `file`, `range: {start,end}` and exact UTF-8 `text`
fields as node JSON, plus a literal `kind`. Kinds are `integer`, `float`,
`boolean`, `string`, `member`, `set`, `tuple`, `record`, `array` and `matrix`.
Booleans include `value`; members include `name`. Sets/tuples include recursive
`elements`; records include `fields: [{label: {name,range,text}, value}]`.
Arrays include `indexing` (`positional`, `explicit` or `starting_key`) and
`entries: [{key, value}]`, with `null` for an unwritten key. Matrices include
`column_keys` and `rows: [{key, cells}]`. Numeric and string literal text remains
source spelling, rather than a normalized JSON value. Literal JSON is recursive
inspection data and also rejects opaque invalid UTF-8 comment bytes.

For example, inspect a keyed record's nested data with:

```sh
zincite query 'values | json' records.dzn
zincite query 'elements | fields | names | json' records.dzn
zincite query 'keys | names | json' records.dzn
zincite query 'filter(name("weights")) | filter_elements(gt(0))' instance.dzn
```

`filter_elements` compares direct scalar elements with `eq(VALUE)`, `lt(NUMBER)`,
`le(NUMBER)`, `gt(NUMBER)` or `ge(NUMBER)`. Equality accepts numbers, Booleans,
double-quoted strings and explicit member identity, for example
`eq(member("A"))`. A member, a record label and a string stay distinct. Invalid
operand syntax/types fail while parsing the query, including for an empty
selection. Collections and records cannot be comparison operands; incompatible
input element types fail at their input ranges rather than comparing unequal.

Integer comparisons support signed 64-bit decimal, hexadecimal and octal
literals, including `-9223372036854775808`. Decimal float comparisons require
finite binary64 values; overflow and a nonzero literal rounded to zero fail.
Mixed integer/float comparisons require an exactly representable integer and
refuse loss of precision. Hexadecimal floats are inspection-only. String
comparison decodes only escaped quote, backslash, `n`, `r` and `t`; other source
escapes remain inspection-only. Query strings retain their existing escape rules.

Filtering emits a complete candidate through the same output/diff/write path as
assignment edits. Lists and sets may become empty. Positional list indices compact
only under this explicit transformation. Fully written integer/member keys retain
their spelling and identity; key types and arity must be compatible, keys must be
unique, integer axes must remain contiguous, and tuple keys must cover rectangular
axes. Changed starting-key arrays with bare tails fail because syntax alone cannot
establish the surviving identities. Tuples and records are inspection forms, not
variable-length filtering targets. Nested values are filtered only when selected
explicitly. Ordinary nested lists can have different lengths.

Matrix filtering compares cells and requires the same retained column mask in
every row. It preserves row keys and filters column keys with that mask. Ragged
rows, incompatible keys and empty surviving column axes fail. Filtering a nested
collection also checks its enclosing matrix/keyed-array shape. Entry comments
follow the same preceding-block and same-line attachment rules as item comments:
removed-entry comments disappear, retained comments and separated section comments
keep their exact bytes, and separators precede trailing line comments. Other
layout bytes remain untouched, so filtering can leave extra spaces or blank lines.
Duplicate or overlapping targets fail as one atomic batch; use `unique` when an
inspection pipeline selects the same collection more than once.

### Enum reduction

```sh
zincite query --model seating.mzn 'reduce_enum("Guests", keep("A", "C"))' data.dzn
zincite-query --diff 'reduce_enum("Guests", keep_first(20))' data.dzn
```

`reduce_enum` requires a top-level item selection in data mode and inspects the
whole document, even after `filter` or `head`. The target must be one assignment
whose value is an explicit set of unique member identifiers. Parentheses and
quoted spelling survive. `keep` chooses identities while preserving original
member order; `keep_first` uses that order. Unknown requested members and
constructed or anonymous enums are located errors. `keep()` and `keep_first(0)`
allow empty retention.

Explicit member keys establish enum identity without a model. With model facts,
unique top-level declarations associate data assignments with enum identities,
expanded types and written index domains. Positional slicing requires the full
original enum domain and exact coverage; an enum index type alone does not prove
that a named subset has the same membership. Unknown aliases, ambiguous
associations, computed indices and starting-key tails remain unresolved.

Reduction removes matching keyed entries, including whole records, and slices
supported positional lists, matrix rows/columns and rectangular tuple-key arrays.
It follows tuple and record fields, matching record labels rather than type field
order, and can edit a nested array beside an unsupported computed field. Repeated
target dimensions receive the same membership mask. Other axes, surviving values,
comments and source spelling keep their written order. Empty lists are supported;
empty multidimensional forms that cannot preserve established axes remain
unresolved. No integer value is treated as an enum identity merely by position or
matching length.

Supported set literals lose direct references to removed members, including sets
inside records, tuples and arrays. Known unrelated element types remain unchanged.
Computed set entries remain visible with located diagnostics while independent
literal members can still be pruned. Required record fields, fixed-index cells and
multidimensional cells retain valid empty sets.

Positional one-dimensional `array[int] of set of Guests` groups may lose entries
that become empty after pruning. Pre-existing empty groups and singletons stay in
order. Fixed ranges, named subsets and unknown axes do not permit group compaction.
An explicit mapping from each original integer index to its retained index repairs
literal group accesses in data. Unique model/data associations and supported
record fields, tuple fields or literal array selectors identify the original group
collection; assignment order does not affect the mapping. Deleted groups,
computed selectors and unknown collection paths remain unresolved. Arbitrary
integers and strings are never reinterpreted as group indices.

`ReductionResult` owns the reparsed `candidate()` and exposes located `unresolved()`
dependencies; `is_complete()` means that list is empty. Surviving removed-member
scalars, relevant computed accesses, missing alignment facts and direct dependencies
in read-only model sources remain visible. An ordinary enum generator such as
`forall(g in Guests)(scores[g] >= 0)` preserves member identity when its source
resolves directly to the target enum. Numeric conversions such as `to_enum` and
read-only model group accesses that need an index repair remain unresolved.
Strings and record labels are never rewritten as enum members. This bounded check
does not establish satisfiability or full
compiler type validity.

### Semantic inspection

Semantic stages consume the public `load_model`, `resolve_bindings`,
`resolve_callables` and `resolve_instantiations` APIs. They do not enable lint
rules. An explicit model-mode input file is its own root by default; `--model`
supplies another retained root. Semantic input must be in model mode and match
that root's original bytes exactly, including the BOM and opaque comments.
Stdin needs explicit `--model`; a matching `--stdin-filepath` label alone does
not load a model. The library requires a `ModelContext` with a retained root.
Syntax pipelines ignore an unused supplied model, including a missing path.
Data reduction keeps its separate optional model interpretation contract.

`references` selects each written reference fully contained in a selected node's
or declaration's owning region. A selected reference has its exact token region.
`declarations` selects declarations whose owning CST range equals a selected
node's range, preserves already selected declaration IDs, or follows proven
reference targets. Use `subtree | declarations` to reach nested declarations.
Value/type targets require a resolved binding. Callable targets require the
selected `CallOutcome::Resolved` overload at that exact file and head location;
lexical overload candidates are never followed as proven edges. An intrinsic is
a known endpoint with no declaration. `uses` compares exact declaration IDs
against retained occurrences across all files. Uncertain targets make its use
set incomplete, even if no occurrence can be retained. Candidate identities
remain available in the native facts and reference JSON.

`transitive_references(n)` starts from selected declarations or proven reference
targets. Each wave enumerates references fully contained in a declaration's
written owning region, including types, signatures, defaults, initializers and
nested bindings, then follows proven targets. `n=0` emits no references. Roots
retain incoming order; within a root, references use retained FileId, original
range and reference-kind order (value, callable, type). Targets expand in first
discovery order. Each stage expands a DeclarationId once and emits a reference
once by FileId/range/kind, retaining its first occurrence. Distinct declarations
sharing a CST node stay distinct. Direct `references`, `declarations` and `uses`
retain incoming-root order and duplicates; `unique` explicitly removes repeated
semantic identities. Reaching the requested depth defines a bounded result.
These are static written relationships and establish neither flattening
dependencies nor full transitive closure, compiler validity or satisfiability.

`types` accepts declaration/reference streams or selected CST nodes. An exact
owning declaration node expands into its declaration identities; other nodes
require an exact expression fact. References use their exact occurrence facts;
call heads use selected call returns. A different expression cannot inherit a
same-named declaration's type. Native `TypeInst` retains instantiation,
optionality, enum DeclarationId and recursive set/array/tuple/record components.
Text emission of type rows gives one kind per line. `instantiations` similarly
emits `par`, `var` or `unknown`; native rows keep the producer's unknown reason.
Declaration instantiation uses the binding fact independently of full typing.
Expression facts may also establish instantiation while typing is unavailable.

Semantic predicates are `type("KIND")`, `instantiation("par"|"var")` and
`source_kind("user"|"standard_library")`. Source kind comes from the retained
ModelFile. Type kinds are `bool`, `int`, `float`, `string`, `annotation`, `enum`,
`set`, `array`, `tuple`, `record` and `bottom`. A type with unknown instantiation,
an unresolved type variable or any unknown nested component yields unknown.
Filters keep only proven true rows. `not` preserves unknown; `and` and `or` use
three-valued Boolean rules and short circuit. An encountered unknown remains a
located report limitation even if another branch establishes the filter result.
`kind("declaration")` and `kind("reference")` distinguish semantic identities;
`name` compares the retained identifier identity. Semantic rows accept
`filter`, `head`, `count`, `names`, `text`, `unique`, `emit` and `json`; projected
strings also accept `tally`. Unrelated CST navigation and data edits produce
located stage-type errors.

Semantic JSON always uses `{ "complete": BOOL, "limitations": [...],
"result": VALUE }`, including counts and empty selections. Rows include actual
file/FileId, source kind and original location. Declaration `range` locates its
name; `syntax_range` locates its owning source, rendered as `text`. Reference
ranges/text locate the written occurrence. Type rows add structured `type`;
instantiation rows add `instantiation` and `reason`. No whole fact table is
serialized. Unknown/unresolved/ambiguous/unsupported outcomes and loader
limitations survive projections and make the report incomplete. Status 1 emits
its result plus located stderr notes; model/dependency errors are status 2 with
empty stdout. An unconfigured standard library remains a limitation even when
some facts are available. This differs from declaration-only data reduction.

Fact families are computed only on demand, once per evaluation. The producers
scan a whole retained context; before calling them, the query bounds its token
and CST-node scan by work and collection limits. Reference scans, target lookups,
expansion, row additions, type components and output bytes are charged too.
Nested type inspection respects `Limits::nesting`. Exceeded limits are actual
errors and never silent truncation.

Use the library independently:

```rust,ignore
use zincite_query::{Input, Limits, Query, QueryResult};
use zincite_syntax::FileMode;

let input = Input::parse(b"'capacity' = 2;\nused = 1;\n".to_vec(), FileMode::Data)?;
let limits = Limits::default();
let query = Query::parse("filter(name(\"capacity\"))", limits)?;
let result = query.evaluate(&input, limits)?;
if let QueryResult::Selection(selection) = &result {
    // SelectedItem exposes kind, name, original item/source ranges and the CST node.
    let item = &selection.items()[0];
    assert_eq!(&input.source_bytes()[item.range.clone()], b"'capacity' = 2;");
}
let original_fragment = result.render();
```

`QueryResult` keeps item/node/literal selections, strings, counts, histograms,
`Document(Vec<u8>)`, `Reduction(ReductionResult)` or `Semantic(SemanticResult)`
native until `render`. `JsonResult::result` exposes the native result behind a JSON emitter.
`NodeSelection` exposes original CST nodes, file identities and byte ranges.
`Document` contains a complete validated edit candidate. `Query::evaluate(input,
limits)` uses no model; `evaluate_with_model(input, Option<&ModelContext>, limits)`
uses optional read-only facts for enum reduction or required retained-root facts
for semantic inspection, and rejects relevant loader errors. Its signature borrows
both `Input` and `ModelContext` for the result lifetime. `SemanticResult` exposes
its final `SemanticStream`, completion, located limitations, immutable model and
only computed fact families. Declaration/reference IDs belong to its `BindingFacts`;
selected overloads remain in `CallableFacts`. Type and instantiation rows retain
native subjects and values. Included source reads its actual ModelFile bytes;
original locations include that file's BOM exactly once. Owning declaration
`syntax_range` in native binding facts remains BOM-stripped. `ErrorLocation::Model(SourceLocation)` retains original model/include
locations for actual failures. A
selection borrows its `Input`, its item CST nodes and identifier identities. `Input` exposes
original bytes, token bytes/ranges and source locations. `SelectedItem::range`
and `source_range`, `Input::token_range` and all input diagnostics use original
byte coordinates. The analysis CST from `Input::syntax` excludes a leading BOM;
add `Input::syntax_offset` to its ranges. Opaque comment bytes appear as markers
in its UTF-8 analysis text, so use original byte accessors for source spelling.

Queries have a hard limit of 1,048,576 UTF-8 bytes and 64 nested parentheses/`not`
operators. Flat `and`/`or` chains do not consume nesting depth. `Limits::nesting`
can lower that ceiling. Default evaluation limits are 100,000 collected entries
and 1,000,000 visits: each initial/reset item, stage, filtered item and visited
predicate costs one visit. Edit traversal costs one visit per inspected token;
replacement validation and each expansion charge the replacement byte length.
Navigation charges node visits and child slots before scanning; projections charge
inspected slots and copied string bytes; text conversion and annotation-head
inspection charge their source-span byte lengths before scanning. Node fragment
emission charges the expanded source byte length. JSON charges selected text/string bytes
and repeated file-label bytes before rendering. Every expanded stream, including
overlapping duplicates, respects the collection limit. Recursive literal views charge
visited child slots and expanded values/keys/rows, and use the nesting ceiling.
Comparison decoding charges inspected spelling bytes. Literal JSON charges every
recursive text/file span before rendering.
Boolean evaluation short circuits. Checks occur before collection additions and visits; exceeding a bound returns a located
error without truncating the result. Library callers can change work/collection
limits. Query nesting is checked during parsing; literal nesting and work/collection
limits are checked during evaluation.

The commands take one query argument and one optional file or `-`.
`--stdin-filepath PATH` selects stdin's language mode and diagnostic label;
without it stdin uses model mode. Extra files and directory inputs are rejected.
`--model PATH` supplies a retained root for semantic inspection or read-only
model declarations for enum reduction. Includes
search the including directory, ordered `-I DIR` directories, then the configured
library's `std` directory. `--stdlib-dir DIR` overrides `MZN_STDLIB_DIR`. Syntax
inspection does not load a supplied model or require a library. A loader note
about unavailable builtins alone does not make a declaration-only reduction
incomplete; missing facts required by the reduction do. Query diagnostics identify
`<query>` line/column and query byte ranges; input diagnostics identify the input
path and original source byte ranges. Both command forms buffer a complete result
before writing stdout. Status 0 means complete, status 1 means an incomplete
semantic report or reduction candidate, and status 2 means an actual
query/input/model/usage/I/O error.
Actual failures leave stdout empty. Incomplete source/diff previews emit the
candidate with located dependencies on stderr; incomplete `--write` emits no
source and preserves the exact original file.

Editing prints the candidate by default and never writes implicitly.
`--diff` and `--write` require an editing pipeline and are mutually exclusive.
Diff accepts stdin and buffers a whole-file unified preview. Write requires a
complete result and the explicit regular file, rejects stdin and symlinks, checks exact original bytes
again before rename, and preserves file permissions through a completed sibling
temporary. Successful writes emit no source. Invalid or stale candidates leave
the original unchanged. Includes and model files are never implicit write targets.

The shared edit API `zincite_lint::prepare_text_edits(snapshot, current_source,
edits)` applies one atomic `TextEdit` batch without fix metadata or lint eligibility.
It accepts an empty batch, retains original copy bytes and rejects stale source,
invalid ranges/copies and overlaps. The caller validates the complete candidate.
`replace_source_file(snapshot, candidate)` checks syntax and uses the existing
single-file replacement operation. Lint's `prepare_edits`, `prepare_fixes` and
`replace_fixed_file` retain their fix metadata, eligibility, conflict omission
and suppression checks.

## Formatter library

Use the libraries independently of the command:

```rust,ignore
let parsed = zincite_syntax::parse("int: count=2; constraint true; solve satisfy;");
let formatted = zincite_fmt::format(&parsed)?;
let data = zincite_syntax::parse_with_mode("count=2;", zincite_syntax::FileMode::Data);
let formatted_data = zincite_fmt::format(&data)?;

// The default wrapper uses four spaces, four-column tabs and 120 columns.
let options = zincite_fmt::FormatOptions {
    indent_style: zincite_fmt::IndentStyle::Tab,
    indent_size: std::num::NonZeroUsize::new(4).unwrap(),
    tab_width: std::num::NonZeroUsize::new(4).unwrap(),
    max_line_length: std::num::NonZeroUsize::new(80),
    ..zincite_fmt::FormatOptions::default()
};
let formatted = zincite_fmt::format_with_options(&parsed, &options)?;
```

The formatter returns owned syntax or formatting-directive diagnostics on error.
It produces no partial formatted source. Standalone `% zincite-fmt: skip` preserves
the next complete top-level item; `% zincite-fmt: off` and `% zincite-fmt: on`
preserve a region between complete items. These spans retain their marker lines,
boundary whitespace, comments and original line endings exactly, including EOF
whitespace. Skipped syntax is still checked. Nested, unmatched, misplaced,
malformed and dangling markers fail the whole file. The library's
`protected_ranges(&parsed)` exposes the validated byte spans for sorting barriers.
Adjacent include groups sort by decoded path, case-sensitively, with `globals.mzn`
first and equal paths retaining their order. Written paths and attached comments
are preserved. Blank lines, separated comments, other items, formatting directives,
lint suppression targets and skipped spans keep groups separate. Interpolated
include paths stay in place because sorting does not evaluate expressions or load
included files.

Indentation size and tab width are nonzero column counts;
tab indentation fills complete tab stops and uses spaces for any remainder.
Width counts Unicode scalar values as one column and tabs to the next configured
tab stop. Set `max_line_length` to `None` for unlimited width; mandatory blocks and
explicitly expanded lists still expand. The library also exposes `line_ending`
(`Lf`, `CrLf` or `Cr`), `insert_final_newline` and `trim_trailing_whitespace`;
the defaults use LF and enable final newline insertion and trimming. Comments,
literal chunks and skipped spans retain their bytes even when these settings
would otherwise change them.

## Lint library

Use the library independently with `zincite_lint::lint(&parsed)`. It returns
warnings with rule, severity, message and byte range, or owned syntax/suppression
diagnostics without partial warnings. `LintOptions::from_selection` resolves
presets and explicit IDs; `lint_with_options(&parsed, &options)` runs a selected
set. `Rule::DEFAULT` and `Rule::THESIS` expose preset membership. The default
selects naming and constraint labels; `thesis` selects fourteen rules and `all`
selects all 26 implemented rules. Semantic selections require the explicit model
API below; `lint_with_options` rejects them without that context.

Explicit model analysis uses `load_model(root_path, &ModelOptions)` and
`analyze_model(&context, &LintOptions)`. The context retains parsed sources,
canonical file identities, include locations and file-local suppressions. It
resolves includes from the including file's directory, then the ordered
`include_dirs`, then `stdlib_dir/std`. A configured library also loads
`std/stdlib.mzn` and its core closure as implicitly available declarations.
Standard-library source identity remains separate from explicit input selection;
system sources receive style warnings only when explicitly selected. Consumers
use the retained CST to identify declarations rather than inferring builtins
from names or bodyless user declarations.

`ModelOptions` takes its configuration from the caller. The CLI accepts ordered
`-I DIR` options and `--stdlib-dir DIR`, with `MZN_STDLIB_DIR` as the fallback for
the latter. The default rules never load dependencies, even with these options.
Implemented rule availability is separate from a particular analysis's
`Completed`, `Limited` or `Inapplicable` outcome.

Selected analysis separates file-aware findings, per-rule execution outcomes,
model limitations and actual errors. Dynamic include paths produce an explicit
incomplete-context limitation; missing or cyclic dependencies, invalid UTF-8,
syntax and suppression failures produce errors. `write_analysis` supplies the
CLI's stderr renderer: limitations alone keep status 0 or 1, warnings produce 1,
and errors take precedence with 2. Unsupported required semantics remain
explicit limitations; analysis does not guess missing parameter values.
