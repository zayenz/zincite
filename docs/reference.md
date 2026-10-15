# Syntax and library reference

Zincite targets MiniZinc 2.10.1 model (`.mzn`) and data (`.dzn`) files.
For installation and command usage, see the [README](../README.md).

For command workflows, start with [installation](getting-started.md),
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
Comments may occur between tokens. Formatting preserves comment, literal and
operator spelling and explicit parentheses. It puts a labelled constraint's
expression on the following line and retains explicitly multiline comma-separated
lists with one entry per line and a trailing comma. A multiline nested entry does
not expand its enclosing list. Generator calls use spaces before their header and
body parentheses. `forall` bodies use block indentation, including quoted
`'forall'` calls. A direct `if` or `elseif` condition with a single short body can
stay inline when the complete condition fits; comments and multiline expressions
retain their layout. Other short bodies stay compact; explicitly expanded bodies stay
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
library and both commands use the same pipeline stages:

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
| `emit` | Render selected original source bytes or the complete reparsed edit candidate |
| `set_value("EXPRESSION")` | Replace selected assignment right-hand sides in data mode |
| `filter_elements(comparison)` | Explicitly filter literal set, array or matrix elements in data mode |
| `remove` | Remove selected data assignments and their attached comments/directives |
| `reduce_enum("Guests", keep("A", "B"))` | Keep explicit enum members and reduce supported dependent arrays, sets and group accesses |

Stages are separated by `|`. The initial selection is `items`, including for an
empty query. Source or projected-string emission is implicit. `emit` and `json` are terminal.
Only `json` may follow `count` or `tally`. A pipeline may contain one transformation stage,
followed only by its optional `emit`; document emission is implicit for
edits. `emit_document` remains a compatibility alias requiring a transformation.
`head` does not avoid parsing or evaluating the earlier selection. No input items
produces an empty selection or count 0; the untouched selection of a comment-only
document still emits its original bytes.

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

### Required query parameters

Query programs declare required scalar inputs with `param name: int;`,
`param name: string;` or `param name: bool;`. Pass each value with a repeated
`--param name=value` option in either command:

```sh
zincite-query --param factor=2 --program 'param factor: int; source d = literal("demand"); report sum([n * factor | n in d]);' data.dzn
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

Program query sources retain their original strings and source coordinates.
Checked primitive arguments such as `name(selected)`, `head(limit + 1)`,
`range(start, end)` and `transitive_references(depth)` use the existing MiniZinc
expression subset and shared evaluator. String `kind`, `type`, `instantiation`
and `source_kind` predicates also accept checked expressions. Native accepted
names, nonnegative indices and range order are still checked. Known wrong
types fail before a query executes, including empty or unchosen predicates.
`source data = literal(field);` accepts a checked string assignment name.
Literal pipeline arguments and short positional queries keep their behavior;
legacy positional queries have no parameter declarations and reject inputs.

Rust `Program::parameters()` exposes declaration metadata.
`evaluate_with_parameters`, `evaluate_with_model_and_parameters` and
`evaluate_with_facts_and_parameters` accept `ParameterInputs::Typed` scalar
pairs or `ParameterInputs::Cli` raw strings alongside `ProgramLimits`:

```rust
use zincite_query::{ConstructedValue, ParameterInputs, Program, ProgramLimits};

let limits = ProgramLimits::default();
let program = Program::parse("param factor:int; report factor;", limits)?;
let result = program.evaluate_with_parameters(
    &input,
    ParameterInputs::Typed(&[("factor".to_owned(), ConstructedValue::Integer(2))]),
    limits,
)?;
```

Existing evaluation methods supply an empty input set. Input conversion and
admission share the request work and aggregate result allowance with evaluation
and rendering. Data edits keep their closed type and call policy, complete
candidate validation and atomic write boundary. Any admission or later failure
leaves CLI stdout empty and the input file unchanged.

### Query programs

`Program::parse(source, ProgramLimits)` accepts ordered reports or one atomic
data edit batch.
One standalone native expression can end at EOF without `;`. Bindings, `report`
statements, multiple statements and data edits keep their required terminators.
A source binding is immutable and can refer only
to earlier bindings. `query("pipeline")` uses the stages above; its string is
parsed once and diagnostics retain the original program byte coordinates.
A bare expression emits its result. `report expression;` emits JSON.

```text
source selected = query("declarations");
source n = selected | query("count");
source histogram = selected | query("names | tally");
report (count: n, histogram: histogram, selected: selected);
```

Record fields use identifiers or nonempty single-quoted labels and preserve written order in the native
`ReportValue::Record`. Duplicate labels fail. Parentheses also group expressions.
A binding can hold a native selection, count, histogram or nested record. Counts
render as integers; histograms render as string-to-count objects. Selections
retain their original nodes or declaration/reference identities, file owners and
ranges. A constructed record has no input-source range. These inspection leaves
are not scalar comprehension values.

`expression | query("pipeline")` runs against that expression's native result.
`| emit` and `| json` are terminal emitters. Query-source strings can also end
with their existing `emit` or `json` stage. An explicit emitter suppresses the
statement's implicit emission. Bindings and record fields cannot emit. Program
parsing checks names, emitters and supported result transitions before execution.
#### Typed query functions

Read-only programs accept named pure functions, including direct and mutual
recursion. Declarations can appear before or after their callers. Each function
has typed parameters, a result type, ordered local `let` bindings and one final
value. MiniZinc `if`/`then`/`elseif`/`else`/`endif` and comprehensions keep their
ordinary query-expression spelling. The shared `if condition { value } else
{ value }` form is also accepted.

```text
fn accumulate(n: int, total: int) -> int {
    if n = 0 then total else accumulate(n - 1, total + n) endif
}
report accumulate(1000, 0);
```

This reports the tagged integer `500500`. A compatible tail call reuses its
active frame, including a mutual tail call. A pending operation such as
`1 + accumulate(n - 1, 0)` needs another frame. Functions have a separate active
call-depth limit of 64; library callers can use
`Program::with_function_limits(zirql::FunctionLimits { active_depth: 1 })`.
The tail example still succeeds at depth one with the default work allowance.
Non-tail calls refuse at the limit. Tail calls still spend work, so a
nonterminating function eventually refuses. Diagnostics keep the original
query or model location and append bounded function-call context.

| Signature form | Accepted values |
| --- | --- |
| `bool`, `int`, `float`, `string` | Checked fixed scalars. |
| `EnumName` | A member of that actual enum in the current retained model. |
| `opt T` | Absence or an admitted `T`. |
| `sequence<T>` | A homogeneous positional sequence, including an empty sequence with its element contract. |
| `tuple<T, U>` | Fixed positions with the declared types. |
| `record{label: T, other: opt U}` | Declared labels; optional fields may be omitted. |
| `set<T>` | Supported Boolean, integer, float or nominal set elements. |
| `keyed<T>` | A keyed array with admitted cells and retained axes. |
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

The parsed program retains its immutable body trees. Each evaluation resolves
small headers and prepares the written bodies and entries once, then reads
admitted immutable arguments and locals through shared owners. Repeated calls
with the same contract avoid deep argument copies and whole native-argument
rescans. The first boundary checks every actual native element, owner and
coordinate. Fresh or changed contracts check again. This reduces allocation
charges for retained reads; materialization, fresh values, emission snapshots
and rendering retain their budget charges. Existing limits and defaults stay
the same. Reusing a parsed `Program` after failure starts a fresh evaluation
with its current input, facts and allowances.

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

Zirql supplies the only breadth-first evaluator. It returns one shortest witness
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

Source/neighbor scans, comparisons, traversal slots, native copies, witnesses
and rendering share the existing aggregate program budget. Paths remain
report-only and are excluded from data expressions and preceding edit lets.
See [the complete inline, file and library example](queries.md#explain-declaration-connections).

#### Native projections and captures

Read-only programs also accept domain stages directly:

```text
report subtree | declarations | unique | project(d, {
  "name": d | names | one,
  "uses": d | uses | count
});
let matches = match {
  from d in (subtree | declarations | unique);
  from u in (d | uses);
};
report matches | project(m, {"name": m.d | names | one, "text": m.u | text | one});
```

`project` returns a positional array of constructed records in native source
order. Field labels are static quoted strings. The explicit subject is local
to the projection and cannot shadow a visible binding. Native handles need
`names`, `text`, `count` or another supported scalar projection before entering
a constructed field; `one` extracts exactly one projected value. Existing
`query("...")` wrappers remain valid native leaves, and ordinary MiniZinc
records and their field access keep their existing syntax.

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

Programs accept `source name = literal("assignment");`, immutable `let` bindings
and pure positional or keyed reports. For input `zeta = [3, 5]; alpha = 2;`, this request
constructs records in source order:

```text
source labels = query("names");
source numbers = literal("zeta");
let rows = [(name: labels[i], doubled: numbers[i] * 2)
  | i in index_set(numbers)];
report rows;
```

Zirql evaluates scalar expressions, scopes and generators after Zincite lowers
its existing MiniZinc CST. Later generators see earlier bindings. Multiple
binders share one source, with the last binder varying fastest; `x = expression`
binds one value per preceding environment. `_` discards its binding. Filters
require fixed Booleans and charge each candidate before filtering.

Pure values include signed 64-bit integers, finite binary64 floats, Booleans,
strings, resolved enum members, positional/keyed arrays, matrices, tuples,
ordered records and homogeneous Boolean/integer/float/nominal sets. Supported operations include checked arithmetic, exact
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
zincite query --program 'source labels = query("items | names"); report [n | n in labels where starts_with(n, "work") /\ contains(n, "load")];' model.mzn
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

`group_by(rows, "field")` groups positional records by a static string selector.
Keys are homogeneous integers, strings or resolved members of one enum. The
positional result has `key` and `items` fields: groups follow first occurrence,
and items retain input order and duplicates. Empty input returns an empty array.
Missing fields and incompatible keys fail at the selector. Keyed arrays require
an explicit positional conversion. Grouping is report-only, including preceding
unused lets and skipped edit branches. See [machine groups](queries.md#group-machine-demands)
for short inline, file and native library examples.

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

`query` sources project `Strings`, `Count` and `Literals` into pure values.
Selections need an explicit `names`, `count` or `values` query projection.
`literal` requires one original assignment and a supported written RHS; computed
values and unresolved members fail at their original input ranges. Native
report expressions retain their existing result shape. `let` always constructs
pure data: `let r = (count: n); report r;` produces a tagged record. A bare native
binding or a record containing only native leaves uses the complete native
grammar and its existing validation. Pure reports use the tagged
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

Model calls remain unsupported. Data batches use `create` and `replace`, followed
by one `emit;` (`emit_document;` remains a compatibility alias). Sources and
immutable `let` bindings precede the edits and read the original input. Their
expression calls are exactly `index_set` and `enum_member`. Report-only calls are refused in unused preceding lets,
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
The existing single `Query` API retains its edit routes. Development
uses a pinned Git revision of Zirql. The published alpha.5 release lacks the
runtime host-validation API, so this integration has no registry version
fallback until release work supplies one. Consumer release packaging also needs
sibling dependency version metadata and registry readiness; it belongs to a
release milestone.

Use `zincite-query --program TEXT [FILE|-]` or `zincite query --program TEXT
[FILE|-]` for inline programs. `-f PROGRAM` and `--program-file PROGRAM` read a
file through the same Program API. Select one source: inline text, a program file
or a legacy positional query. Repeated or mixed program sources fail. Program
mode accepts at most one input file or `-` and has no positional query slot.
Input, stdin and model options have their existing meanings. Report programs
reject `--diff` and `--write`; editing programs use the existing preview and
complete-candidate write rules. Both commands evaluate and render all requested
outputs before writing stdout. A later runtime or rendering error leaves stdout
empty. Inline program errors use `<program>`; file errors name the program file.
Both retain original program byte ranges. Source errors retain
the original input or model location. Incomplete semantic reports still carry
`complete:false` and located `limitations`, and exit 1.

For `demand = [2,3,4];`, both commands return the tagged integer `9`:

```sh
zincite-query --program 'source d = literal("demand"); report sum(d);' data.dzn
zincite query --program 'source d = literal("demand"); report sum(d);' data.dzn
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
statements. `Program::is_editing()` identifies a data batch, and
`ProgramResult::candidate()` exposes its reparsed complete data document.
`ProgramResult::emissions()` exposes report values; `semantic()` retains
the public fact owner and `is_complete()`/`limitations()` retain uncertainty.
Call `render(&mut self)` to obtain one output buffer, then write those bytes only
after success. Every JSON emission uses `{complete, limitations, result}`, including
syntax-only reports. Repeated rendering spends the same remaining allowance again.

`ProgramLimits` defaults to 1,000,000 work units, 100,000 aggregate result units,
100,000 items per stream and 64 nesting levels. CLI programs accept `--max-work`,
`--max-results`, `--max-items` and `--max-nesting`. Aggregate units cover produced
and copied native or constructed entries, generator candidates before filters,
nested containers/fields, emission snapshots and JSON storage. Pure-expression
lowering, validation and free-name discovery share the parse allowance across
statements; evaluation admits those costs before input/fact preparation, and
repeated evaluation pays them again. Data edits lower their original RHS text
again with declared target evidence during evaluation; this work shares the
request allowance. Candidate assembly, full reparsing and source rendering spend
that allowance too. Reducers retain full materialization/candidate/copy costs,
then sum/forall/exists pay one work unit per scanned element and length pays one
for its count/conversion. Each summary pays one scalar result unit, including
empty arrays. Require preflight pays literal message bytes; false diagnostic
copying pays bytes before allocation. True require moves its child without
another result or copy charge. Emission snapshots and rendering pay again.
Grouping charges field/key comparisons, nominal checks, new containers/slots
and copied keys/labels; rows move into items under the continuing budget.
Before building the CST, parsing applies
the nesting allowance conservatively to enclosed expression segments and their
operator chains; sibling values and conditional branches reset local counts.
JSON metadata reserves a bounded amount before construction. Work also covers
copied strings/source text and encoded output bytes. Raw model tokens and
CST nodes consume work when facts are first demanded; they do not consume result
units. A warm fact owner still pays the current request's admission. These bounds
refuse an entire request rather than truncate individual streams.

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
`resolve_source_callables` and `resolve_instantiations` APIs. Explicit navigation
into a standard callable body requests its source occurrence typing through
`resolve_source_callable_body`, without expanding transitive implementations for
proofs. The public `resolve_callables` API retains its body-aware behavior.
Semantic stages do not enable lint rules. An explicit model-mode input file is
its own root by default; `--model` supplies another retained root. Semantic input must be in model mode and match
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
The owner reuses requested facts; each call runs diagnostic policy for its own
selection, options and suppressions. It borrows one immutable root and include
closure. Source signatures and types establish neither bounds nor output proofs;
selected proof consumers retain their own prerequisites. The context retains
parsed sources, canonical file identities, include locations and file-local suppressions. It
resolves includes from the including file's directory, then the ordered
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
located model diagnostics. Their default presentation gives each root's exact
limitation count and incomplete-analysis/detail guidance; `--show-limitations`
renders the located limitations without changing the library result.
Informational findings (`info`) alone keep status 0; unsuppressed warnings
produce 1, and errors take precedence with 2. Symbolic Cartesian structure
advice is informational; proved threshold exceedance remains a warning.
Limitations alone keep status 0 or 1. Unsupported required semantics remain
explicit limitations; analysis does not guess missing parameter values.

`Rule::PreferEnum` is opt-in style advice with `FixSupport::None`. It consumes
bindings, source callable signatures and raw domains, without demanding
instantiations, definitions or selected standard-body proofs. Mixed selections
keep its source facts separate from the body-aware proof route required by other
rules. The private member classifier keeps nominal candidate sets, scalar members and array axes separate;
its findings never carry fixes. See [the supported identity-use and output
boundary](linting.md#integer-identity-sets-and-enums). Fix support distinguishes
available automatic edits from diagnostic-only rules; advisory wording remains
separate from correctness, family, severity and individual fix safety.
