# Syntax and library reference

This reference describes accepted MiniZinc syntax, query operations and the
Rust library interfaces. Zincite targets MiniZinc 2.10.1 model (`.mzn`) and
data (`.dzn`) files.

For worked command examples, start with [installation](getting-started.md),
[formatting](formatting.md), [linting](linting.md), [queries](queries.md) or
[data edits](data-edits.md). [Support limits](support.md) separate current
capabilities from incomplete analysis and planned work.

## Syntax support

This describes supported syntax families, not complete compiler compatibility.
The parser supports form-feed whitespace, escaped single quotes in strings,
deprecated declarations without the `function` keyword, anonymous enum
constructors, standalone equality items and callable annotation capture. The
[syntax reconciliation](../scripts/syntax-corpus-reconciliation.md) records their
corpus and compiler checks. See the [acceptance report](../scripts/corpus-acceptance.md)
for the checked inventory, remaining coverage limits and final validation status.

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
Comments may occur between tokens. The parser keeps their original spelling,
as well as literal/operator spelling and explicit parentheses. Formatting
changes layout and sorts adjacent include groups; it does not load includes
or check types. See the [formatting style guide](formatting-style.md) for the
layout rules and before-and-after examples.

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
`variant_record` and `case` are reserved keywords without productions in the
pinned grammar; they remain unsupported and produce diagnostics.
Range-type bounds follow the numeric-expression
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
Type-inst concatenation examples establish syntax and format stability only:
MiniZinc rejects an enum-target `type Joined = Left ++ Right` with a type error.
Zincite retains the pinned concatenation syntax without imposing that semantic
restriction on parsing.
Written indices and cardinalities are retained without evaluation, domain inference,
contiguity checks or rectangularity checks. The pinned grammar permits an empty
index tuple `()` as a key, although the installed MiniZinc 2.10.1 compiler rejects
it. That compiler also crashes on a mixed binder/plain-index declaration during
`--model-check-only`; Zincite accepts these forms from the specification.

The [MiniZinc 2.10.1 grammar](https://docs.minizinc.dev/en/2.10.1/spec.html#full-grammar)
is the syntax target. The grammar/compiler differences above still apply:
accepting its syntax does not establish full compiler compatibility.
`variant_record` and `case` have no productions in that grammar. JSON data
and solver output are outside the `.mzn`/`.dzn` inputs.

Data assignment expressions
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

## Development report transformations

The development query adapter pins Zirql Git revision
`90d184e9376a5ab039eecba0e62f3f1afd778a9b`. It supports positional `map` and
`flat_map`, selective `case` generators, `windows`, `runs_by`, record updates
with `with`, and full `align` by a unique field. The
[report transformation example](query-comprehension-contract.md#transform-report-rows)
shows their syntax and required checkout. Text programs and
`Program::from_values` use the same evaluator and retain native selections
inside report records. Data edits keep their closed expression subset.

Declaration metadata in this development adapter uses Zirql's budgeted copy
operation, preserving record order and empty element contracts before wrapping
it in Zincite's `Rc` storage. Common scalar arguments use shared type-fact
admission; preparation may defer unknown facts, while evaluated values must
satisfy their scalar contract. Zincite retains its own classification of native
scalar/value wrappers, MiniZinc model admission and Data edit restrictions.
Counts and source-copy bounds use the shared located nonnegative integer
conversion. See [the adapter API contract](https://zayenz.se/zirql/guides/host-integration/#declaration-metadata-and-scalar-arguments).

## Query library

`zincite-query` selects top-level items from one model or assignment-only data
file and explicitly edits data assignments and enum membership. Its reusable
library and both commands use the same pipeline stages, listed below.

Zirql supplies shared evaluation, while this library supplies MiniZinc inputs,
model facts, source output and data edits. To build a host for another domain,
see [Zirql's embedding guide](https://zayenz.se/zirql/guides/host-integration/).
To call Zincite itself, use the APIs below.

| Stage | Result |
| --- | --- |
| `items` | Reset to all top-level items in written order |
| `filter(predicate)` | Keep matching items in their existing order |
| `head(n)` | Keep the first `n` items of the computed selection |
| `tail(n)` | Keep the last `n` rows in their current order |
| `reverse` | Reverse current row order, retaining duplicates |
| `check`, `check(n)`, `check(message)`, `check(n, message)` | Require nonempty or exactly `n` rows, then pass the stream through |
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
| `emit` | Render selected original source bytes or the complete reparsed edit candidate |
| `set_value("EXPRESSION")` | Replace selected assignment right-hand sides in data mode |
| `filter_elements(comparison)` | Explicitly filter literal set, array or matrix elements in data mode |
| `remove` | Remove selected data assignments and their attached comments/directives |
| `reduce_enum("Guests", keep("A", "B"))` | Keep explicit enum members and reduce supported dependent arrays, sets and group accesses |

Stages are separated by `|`. The initial selection is `items`, including for an
empty query. Source or projected-string emission is implicit. `emit` and `json` are terminal.
In positional query syntax, only `json` may follow `count` or `tally`. A pipeline may contain one transformation stage,
followed only by its optional `emit`; document emission is implicit for
edits. `emit_document` remains a compatibility alias requiring a transformation.
`head` does not avoid parsing or evaluating the earlier selection. No input items
produces an empty selection or count 0; the untouched selection of a comment-only
document still emits its original bytes.

`tail(0)` returns no rows; a bound larger than the stream keeps all rows.
`reverse` uses the stream's current order and retains duplicate occurrences.
These controls preserve native owners, original source ranges and semantic
limitations. Reversing item fragments also moves their attached comments;
unsafe formatter directive pairing remains an error.

Bare `check` and `check(message)` require at least one returned row.
`check(n)` and `check(n, message)` require exactly `n`, including `check(0)`
on an empty stream. Success returns the original stream unchanged. Failure
reports the expected and actual counts with the optional message. `check()`
is invalid; `reverse` takes no parentheses. Positional queries require literal
nonnegative integer bounds and double-quoted messages. In programs, counts and
messages can be scoped expressions; a single `check(value)` accepts an integer
count or a string message. Known wrong types fail during preparation, including
unused code and empty selections.

A cardinality check counts returned rows, without proving complete semantic
knowledge. A literal array is one row unless `elements` selects its contents.
The Boolean `require(condition, value, message)` expression keeps its separate
contract. These native controls do not accept constructed arrays or records and
do not expand the stages allowed in data-edit programs.

Predicates are `kind("assignment")` and `name("capacity")`, combined with
parentheses and `not`, `and`, `or` in that precedence order. Query strings are
double quoted and accept `\"`, `\\`, `\n`, `\r` and `\t` escapes. Other escapes
and literal control characters are errors; Unicode characters may be written
directly. Whitespace between query tokens is ignored. Query comments are not
implemented. Read-only programs add bindings and reports around these pipelines;
see [query programs](#query-programs).

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
`tally` counts equal strings and emits a JSON object in first-seen key order;
an empty string stream yields `{}`.

Item filters keep the kind aliases above. Navigated node filters and JSON use
original CST kinds in snake_case, such as `call_expression`,
`annotation_declaration`, `annotation`, `solve_minimize` and `solve_maximize`.
`name` on navigated nodes compares the first direct identifier identity.
`head`, `tail`, `reverse`, `check` and `count` accept node, literal or string
streams. `unique` accepts node or string streams. Navigation,
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
`filter`, `head`, `tail` and `reverse` produce fragment selections, even if every
item survives. Successful `check` preserves the selection, including
whole-document output.
Fragments emit original item bytes and attached comments in the selected order:

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

### Required query parameters

Query programs declare required scalar inputs with `param name: int;`,
`param name: string;` or `param name: bool;`. Pass each value with a repeated
`--param name=value` option in either command:

```sh
zincite-query --param factor=2 --program '
param factor: int;
let d = items | filter(name("demand")) | values | one;
sum([n * factor | n in d]);
' \
  data.dzn
zincite query --param factor=2 --program-file capacity-parameters.zq data.dzn
```

The companion program [capacity-parameters.zq](../crates/zincite-query/tests/fixtures/capacity-parameters.zq)
uses the capacity report's inputs `demand=[2,3,4]; capacity=[6,8,5];`. For
`factor=2`, total demand is `18`, total capacity is `19`, headrooms are
`2,2,-3`, and only the third row violates its capacity. The result is complete
with no limitations. Inline and file programs use the same evaluator.

CLI integers are signed 64-bit ASCII decimal values with an optional `+` or `-`;
whitespace, fractions and overflow fail. Booleans are exactly `true` or `false`.
Strings contain everything after the first `=`, including empty text, quotes,
backslashes, whitespace or additional `=` characters. Shell quoting protects
arguments; their contents are never interpreted as query source.

Declarations have no defaults. Every declaration needs exactly one input.
Duplicate declarations or inputs, missing inputs, extra names and type
mismatches fail before native query work or data construction. Names use the
shared ASCII binding rules and cannot shadow reserved names or earlier visible
bindings. A declaration becomes visible at its written position; forward
references and shadowing it in a local comprehension fail at the name.

Native pipeline stages retain their original program coordinates.
Checked primitive arguments such as `name(selected)`, `head(limit + 1)`,
`tail(limit)`, `check(expected, message)`,
`range(start, end)` and `transitive_references(depth)` use the existing MiniZinc
expression subset and shared evaluator. String `kind`, `type`, `instantiation`
and `source_kind` predicates also accept checked expressions. Native accepted
names, nonnegative indices and range order are still checked. Known wrong
types fail before a query executes, including empty or unchosen predicates.
`items | filter(name(field)) | values | one` reads the one assignment selected
by a checked string name.
Positional pipelines have no parameter declarations and reject `--param`.

Rust `Program::parameters()` exposes declaration metadata.
`evaluate_with_parameters`, `evaluate_with_model_and_parameters` and
`evaluate_with_facts_and_parameters` accept `ParameterInputs::Typed` scalar
pairs or `ParameterInputs::Cli` raw strings alongside `ProgramLimits`:

```rust
use zincite_query::{ConstructedValue, ParameterInputs, Program, ProgramLimits};

let limits = ProgramLimits::default();
let program = Program::parse("param factor:int; factor;", limits)?;
let result = program.evaluate_with_parameters(
    &input,
    ParameterInputs::Typed(&[("factor".to_owned(), ConstructedValue::Integer(2))]),
    limits,
)?;
```

Evaluation methods without a parameter argument supply an empty input set. Input conversion and validation use the same work and result budgets as
evaluation and rendering. Parameterized data edits have the same supported
types, calls and candidate checks as other edits. Any failure leaves CLI
stdout empty and the input file unchanged.

### Query programs

`Program::parse(source, ProgramLimits)` accepts ordered read-only statements or
one atomic data edit batch. Use `let name = expression;` for immutable input
values and calculations, including native pipelines. A name becomes visible
after its binding, and names cannot be reassigned. Input reads and calculations
can interleave; every input read uses the original document.

An expression outside a binding emits its result. Native selections retain
source output, counts print numbers and calculated values or records produce
JSON. `report expression;` explicitly selects JSON for a native result. It is
useful when an intermediate selection should be inspected as structured data.
All standalone expressions emit, including earlier expressions in a program;
use `let` to retain an intermediate result without printing it.

Bindings and earlier statements require semicolons. The final expression may
end at EOF without one. Data edit batches require the explicit final `emit;`.

```zirql
let selected = declarations;
let n = selected | count;
let histogram = selected | names | tally;
(count: n, histogram: histogram, selected: selected)
```

Record fields use identifiers or nonempty single-quoted labels and preserve
written order in `ReportValue::Record`. Duplicate labels fail. Parentheses also
group expressions.
A binding can hold a source selection, count, histogram or nested record. Counts
render as integers; histograms render as string-to-count objects. Selections
retain their original nodes or declaration/reference identities, file owners and
ranges. A constructed record has no input-source range. These inspection leaves
are not scalar comprehension values.

`expression | names` runs a stage against that expression's native result.
`| emit` and `| json` are terminal emitters. An explicit emitter suppresses the
statement's implicit emission. Bindings and record fields cannot emit. Program
parsing checks names, emitters and supported result transitions before execution.

#### Patterns and pure control values

Immutable `let` bindings and function parameters accept names, `_`, nested
exact tuples, positional list patterns and records, scalar literals, `present(pattern)` and
`absent`. Record patterns use `{label, other: renamed}`; labels may be quoted
strings. The shape is exact unless a final `..` ignores extra fields. There is
no bound rest value. Tuple length is always exact. `[a,b]` requires exactly two list elements;
`[a,b,..]` accepts a longer list and discards the rest. Lists remain distinct
from tuples, sets and keyed arrays. Bare `present` is an ordinary name;
`present(...)` and bare `absent` are presence patterns. Quote a binding named
`absent`.

```zirql
let {pair: (left, right), "label": title, ..} =
    (pair: (3, 4), label: "sum", unused: true);
report if left < right { left + right } else { 0 };
```

Each initializer and argument runs once in written order. A function checks
each parameter's full type before destructuring it. A mismatch reports an error
at the pattern; it never filters a selection. Names become visible only
after the whole pattern succeeds. Duplicate names and prohibited shadowing fail
before execution, including in unused arms.

`match(subject) { pattern [if guard] => value, ..., _ => fallback }` evaluates
its subject once and chooses the first successful pattern with a true guard.
Shape, literal and presence mismatches try the next arm. A guard must return a
known Boolean. Absence, a non-Boolean result, Unknown or an evaluation error
stops the request, as do failed input checks and exhausted limits. Matching
does not catch errors. `match { from ... }` enumerates native captures.

Function bodies, conditional branches and match arms accept blocks with local
`let pattern = value;` declarations and one final value. The common spelling is
`if condition { ... } else if condition { ... } else { ... }`, with a final
else. MiniZinc `if ... then ... else ... endif`, records, operators and
comprehensions keep their query-expression syntax. Only the selected bodies run,
but all bodies are checked for valid scope, types and permitted operations before
execution. A compatible function call at the end of a branch or arm can be a tail
call. Blocks and guards cannot emit or edit data.

Rust callers can use
`Program::from_values(functions, bindings, final_report, limits)` to construct a
program without generating and parsing query text. `ProgramFunction` stores a
shared function declaration whose names have not yet been resolved.
`ProgramValue` is a shared value with a source location; its native leaves use
`zirql::Expression<Query, (), ()>` wrappers. Bindings are ordered
`(zirql::Pattern, ProgramValue)` pairs. Construction takes ownership of all
input trees, even if construction fails, and releases shared wrappers
iteratively. Caller-supplied source ranges are preserved. Evaluation uses the
current input and model, with the same checks as a parsed program. Native fields
preserve their capture views, handles and source coordinates.

These constructs are report-only. Data edits reject them, including in unused
bindings and skipped branches. The [pattern examples](queries.md#destructure-a-report)
show a destructured native report and an optional guarded match through both
query commands.

#### Explicit loops and worklists

`loop (name = initial, ...) { body }` binds immutable state. Initializers read
the outer scope. Every body path ends in `break result` or `continue (next, ...)`;
all next values read the old state, then replace it together. State contracts
are established at initialization and checked on every advance. Nested transfers
target the innermost loop, and transfers cannot cross a function boundary.
Iterations spend work without adding call depth; nontermination ends with the
ordinary located work-limit error.

`fold(items, initial, step)` and `scan(items, initial, step)` call a named pure
function as `step(state, item)` from left to right. Both evaluate items and
initial once. The step's two parameter declarations, purity and state result
contract are checked even for empty input. Fold returns initial for empty input;
scan returns the post-step states, excludes initial and is empty for empty input.
Native earlier states retain their owners and remain unchanged.

`queue(items)` infers its element contract from admitted nonempty input;
`queue<T>(items)` supports empty input. `pop_front(q)` is absent when empty or
present with `(first, remaining)`. `push_back_all(q, items)` appends in source
order. `identity_set<Declaration>()` and `identity_set<Node>()` create empty
sets; `item in seen` tests native identity and `insert(seen, item)` is idempotent.
Queue/set updates preserve retained aliases. Optional-pop decomposition borrows
tuple fields and shares the remaining queue's roots.

`same_declaration(a, b)` uses the exact retained binding owner and declaration ID,
matching identity-set membership. `reference_info(d)` returns an explicit native
record with `targets`. Targets use the established `referenced_declarations`
meaning: proven targets of references wholly inside the owning CST region,
ordered by file, source range and reference kind, with first-target deduplication.
Known intrinsic endpoints have no declaration target. Unresolved, ambiguous or
unsupported endpoints refuse at their model location, including a repeat lookup
after an earlier diagnostic. Legacy `referenced_declarations` keeps its partial
result and limitation behavior. No model function or solver executes.

Queues and identity sets can remain in library results, tuple fields and scan
states. Their direct or nested rendering fails at the computational expression's
query range. Inspect native results through `ReportValue::Computational`,
`ReportValue::Tuple` and `ReportValue::Sequence`; `len()`, `values()` and `range()`
borrow their retained data. `ReportValue::Retained` transparently retains an
owner; `fields()` and `stream()` already follow it. Native declarations and
nodes retain their source coordinates. Metadata, fresh elements, native scans,
operations and rendering spend the same cumulative request allowance. See the
[executable graph and reducer examples](queries.md#walk-declaration-references).

#### Typed query functions

Read-only programs accept named pure functions, including direct and mutual
recursion. Declarations can appear before or after their callers. Each function
has typed parameters, a result type, ordered local `let` bindings and one final
value. MiniZinc `if`/`then`/`elseif`/`else`/`endif` and comprehensions keep their
ordinary query-expression spelling. The shared `if condition { value } else
{ value }` form is also accepted.

```zirql
fn accumulate(n: int, total: int) -> int {
    if n = 0 then total else accumulate(n - 1, total + n) endif
}
accumulate(1000, 0)
```

This reports the tagged integer `500500`. A compatible tail call reuses its
active frame, including a mutual tail call. A pending operation such as
`1 + accumulate(n - 1, 0)` needs another frame. Functions have a separate active
call-depth limit of 64; library callers can use
`Program::with_function_limits(zirql::FunctionLimits { active_depth: 1 })`.
The example succeeds at depth one with the default work allowance.
Non-tail calls refuse at the limit. Tail calls still spend work, so a
nonterminating function eventually refuses. Diagnostics keep the original
query or model location and append bounded function-call context.

| Signature form | Accepted values |
| --- | --- |
| `bool`, `int`, `float`, `string` | Checked fixed scalars. |
| `value` | Any constructed value; excludes native handles and computational worklists. |
| `dict<K, V>` | Constructed keys and values matching their declared types. |
| `EnumName` | A member of that actual enum in the current retained model. |
| `opt T` | Absence or a value matching `T`. |
| `sequence<T>` | A homogeneous positional sequence, including an empty sequence with its element contract. |
| `tuple<T, U>` | Fixed positions with the declared types. |
| `record{label: T, other: opt U}` | Declared labels; optional fields may be omitted. |
| `set<T>` | Boolean, integer, float, string or nominal elements; `set<value>` admits mixed and nested constructed values. |
| `keyed<T>` | A keyed array whose cells match `T`, preserving its axes. |
| `Node`, `Declaration` | One native syntax node or semantic declaration. |

Constructors nest. Native node/declaration sequences retain their owners and
original coordinates; a singleton and a sequence are distinct contracts.
Native capture records can pass through `record{...}` and
`sequence<record{...}>` without rendering their handles. Constructed records,
tuples, arrays and nominal values retain their existing checked representation.
Enum names resolve once per evaluation against its current `ModelFacts`,
including nested and set signatures. A name alone never establishes enum
identity. A `.dzn` request with nominal signatures needs `--model`.

A function sees its parameters, earlier local lets, named query functions and
registered pure operations. It cannot capture top-level bindings or parameters,
shadow an existing local, emit, edit data, or call a model function. Every
written body, native argument and branch is preflighted before statements run,
including unused declarations and skipped branches. Argument and result
contracts check actual selected data. A failure returns no partial report.
Function declarations and calls are excluded from atomic Data programs,
including unused declarations and skipped expressions.

Three pure operations inspect explicit native nodes:

| Operation | Result |
| --- | --- |
| `direct_children(node)` | `sequence<Node>` in direct containment order. |
| `node_kind(node)` | The CST kind string, such as `call_expression` or `generator_call_expression`. |
| `written_call_name(node)` | `opt string` containing the direct written call name. |

These operations inspect syntax. They do not resolve or execute a MiniZinc
model call or run a solver. [Count written calls](queries.md#count-written-calls-with-a-function)
contains a complete recursive program and input. Declaration navigation still
uses the semantic adapters, retaining incomplete facts and located Unknown
refusals. Calls in projection fields, computed native arguments, dependent
match sources, guards and path targets share the same prepared request and
active call-depth allowance. Later match clauses see only earlier captures.
Boolean guards and selected fallbacks remain lazy at runtime.

A parsed `Program` can be reused with different inputs. Each evaluation prepares
the function bodies once. Arguments and locals are immutable shared values, so
repeated calls with the same type contract avoid copying their contents or
rescanning native arguments. The first check validates every native element,
its source owner and coordinates. A new value or changed contract requires
another check.

Creating values, snapshotting emissions and rendering output still consume the
request budget. Reusing a parsed `Program` after an error starts a fresh
evaluation with the current input, facts and limits.

#### Bounded declaration paths

Read-only native programs accept
`paths(referenced_declarations, to=predicate, max_depth=n)`. The bound is a
required nonnegative integer literal measured in declaration-to-declaration
edges. The target uses ordinary semantic predicates and shared Boolean control.

Each edge follows a proven target of a reference written anywhere in the owning
declaration region, using `transitive_references`' file, range and reference-kind
order. Resolved value/type bindings and exactly selected resolved calls can
produce declaration edges; intrinsic endpoints produce none. Repeated targets
keep their first occurrence. Reference occurrences are not extra path steps.

Paths are evaluated breadth-first by Zirql. It returns one shortest witness
per source/sink pair, suppresses repeated source identities and cycles, and uses
edge order to choose equal-length ties. Matching sinks can lead to further sinks.
Rows have native `source`, `sink` and ordered `steps` fields; steps include both
endpoints. A matching root has `[source]` at depth zero. Empty path collections
retain their field schema for later projection.

Paths require declaration selections and never guess a target by spelling.
Unresolved edges retain located limitations, `complete:false` and status 1.
A final Unknown target, including under `not`, refuses at the responsible model
location. A known Boolean alternative may retain a witness and its limitations.
No witness within a bound does not establish global disconnection.

Source and neighbor scans, comparisons, traversal storage, native copies,
returned paths and rendering share one program budget. Paths remain
report-only and are excluded from data expressions and preceding edit lets.
See the [declaration-path example](queries.md#explain-declaration-connections).

#### Native projections and captures

Read-only programs also accept domain stages directly:

```zirql
subtree | declarations | unique | project(d, {
  "name": d | names | one,
  "uses": d | uses | count
});
let matches = match {
  from d in (subtree | declarations | unique);
  from u in (d | uses);
};
matches | project(m, {"name": m.d | names | one, "text": m.u | text | one})
```

`project` returns a positional array of constructed records in native source
order. Field labels are static quoted strings. The explicit subject is local
to the projection and cannot shadow a visible binding. Native handles need
`names`, `text`, `count` or another supported scalar projection before entering
a constructed field; `one` extracts exactly one projected value. Ordinary MiniZinc
records and field access use the syntax shown in the value examples below.

`match { ... }` returns `ReportValue::Captures`. `CaptureMatches::rows()` borrows
its ordered `zirql::NativeCapture<ReportValue>` rows. The retained schema checks
field access even when there are no rows. `m.d` preserves the field's native
kind and identity; it does not render or convert the handle. The enclosing
`ProgramResult` retains its model fact owner. A singleton capture exposes its
fields through the existing `ReportValue::fields()` view.

Clauses run in written order. `from name in query;` enumerates native subjects;
`bind name = query;` requires exactly one. A capture is absent in its own
operand. Duplicate names, forward references and ambient shadowing fail before
enumeration. `where type(d, "tuple");`, `instantiation(d, "par")`, `name(d,
"capacity")`, `kind(d, "declaration")` and `source_kind(d, "user")` adapt the
existing domain predicates to explicit subjects. Boolean wrappers use shared
short-circuit control. A final Unknown required guard refuses at the
responsible model file/range with its reason. Known matches from incomplete
input retain limitations and the `complete:false` report envelope.

Projection fields and match operands are pure. Empty inputs and unchosen
Boolean branches still undergo scope/type/purity checks. Candidate expansion,
native copies, capture metadata, scalar conversion and rendering spend one
continuing request allowance. These constructs are report-only: an editing
request refuses them in preceding lets or edit expressions. See the
[declaration/use example](queries.md#correlate-declarations-with-their-uses)
for inline and file commands.

#### Values and report expressions

`items | filter(name("assignment")) | values | one` reads exactly one written
literal RHS into a value. `values` projects a sequence of literal values;
`one` requires exactly one projected value and extracts it. An array RHS is
one value regardless of its element count. Missing or duplicate matches fail at
`one`; computed RHSs retain their located refusal. The same `let` spelling binds
calculated values. For input `weights = [3, 5];`,
this program doubles each weight and keeps its original position:

```zirql
let weights = items | filter(name("weights")) | values | one;
let rows = [(index: i, doubled: weights[i] * 2)
  for i in index_set(weights)];
rows
```

The rows contain `(index: 1, doubled: 6)` and `(index: 2, doubled: 10)`.

Zirql reads and evaluates scalar expressions, scopes and generators. Zincite
retains the input-file CST, native operations and model-backed enum lookup. `[value for pattern in source if condition]` and
`[value | pattern in source where condition]` use the same generators. Later
generators see earlier bindings. Multiple
binders share one source, with the last binder varying fastest; `x = expression`
binds one value per preceding environment. `_` discards its binding. Filters
require fixed Booleans and charge each candidate before filtering.

Pure values include signed 64-bit integers, finite binary64 floats, Booleans,
strings, resolved enum members, positional/keyed arrays, matrices, tuples,
ordered records, general dictionaries and sets of constructed values, including
strings and nested or mixed values. Data edits retain their narrower supported
MiniZinc shapes; native containers retain their host-specific support.
Supported operations include checked arithmetic, exact
numeric promotion, comparison, equality, membership, concatenation, access,
finite inclusive integer ranges, rank-one `index_set`, `enum_member`, report-only
`length`/`sum`/`forall`/`exists`/`require`/`group_by`/`entries`/`zip_exact`,
`contains`/`starts_with`/`ends_with`,
`sort`/`sort_by`/`min`/`max`/`min_by`/`max_by`,
absence `<>`, `occurs`/`absent`/`lookup`/`has_key`, lazy `default`, and
`if`/`elseif` with mandatory
`else`. Unsupported forms and known incompatible types fail before evaluation,
including in skipped branches. Skipped runtime branches perform no arithmetic
or missing-access work. Overflow, zero division, inexact promotion, nonfinite
results and nonzero floating results rounded to zero are located errors.

Use `contains(text, fragment)`, `starts_with(text, prefix)` and
`ends_with(text, suffix)` to filter projected names. Each takes two strings and
returns a fixed Boolean. Matching is exact and case-sensitive; it does not
coerce values, interpret regex, fold case or normalize Unicode. Empty searches
match. For example, `contains("Ångström", "ström")` is true,
`starts_with("Ångström", "ång")` is false, and `ends_with("Ångström", "")`
is true. Precomposed `é` does not contain decomposed `é`.

```sh
zincite query --program '
let labels = items | names;
[n | n in labels where starts_with(n, "work") /\ contains(n, "load")];
' \
  model.mzn
```

The [saved string-matching program](../crates/zincite-query/tests/fixtures/string-matching.zq)
adds a native record projection and the Unicode controls. Inline programs,
`--program-file` and `Program::parse` share the evaluator. Both operands run in
written order. Non-string operands fail with a query location, even in skipped
branches when their types are known. Scans and diagnostics spend the continuing
request budget; matching borrows the text without substring or normalization
copies. These are report helpers and remain unavailable in data edits and
preceding lets, including unused or skipped calls. Native `name(...)` selectors
keep their existing host behavior.

Array reducers take one fully evaluated positional or keyed array. `length`
counts present values and permits any element type; nested arrays remain nested.
`sum` adds integers in order with checked overflow. `forall` and `exists` scan
all fixed Boolean elements. Empty results are `0`, `0`, `true` and `false`.
A comprehension head failure occurs before reduction even after a decisive
Boolean. Sets, tuples, float sum and generator-call shorthand are refused.

`lookup(value,key)` returns absence for a compatible missing record label or
array key. `has_key` distinguishes omitted storage from a stored `<>`.
`occurs`/`absent` inspect presence; false, zero and empty containers are present.
`value default fallback` selects its fallback only for absence, with scope and
known-type checks on both branches and no skipped fallback data projection.
Fixed model declarations constrain optional source values and record labels.
Strict dot/bracket access still fails on missing data. These forms are read-only;
external inputs stay required int/string/bool and data edits refuse optional
sources, targets and values. See the [optional contract](query-comprehension-contract.md#optional-read-only-values)
and [executable report](queries.md#read-optional-fields-and-sparse-cells).

`require(condition, value, "message")` has exactly three arguments and a literal
string message. It preflights both expressions, then evaluates the fixed Boolean
condition. True evaluates and returns the value; false fails at the require call
with the literal message. False skips value arithmetic, access, construction and
runtime host checks, while referenced values may still be projected or inspected
during preflight. Undefined names and incompatible known types still fail.

`sort(xs)` and `sort_by(rows, "field")` are stable ascending positional sorts.
Their optional final `descending=true` reverses key order and keeps input ties;
`descending=false` is the default. `min`/`max` and `min_by`/`max_by` select the
first tied scalar or original record and return `<>` for empty input. Defaults
are explicit, as in `min([]) default 0`. Keys must be present homogeneous int,
finite float, exact string or one nominal type; no numeric coercion or automatic
absence order. Nominal order follows the declaration. Missing/absent/incompatible
fields point to the literal selector. These calls are report-only and share
work, scratch, output-copy and comparison limits. See the [complete contract](query-comprehension-contract.md#report-expressions-and-edit-grammar)
and [executable report](queries.md#sort-rows-and-select-extrema).

Native report pipelines accept `sort`, `min`, `max`, `min_all`, `max_all` and
`*_by(selector)` for each form. A selector runs against one incoming item and
can use the containing lexical bindings. Direct ordering accepts scalar string
or literal streams; handles need an explicit scalar projection, for example
`items | filter(name("A") or name("B")) | sort_by(expressions | tail(1) | values)`. Each selector must
produce exactly one present integer, finite float, exact string or nominal
member of one coherent type. Counts use checked signed conversion. All keys
are checked, including keys that cannot win an extremum. Sorting keeps ties;
single extrema keep the first tie and all-ties forms retain tied items in their
incoming order. Empty native extrema retain the empty selection's kind and
owner. Constructed extrema retain the optional contract above.

`left union right`, `left intersect right` and `left except right` combine native
selections using their existing identities. Operators have equal precedence and
associate left to right. Both operands read the same incoming input and execute
left then right. Union keeps first appearance in the left operand, then the
right. Intersection and difference deduplicate in left order. Nodes, semantic
entities/type/instantiation rows and strings have identities; literal streams,
counts, dictionaries, captures and computational identity sets do not support
this algebra. A scalar Node or Declaration contributes one row; set results
are selections, including empty results. Both operands are checked completely,
including discarded rows.
Use `(left union right) | names` to continue from a combined selection.

`selection | map_by(key_query, value_query)` constructs an insertion-ordered
string dictionary. The key must produce exactly one string. Each selector sees
the same singleton and lexical scope; a duplicate fails before its value runs.
Payloads can retain any admitted pure native or constructed value.
`dictionary | sort_by(value)` uses the typed scalar key contract above and
keeps current-order ties. `reverse`, `head(n)` and `tail(n)` retain dictionary
kind for unsorted, sorted, empty, zero and oversized selections. Native `tally`
counts support these operations directly. JSON object member order agrees with
library iteration, including nested dictionaries. See the
[dictionary report](queries.md#rank-a-native-dictionary).

These operations are read-only program forms. Known unsupported kinds and
impure selectors fail preflight even for empty inputs or unused branches.
Typed programs use `ProgramNative::Query` leaves, `ProgramNative::Ordering`,
`ProgramNative::Mapping`, `ProgramNative::MapSorting`, `ProgramNative::Exists`,
`ProgramNative::FixedPoint` and `NativeSetOperator`
in `ProgramValue`; they lower to the same execution.
Native owners, source coordinates and analysis limitations survive ordering
and set operations, and every selector shares the request budget.

Constructed positional arrays also accept `| filter(word => starts_with(word,"a"))`,
`| count`, `| sort`, `| unique` and terminal `| tally`. The same filter can use `filter(starts_with(_,"a"))` or the compact
`filter(starts_with("a"))`. General predicates require a fixed Boolean and may
use lexical captures and permitted pure calls. Their subject is one element; `count` returns the number of present
array entries. `sort` uses the scalar ordering above; `unique` keeps first
occurrences; string-only `tally` produces a count dictionary in first-seen key order.
Use grouping for saved values or calls, such as `(rows) | sort | unique`.
MiniZinc comprehension pipes stay inside brackets. Keyed inputs need an explicit
positional conversion. These suffixes are report-only, including unused and
skipped data-edit expressions. See the [pipeline contract](query-comprehension-contract.md#constructed-pipelines)
and [saved-selection report](queries.md#continue-a-calculated-array).

`group_by(rows, "field")` groups positional records by a static string selector.
Keys are homogeneous integers, strings or resolved members of one enum. The
positional result has `key` and `items` fields: groups follow first occurrence,
and items retain input order and duplicates. Empty input returns an empty array.
Missing fields and incompatible keys fail at the selector. Keyed arrays require
an explicit positional conversion. Grouping is report-only, including preceding
unused lets and skipped edit branches. See [machine groups](queries.md#group-machine-demands)
for a complete input, query and result.

`entries(a)` takes one keyed array and returns positional records with `key`
followed by `value` in stored entry order. Rank-one keys are integer or enum
scalars; larger keys are tuples. `zip_exact(a,b)` takes two keyed arrays with
identical ordered axes and present full keys. It returns a keyed array with
left/right tuple cells and retains left axes and keys, including equal empty
axes. Ordinary positional arrays are refused. Counts alone do not establish
alignment; different axis order, types or keys cause a located error. The
helpers perform no filling, reindexing or data writes. Both are report-only,
including preceding unused lets and skipped edit branches. See
[matrix comparison](queries.md#compare-keyed-matrix-cells) for short inline and
file reports with an explicit per-cell delta.

Native string, count and literal projections can supply calculated values.
`items | names` provides a positional array of strings, `items | count` a count,
and `items | values` a positional array of literal RHS values. Add `| one` to
extract exactly one value from a projection. Native node and semantic selections
retain their identities; use an explicit projection before passing them to a
scalar operation or comprehension.

The development dependency pins Zirql `90d184e9376a5ab039eecba0e62f3f1afd778a9b`.
Zirql traverses mixed native/constructed containers, converts constructed nodes
with its tagged-data codec, and writes JSON with the active request budget.
Zincite supplies native source/semantic descriptors, shape admission and the
completion envelope. Native sequences stay ordinary arrays even when a child
is tagged constructed data. See [mixed report output](queries.md#render-a-mixed-native-report).

A `let` binding can retain a native result or hold calculated data. Constructing
a dictionary in a binding, as in `let r = (count: n); r`, produces an ordered
JSON object with tagged constructed fields. Native dictionary payloads retain
their ordinary native representation. Pure reports use the
[constructed-value schema](query-comprehension-contract.md#constructed-shape-and-result),
including explicit `1..n` axes and keys for positional arrays. Generated values
have no input-source ranges.

Keyed reports retain integer/member axes and multidimensional access uses
`a[i,j]`; `a[i][j]` retains nested-array access. Keys follow numeric or explicit
enum declaration order, with the rightmost axis varying fastest. Read-only
reports permit noncontiguous axes and omitted cells; stored keys stay unchanged.
Data edits require contiguous axes and full rectangular coverage. Invalid keys
fail without shifting keys. Known empty axes retain their rank and the other axes;
uncertain keys cannot establish an empty fallback. Declared source axes may
resolve bounded integer endpoints from unique original scalar literals. `enum_member` takes
an enum identifier and a member-name string, and requires unique explicit model
cases. Empty keyed comprehensions can infer axes only from supported simple key
binders; a head key such as `e.key` after `entries` cannot recover empty axes.
Direct `zip_exact` retains established empty axes. Pure `.dzn` reports retain an explicitly supplied `--model` without using
the semantic query's `.mzn` root path.

Model calls remain unsupported. Constructed data batches use `create` and
`replace`. Zincite also supports native `append` and bounded
`scale_enum` edits, described under [source-copy edits](#source-copy-edits).
All data batches end with one `emit;` (`emit_document;` remains an explicit
document-emission spelling).
Input bindings and calculations may interleave before the edits. Every binding
reads the original input. Native data reads use the bounded selection, filtering
and value-projection stages in the [data contract](query-comprehension-contract.md#report-expressions-and-edit-grammar), including shared `one`. Native
report composition remains excluded.
Constructed data expression calls are exactly `index_set` and `enum_member`. Report-only
calls are refused in unused preceding lets,
skipped branches and edit RHSs. A batch cannot include report/query emissions or
repeat a target name. Creation requires an absent assignment; replacement requires
exactly one original assignment. Both require a unique fixed nonoptional model
declaration. Final validation overlays edited integer scalars on original literals
to check every target's type and axes, plus known unchanged assignments affected
by those scalars. Written integer scalar intervals and sets are checked too,
including elements inside records and arrays. Unsupported domain forms,
unknown or computed dependencies refuse the batch. Declared
targets can establish empty axes; they cannot repair nonempty keys or holes.
See the [data-edit guide](data-edits.md#construct-new-data) and the
[explicit doubled instance](query-comprehension-contract.md#a-complete-doubled-instance).
The single-pipeline `Query` API also supports the assignment edits described
below. Add the 0.2.0 library to a Rust project with `cargo add zincite-query@0.2.0`.
The [Rust API documentation](https://docs.rs/zincite-query/0.2.0) describes its
native result types.

#### Brace collections in reports

`{3, 1, 3}` constructs an integer set in first-insertion order, removing
repeated values. `{k: value}` constructs a dictionary using the constructed value
of `k`, including an array, tuple, set or dictionary; quoted keys remain literal
strings. Repeated `for name in source`
clauses and optional `if condition` filters use the shared evaluator:

```zirql
{k: {n mod 3 for n in 1..5}
 for w in ["pear", "apple"]
 for k in [w, w ++ "!"] if ends_with(k, "!")}
```

The keys are `pear!`, then `apple!`; both sets keep `1, 2, 0`. Each dictionary
key runs and is checked for a collision before its value runs. Arrays and tuples
compare by position, sets by membership, and dictionaries by associations,
independently of insertion order. Different value kinds remain distinct keys.
Strict `.field` selects a string field; `[key]` accepts general keys and fails
when missing. `lookup` is optional. Declared record schemas still constrain labels.
General dictionaries render as tagged `dictionary` entries with explicit keys
and values; string dictionaries retain ordinary JSON object output.

Empty `{}` is a dictionary. `set<int>()`, `set<bool>()`, `set<float>()` and
`set<string>()` construct typed empty scalar sets. `set<value>()` constructs an
empty general set; report sets can contain mixed and nested constructed values.
Natural MiniZinc set/comprehension/record forms remain supported. General dictionaries and string, mixed or nested sets
are refused by the data-edit boundary. These report forms do not expand native
node/declaration containers, model evaluation or `.dzn` serialization. See the
[dictionary example](queries.md#build-a-dictionary-from-string-keys).

#### Program invocation and Rust results

Use `zincite-query --program TEXT [FILE|-]` or `zincite query --program TEXT
[FILE|-]` for inline programs. `-f PROGRAM` and `--program-file PROGRAM` read a
file through the same Program API. Select one source: inline text, a program file
or a positional query. Repeated or mixed program sources fail. Program
mode accepts at most one input file or `-` and has no positional query slot.
Input and model options follow the [command reference](cli.md). Report programs
reject `--diff` and `--write`; editing programs use the existing preview and
complete-candidate write rules. Both commands evaluate and render all requested
outputs before writing stdout. A later runtime or rendering error leaves stdout
empty. Inline program errors use `<program>`; file errors name the program file.
Both retain original program byte ranges. Source errors retain
the original input or model location. Incomplete semantic reports still carry
`complete:false` and located `limitations`, and exit 1.

For `demand = [2,3,4];`, both commands return the tagged integer `9`:

```sh
zincite-query --program 'let d = items | filter(name("demand")) | values | one; sum(d)' data.dzn
zincite query --program 'let d = items | filter(name("demand")) | values | one; sum(d)' data.dzn
```

Saving that exact source as `sum.zq` and using `--program-file sum.zq` produces
the same result as `Program::parse` followed by evaluation and rendering. No
parameter or named entry point is needed. The [capacity report](queries.md#compare-demand-and-capacity)
shows a longer program with local `factor = 2`.

The library provides `evaluate`, `evaluate_with_model` and `evaluate_with_facts`.
`Program::requires_model_facts()` reports semantic or possible source/member demand.
`ProgramResult::model_facts()` retains the owner for native nominal identities,
including pure `.dzn` reports. Each evaluation prepares
one initial selection and retains one context, `Rc<ModelFacts>` and budget across
statements. `Program::is_editing()` identifies a data batch or scalar edit pipeline, and
`ProgramResult::candidate()` exposes its reparsed complete data document.
`ProgramResult::emissions()` exposes report values; `semantic()` retains
the public fact owner and `is_complete()`/`limitations()` retain uncertainty.
The legacy `QueryResult::render(&self) -> Vec<u8>` stays infallible and has no
retained request budget. It keeps its native-only compatibility rendering.
Program rendering uses the shared writer and spends its active request allowance;
it does not open a fresh rendering budget.

Call `render(&mut self)` to obtain one output buffer, then write those bytes only
after success. Every JSON emission uses `{complete, limitations, result}`, including
syntax-only reports. Repeated rendering spends the same remaining allowance again.

#### Program limits

`ProgramLimits` bounds the work and memory used by a query program. The CLI
exposes the same settings:

| CLI option | Default | What it limits |
| --- | --- | --- |
| `--max-work` | 1,000,000 | Evaluation work, including traversal, copying and rendering. |
| `--max-results` | 100,000 | Aggregate result units across statements, nested values and output. |
| `--max-items` | 100,000 | Items in any one stream. |
| `--max-nesting` | 64 | Nesting of expressions and values. |

Exceeding a limit fails the whole request. Zincite does not return a truncated
stream. These are operation counts, not byte or elapsed-time limits.

A small final report can still require substantial work. Generator candidates
count before filtering, and reducers such as `sum`, `forall` and `exists`
count the elements they scan. Constructing, copying and serializing a value
also consume the budget. Nested containers and fields count toward the
aggregate result limit; emission snapshots and rendering count separately.

Parsing, expression validation and free-name discovery share an allowance
across statements. Evaluation counts that preparation work again for each
request. For data edits, type-aware expression processing, candidate assembly,
reparsing and source rendering use the same request budget. Grouping counts
key comparisons, type-identity checks, containers and copied keys or labels.
`require` counts message bytes before preparing a failure diagnostic; a
successful `require` returns its value without an extra copy.

Work includes inspected model tokens and CST nodes when a query needs model
facts, as well as copied source text, strings and encoded output. Reusing
cached facts does not bypass the new request's limits. Model tokens and CST
nodes do not count as result units.

The parser checks expression nesting and operator chains before building the
CST. Separate sibling values and conditional branches have separate depth
counts. JSON construction also reserves bounded space for metadata before
allocating it.

### Native existence and fixed points

`where_exists(relation)` evaluates a pure native relation against each singleton
subject. It returns the original subjects with nonempty complete relations, in
incoming order with duplicates retained. Relations can inspect another admitted
native kind and read the surrounding lexical scope. A singular node or
declaration becomes its corresponding possibly empty selection.

`fixpoint(body)` uses replacement, exact native kind and owner, and ordered,
duplicate-sensitive identity equality. Use `input union successors` inside the
body to accumulate. Stable empty selections succeed. Repeated non-fixed
selections fail under the bounded checkpoint policy. Both operators share the
active work, collection and nesting budgets with their bodies.

Only existing identity-bearing native selections are admitted. Counts,
dictionaries, constructed values, literal streams and computational
`identity_set` values do not gain native identity. Bodies preflight kinds,
lexical names and purity before empty-input shortcuts. They cannot emit, edit
or make report-only operations legal in data expressions.

A complete empty relation establishes absence. Uncertain adjacency and final
unknown predicates refuse at the responsible model location, even after the
same limitation was recorded or the partial result was saved. Ordinary
best-effort reports keep their output and limitations. A transparent
`ReportValue::IncompleteSelection` retains one responsible error through native
copies, aliases, ordering and set results, including discarded or empty results;
strict nested observations refuse that value.

The [walkthrough](queries.md#filter-connections-and-accumulate-declarations)
shows both forms. From the [v0.2.0 checkout](getting-started.md#build-from-a-checkout)
root, the saved program also runs as:

```sh
zincite-query --program-file crates/zincite-query/tests/fixtures/native-fixed-points.zq crates/zincite-query/tests/fixtures/worklist-reachability.mzn
```

`Program::from_values` uses `ProgramNative::Exists` and
`ProgramNative::FixedPoint` with retained native expression bodies. Typed,
text, ordinary and prepared function paths use the same native algorithms.

### Assignment edits

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
`values` accepts `head`, `tail`, `reverse`, `check`, `count`, `json` and `emit`;
implicit/explicit source
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

For example, save these keyed records as `records.dzn`:

```minizinc title="records.dzn"
jobs = [A: (duration: 2, cost: 5), B: (duration: 3, cost: 4)];
```

Inspect the literal, record field names or array keys:

```sh
zincite query 'values | json' records.dzn
zincite query 'elements | fields | names | json' records.dzn
zincite query 'keys | names | json' records.dzn
```

To edit literal collections, see the [data-edit examples](data-edits.md).
`filter_elements(value => value > 0)` and `filter_elements(_ > 0)` evaluate a
shared scalar predicate for each direct element. Bodies follow the closed Data
expression policy, including every unused binding and unchosen branch. They
cannot call report helpers or declared functions. Preparation checks the body
before enumeration, including empty selections. The scalar values and errors
retain their input ranges; retained cells, keys and comments follow the same
edit checks as the direct comparisons below.

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

#### Source-copy edits

Since version 0.1.2, Zincite supports explicit source-copy and enum-scaling edits
in a data program ending with `emit;`. All require `--model` (or retained model
facts in Rust). The copy stages take one original assignment selected through
`values`, without `one`:

| Stage | Native result |
| --- | --- |
| `clone_members(additional_count, prefix)` | Ordered fresh-member maps for an uninitialized enum's literal member set |
| `clone_elements(copies)` | Literal collection interiors with typed member/key substitutions |
| `repeat_elements(additional_count)` | Literal collection interiors with original tokens |

Counts are nonnegative integer expressions. The prefix is a string expression
whose contents start with an ASCII letter and contain only ASCII letters, digits
and underscores. Current parameters and earlier immutable bindings are available. Names are `prefix_copy_position`, with both
numbers starting at one. Copies are appended in copy order and original written
order. Zero copies preserve the input bytes exactly. Generated names must not
collide with input identifiers, model/include declarations or another appended
member. `clone_members` requires a nonempty set of distinct explicit members.
`clone_elements` takes a saved member-copy binding; repetition performs no
remapping or explicit-key reindexing.

| Statement | Effect |
| --- | --- |
| `append Target = native_expression;` | Append native member or element copies to one original collection assignment. |
| `scale_enum Target = factor;` | Multiply the literal positive bound in one original `C(1..N)` enum assignment. |

`append` retains the target's original RHS bytes and inserts copies before its
closing delimiter. Member maps can append only to their own enum. Other copies
support literal sets and rank-one positional or explicitly keyed arrays, with
recursive records, tuples, sets and rank-one arrays. Final validation checks
edited and unchanged affected data against retained model types and final enum
membership and array coverage. Enum-valued elements require a full enum domain,
directly or through a type alias; enum subset domains refuse, including in nested
collections and records. Starting-key arrays, matrices, computed data, optional
types, unavailable domains, duplicate keys and copied directives refuse.

`scale_enum` requires a unique uninitialized model enum and the exact bounded
constructor shape `C(1..N)`, with a named constructor and a positive integer
literal `N`. Its factor uses the shared scalar evaluator and must be a positive
integer. Checked multiplication produces the new bound; only that token changes.
Factor 1 preserves the original bytes. Computed bounds, other constructor forms
and constructor-member references outside the defining assignment refuse with
locations, including explicit `C(1)` array keys and member values. Positional
arrays use the final enum cardinality for coverage. Leaving an affected array
unchanged does not exempt it from that check.

Copy values stay native and retain their input/model owner. They can be bound and
reused, but cannot become ordinary constructed values or report output. Each edit
target must be unique across all edit kinds. Constructed edits whose types depend
on a changed enum refuse; unrelated supported create/replace edits may coexist.
All reads use the original input. Model-derived expressions and decision
declarations do not become data-validation obligations. Candidate assembly,
validation and reparsing finish before any output or write. An error leaves
stdout empty and the input unchanged. Normal [program limits](#program-limits)
and [file-write checks](cli.md) apply. See the
[source-copy contract](query-comprehension-contract.md#copy-existing-enum-backed-data)
and the complete [wedding growth query](data-edits.md#grow-a-wedding-instance).

### Enum reduction

For an enum assignment such as `Guests = {A, B, C}`, `keep("A", "C")`
selects members by name; `keep_first(2)` selects the first two in written order.
These commands use `guests.dzn` from the
[enum reduction walkthrough](data-edits.md#reduce-enum-membership):

```sh
zincite query 'reduce_enum("Guests", keep("A", "C"))' guests.dzn
zincite-query --diff 'reduce_enum("Guests", keep_first(2))' guests.dzn
```

`reduce_enum` requires a top-level item selection in data mode and inspects the
whole document, even after `filter` or `head`. The target must be one assignment
whose value is an explicit set of unique member identifiers. Parentheses and
quoted spelling survive. `keep` chooses identities while preserving original
member order; `keep_first` uses that order. Unknown requested members and
constructed or anonymous enums are located errors. `keep()` and `keep_first(0)`
allow empty retention.

Explicit member keys establish enum identity without a model. Supply
`--model model.mzn` when declarations are needed to associate positional data
with the enum. With model facts, unique top-level declarations associate data
assignments with enum identities, expanded types and written index domains.
Positional slicing requires the full
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
resolves directly to the target enum. Nested iteration such as
`forall(group in groups)(forall(g in group)(scores[g] >= 0))` also preserves
membership when `groups` resolves to a uniquely associated positional data array
of literal guest sets that the reducer prunes. Generator bindings retain their
lexical identity; a shadowed name does not inherit this proof. Computed group
sources, numeric conversions such as `to_enum` and read-only model group accesses
that need an index repair remain unresolved.
Strings and record labels are never rewritten as enum members. This bounded check
does not establish satisfiability or full
compiler type validity.

### Semantic inspection

Semantic stages follow declaration identities and report available type and
instantiation facts. They do not run lint rules. An explicit model input file is
its own root by default; `--model` supplies another root. The query input must be
in model mode and match that root's original bytes exactly, including the BOM
and opaque comments.
Stdin needs explicit `--model`; a matching `--stdin-filepath` label alone does
not load a model. The library requires a `ModelContext` with a retained root.
Syntax pipelines ignore an unused supplied model, including a missing path.
Data reduction uses its separate rules for optional model loading.

These stages use the public `load_model`, `resolve_bindings`,
`resolve_source_callables` and `resolve_instantiations` APIs. Explicit navigation
into a standard callable body requests its source occurrence types through
`resolve_source_callable_body`. It does not recursively inspect implementations
to prove value properties. The public `resolve_callables` API also analyses
bodies, as described in the [lint library](#lint-library).

`references` selects each written reference fully contained in the source region
of a selected node or declaration. A selected reference covers its exact token.
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
source region, including types, signatures, defaults, initializers and
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
`filter`, `head`, `tail`, `reverse`, `check`, `count`, `names`, `text`, `unique`,
`emit` and `json`; projected
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

Fact families are computed on demand and reused by `ModelFacts` for one immutable
`ModelContext`. `evaluate_with_model` creates an owner for its evaluation;
`evaluate_with_facts` accepts an `Rc<ModelFacts>` that lint and query requests can
share. Changed source or include configuration requires a new context and owner.
Source typing stays separate from selected bound, definedness and output proofs.
Explicit `resolve_callables` and `ModelFacts::proof_callables` retain bulk
body-aware compatibility behavior; ordinary queries do not request that closure.
The producers scan a whole retained context; before calling them, the query
bounds its token and CST-node scan by work and collection limits. Reference scans, target lookups,
expansion, row additions, type components and output bytes are charged too.
Each evaluation charges these limits even with a warm owner and checks the exact
root bytes, including the BOM. Nested type inspection respects `Limits::nesting`.
Exceeded limits are actual errors and never silent truncation.

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
both `Input` and `ModelContext` for the result lifetime. `evaluate_with_facts(input,
Rc<ModelFacts>, limits)` retains a shared owner in the result; the last owner or
result releases its cached facts. `SemanticResult` exposes
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

The adapter translates Zirql's `WorkLimit` and `ItemLimit` categories into the
existing query work and collection diagnostics. Other core failures keep their
full message and query/input range. A final `UnknownPredicate` from a native
match guard or path target uses the responsible model diagnostic when available;
otherwise it retains the shared query error. Native host errors keep their file,
range and message. These translations depend on categories, independently of
human-readable wording.

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

Expanded `forall` bodies put top-level conjunction clauses on separate lines.
Short conjunctions nested in calls or parentheses remain compact. When a multiline
Boolean expression combines unparenthesized precedence levels, the weaker
operator occupies its own line and its operands are indented. For example:

```minizinc
constraint
    a /\ b
<->
    forall (i in I) (
        p(i)
    );
```

The left operand of `<->` is the whole `a /\ b` conjunction. Short expressions
stay inline, and chains at the same precedence share an indentation level.
Formatting preserves the expression tree, parentheses and operator spellings.

## Lint library

Use `zincite_lint::lint(&parsed)` for the source-local default rules. It returns
warnings with rule, severity, message and byte range, or owned syntax/suppression
diagnostics without partial warnings. `LintOptions::from_selection` resolves
presets and explicit IDs; `lint_with_options(&parsed, &options)` runs a selected
set. `Rule::DEFAULT` and `Rule::THESIS` expose preset membership. The default
selects `naming`, `missing-constraint-label`, `prefer-array-comprehension`,
`index-set-mismatch`, `suspicious-shadowing` and `partial-expression`, in that
order. Omitted selection, `default` and `preset:default` are equivalent. Other
rules stay opt-in; `thesis` selects fourteen rules and `all` selects all 28
implemented rules. The `lint` convenience function runs only the source-local
subset of the default. Semantic selections require the explicit model API below;
`lint_with_options` rejects them without that context.

Explicit model analysis uses `load_model(root_path, &ModelOptions)` and
`analyze_model(&context, &LintOptions)`. For repeated or mixed requests, construct
`ModelFacts::new(&context)` and call `analyze_model_with_facts(&facts, &LintOptions)`.
`ModelFacts` reuses requested facts, while each call applies its own rule
selection, options and suppressions. It borrows one immutable model root and its
included files. A source signature or type alone does not establish a bound or
output definition; rules needing those proofs request them separately. The
context stores parsed sources, canonical file identities, include locations and
file-local suppressions. It resolves includes from the including file's directory, then the ordered
`include_dirs`, then `stdlib_dir/std`. A configured library also loads
`std/stdlib.mzn` and its core closure as implicitly available declarations.
Standard-library source identity remains separate from explicit input selection;
system sources receive style warnings only when explicitly selected. Consumers
use the retained CST to identify declarations rather than inferring builtins
from names or bodyless user declarations.

`ModelOptions` takes its configuration from the caller. The CLI accepts ordered
`-I DIR` options and `--stdlib-dir DIR`, with `MZN_STDLIB_DIR` as the fallback for
the latter. The default loads the root and include closure for comprehension
advice, index-set checks, lexical shadowing and partial expressions. Supply
matching include and standard-library paths when needed. Selecting only
`naming,missing-constraint-label` keeps analysis source-local. Default analysis
retains explicit limitations for unsupported required semantics; a quiet result
does not establish totality or index-set agreement. Ordinary analysis writes
nothing; fixes require an explicit request.
Implemented rule availability is separate from a particular analysis's
`Completed`, `Limited` or `Inapplicable` outcome.

Selected analysis separates file-aware findings, per-rule execution outcomes,
model limitations and actual errors. Dynamic include paths produce an explicit
incomplete-context limitation; missing or cyclic dependencies, invalid UTF-8,
syntax and suppression failures produce errors. `write_analysis` supplies the
library's full detailed renderer. Both lint command forms add `[root PATH]` to
located model diagnostics. By default they report each root's limitation count;
`--show-limitations` also prints each limitation's location and reason. This
changes presentation only, not the library result.
Informational findings (`info`) alone keep status 0; unsuppressed warnings
produce 1, and errors take precedence with 2. Symbolic Cartesian structure
advice is informational; proved threshold exceedance remains a warning.
Limitations alone keep status 0 or 1. Unsupported required semantics remain
explicit limitations; analysis does not guess missing parameter values.

`Rule::PreferEnum` is opt-in style advice with `FixSupport::None`. It consumes
bindings, source callable signatures and raw domains, without demanding
instantiations, definitions or selected standard-body proofs. Mixed selections
keep its source facts separate from the body-aware proof route required by other
rules. The analysis distinguishes candidate sets, their members and their use as
array indices; its findings carry no fixes. See
[integer sets and enums](linting.md#integer-identity-sets-and-enums).
A rule's fix support describes whether it can provide automatic edits. Each
proposed edit still has its own safety classification; the rule's severity and
advisory wording do not determine that safety.
