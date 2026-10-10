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
file and explicitly edits selected data assignments. Its reusable library and
both commands use the same fixed pipeline:

| Stage | Result |
| --- | --- |
| `items` | Reset to all top-level items in written order |
| `filter(predicate)` | Keep matching items in their existing order |
| `head(n)` | Keep the first `n` items of the computed selection |
| `count` | Return a native count; render as decimal text followed by LF |
| `emit` | Render the selected original source bytes |
| `set_value("EXPRESSION")` | Replace selected assignment right-hand sides in data mode |
| `remove` | Remove selected data assignments and their attached comments/directives |
| `emit_document` | Render the complete validated edit candidate |

Stages are separated by `|`. The initial selection is `items`, including for an
empty query. Source emission is implicit. `count` and `emit` are terminal;
following stages are errors. A pipeline may contain one transformation stage,
followed only by its optional `emit_document`; document emission is implicit for
edits. `emit_document` requires a transformation. `head` does not avoid parsing
or evaluating the earlier selection. No input items produces an empty selection or count 0;
the untouched selection of a comment-only document still emits its original bytes.

Predicates are `kind("assignment")` and `name("capacity")`, combined with
parentheses and `not`, `and`, `or` in that precedence order. Query strings are
double quoted and accept `\"`, `\\`, `\n`, `\r` and `\t` escapes. Other escapes
and literal control characters are errors; Unicode characters may be written
directly. Whitespace between query tokens is ignored. Query comments, program
files, bindings and other stages are not implemented.

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

`QueryResult` keeps a `Selection`, `Count` or owned `Document(Vec<u8>)` native
until `render`. `Document` contains a complete validated edit candidate. A
selection borrows its `Input`, its item CST nodes and identifier identities. `Input` exposes
original bytes, token bytes/ranges and source locations. `SelectedItem::range`
and `source_range`, `Input::token_range` and all input diagnostics use original
byte coordinates. The analysis CST from `Input::syntax` excludes a leading BOM;
add `Input::syntax_offset` to its ranges. Opaque comment bytes appear as markers
in its UTF-8 analysis text, so use original byte accessors for source spelling.

Queries have a hard limit of 1,048,576 UTF-8 bytes and 64 nested parentheses/`not`
operators. Flat `and`/`or` chains do not consume nesting depth. `Limits::nesting`
can lower that ceiling. Default evaluation limits are 100,000 collected items
and 1,000,000 visits: each initial/reset item, stage, filtered item and visited
predicate costs one visit. Edit traversal costs one visit per inspected token;
replacement validation and each expansion charge the replacement byte length.
Boolean evaluation short circuits. Checks occur before collection additions and visits; exceeding a bound returns a located
error without truncating the result. Library callers can change work/collection
limits. Nesting is checked during query parsing; work/collection during evaluation.

The commands take one query argument and one optional file or `-`.
`--stdin-filepath PATH` selects stdin's language mode and diagnostic label;
without it stdin uses model mode. Extra files, directories and semantic options
such as `--model`, `-I` and `--stdlib-dir` are rejected. Query diagnostics identify
`<query>` line/column and query byte ranges; input diagnostics identify the input
path and original source byte ranges. Both command forms buffer a complete result
before writing stdout, exit 0 on success and 2 on query/input/usage/I/O errors.
Query and input failures leave stdout empty.

Editing prints the complete candidate by default and never writes implicitly.
`--diff` and `--write` require an editing pipeline and are mutually exclusive.
Diff accepts stdin and buffers a whole-file unified preview. Write requires the
explicit regular file, rejects stdin and symlinks, checks exact original bytes
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
