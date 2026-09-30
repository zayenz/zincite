# Zincite

Zincite is a Rust project for MiniZinc source tooling. `zincite-fmt` formats
model and data syntax using the shared `zincite-syntax` parser and concrete
syntax tree. The tree retains source spelling, comments, whitespace and byte
ranges. `zincite-lint` checks declared naming roles and advises on missing
constraint labels using the same tree.

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
type child, direct binding name and optional default expression; name tokens and
parameter nodes retain precise byte ranges. Generic variables have a distinct
`TypeInstVariable` kind and retain their qualifiers and written `$T`/`$$Index`
spelling. These views borrow the existing tree.
Interpolated strings expose their embedded expressions as ordered child nodes,
with literal chunks retained as token leaves in the same tree.
Include nodes expose their path literal as a child. Output nodes expose an optional
`Annotation` child followed by the output expression. `Solve` identifies a satisfy
item; `SolveMinimize` and `SolveMaximize` expose direct solve annotations followed
by the objective expression. Each node retains its precise source range.

## Formatter usage

Install the command from this checkout:

```sh
cargo install --path crates/zincite-fmt
zincite-fmt model.mzn
zincite-fmt < model.mzn
zincite-fmt - < model.mzn
zincite-fmt data.dzn
zincite-fmt --stdin-filepath data.dzn < data.dzn
zincite-fmt --check model.mzn data.dzn
zincite-fmt --check --stdin-filepath model.mzn < model.mzn
zincite-fmt --write model.mzn data.dzn
zincite-fmt --help
```

The command defaults to formatting one UTF-8 file or stdin to stdout. Multiple
files require `--check` or `--write`. No input argument means stdin; `-` also
selects stdin. `.dzn` paths select data mode;
`.mzn` and other paths select model mode. Data mode permits only top-level
assignments. `--stdin-filepath PATH` supplies the stdin language mode and
path used in diagnostics and EditorConfig lookup; without it stdin uses model
mode and default settings without filesystem lookup. This option requires stdin,
and mixed stdin/file inputs are rejected.

`--check` writes nothing and exits 1 if any input would change, or 0 when all
inputs are already formatted. `--write` requires file paths and replaces each
successfully formatted file through a sibling temporary file, retaining its
permissions. It refuses writes through symlink inputs; stdout and check modes
may read them. The two modes are mutually exclusive.

Successful formatting or writing exits 0. Syntax, formatting-directive,
invalid UTF-8, usage and I/O errors exit 2 and take precedence over check changes.
Diagnostics go to stderr; syntax and directive diagnostics include the path,
line/column and byte range. Independent files continue to process after an error,
and failing files remain untouched. Check and write modes emit no source to stdout.

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
  generic type-inst variables, `any`-qualified variables and default expressions.
  Function, predicate and test declarations support declaration annotations; the
  pinned grammar does not permit those on annotation declarations or parameters.
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
- Declaration and expression annotations. Annotation literals use the same
  identifier and call syntax as other expressions.
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
field and accept a trailing comma. Tuple literals and record literals require
their first comma, including `(value,)` and `(field: value,)`. These follow the
pinned grammar: unary tuple syntax parses despite the specification prose
excluding it. MiniZinc 2.10.1 accepts unary tuples and also accepts singleton
record literals without the comma. Zincite follows the written grammar.
The structured fixture and its formatted output pass MiniZinc 2.10.1 model checks.
`variant_record` and `case` are reserved keywords without productions in the
pinned grammar; they remain unsupported and produce diagnostics. Unsupported
input produces diagnostics. Range-type bounds follow the numeric-expression
grammar: their parentheses may contain numeric operators, identifiers and calls;
call arguments and annotations use the general expression grammar.

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

For input files and `--stdin-filepath`, the command searches parent `.editorconfig`
files, respecting `root = true`, matching section order and `unset`. Supported
properties are `indent_style` (`space` or `tab`), `indent_size` (positive integer or
`tab`), `tab_width` (positive integer), `max_line_length` (positive integer or
`off`), `end_of_line` (`lf`, `crlf` or `cr`), `insert_final_newline` and
`trim_trailing_whitespace` (`true` or `false`), and `charset` (`utf-8` or
`utf-8-bom`). Unknown properties are ignored; invalid supported values and other
charsets fail. UTF-8 BOM input is accepted; output has a BOM only when
`charset = utf-8-bom` is resolved.

Explicit `--indent-style`, `--indent-size`, `--tab-width`, `--end-of-line` and
`--max-line-length` options override the corresponding properties. Their values
match the properties above, except `--indent-size` requires a positive integer.
Check and write modes use the same resolved settings. For example:

```sh
zincite-fmt --indent-size 2 --end-of-line crlf --max-line-length off model.mzn
```

Before a stable release, formatting may change between versions.

## Linter usage

Install and run the lint command from this checkout:

```sh
cargo install --path crates/zincite-lint
zincite-lint model.mzn other.mzn
zincite-lint < model.mzn
zincite-lint --stdin-filepath data.dzn < data.dzn
zincite-lint --help
```

Explicit files may be checked together; no input or `-` reads stdin. Stdin cannot
be mixed with file inputs. `--stdin-filepath` supplies its diagnostic path and
language mode, with `.dzn` selecting data syntax. Diagnostics go to stderr with
path, line/column and precise byte ranges; warnings identify their severity and
rule. The command exits 0 when clean, 1 for unsuppressed warnings, and 2 for
input, syntax, suppression or usage errors. Errors take precedence and independent
files are still checked. Source files are never rewritten.

The `missing-constraint-label` warning suggests a string label to explain a
constraint's modelling intent. A direct header label such as
`constraint :: "explanation" true;` counts, including interpolated strings.
Strings inside calls and expression annotations do not count. This is advice,
not a claim that an unlabelled constraint is incorrect or slower.

These standalone comments suppress a rule throughout the next top-level item,
including its local constraints and bindings:

```minizinc
% zincite-lint: ignore naming
% zincite-lint: ignore missing-constraint-label
constraint true;
```

Consecutive comments may suppress both rules. Unknown rule IDs, malformed or
misplaced directives and directives without a following item fail the file.
Both rules are warnings enabled by default. `naming` checks ordinary values,
arrays, callable names, parameters, fields, dependent index bindings, generators
and local declarations for `snake_case`. Enum/type names, constructors and
explicitly declared parameter sets/domains use `UpperCamelCase`; set-valued
decision variables use `snake_case`. Arrays keep `snake_case` even when their
elements are sets. `SHOUTY_CASE` is discouraged. Anonymous `_` and quoted names
are exempt. A single leading underscore is allowed for an unused binding, while
the remaining spelling is checked normally.

Naming checks declared syntax roles and skips references, standalone data-file
assignment targets and classifications that need alias or type resolution.
Syntax errors omit all lint rules for that file. The shared parser's supported
syntax and exclusions described above also apply to linting; includes are never
loaded and name resolution, type checking and solver analysis are outside scope.

Use the library independently with `zincite_lint::lint(&parsed)`. It returns
warnings with rule, severity, message and byte range, or owned syntax/suppression
diagnostics without partial warnings.

## Scope

The initial target is MiniZinc 2.10.1, covering model (`.mzn`) and data (`.dzn`)
files. The formatter supports explicit files and stdin, with stdout, check
and in-place modes, with EditorConfig and explicit CLI layout overrides.
Linting checks declared naming roles and advises on missing constraint labels.

Formatting will preserve comments and source meaning, with layouts chosen to
keep later diffs small. The default width is 120 columns with four-space
indentation; EditorConfig and explicit command-line options control the
supported layout settings.

Zincite will own its Rust parser and source representation. Full type checking,
flattening, solving and automatic semantic rewrites are outside the initial
scope.

## Development

The [area brief](.zdev/base/brief.md) records the decisions and detailed behavior.
The [task list](.zdev/base/TASKS.md) records implementation order and dependencies.
Read [AGENTS.md](AGENTS.md) before making changes.

Work is tracked with zdev in the `base` area. To inspect the current state and
validate the planning records:

```sh
zdev status base
zdev check base --format json
```

Run the standard Cargo checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep tests focused on observable behavior: exact source coverage, recovery after
an error, stable formatting, preserved comments and useful lint diagnostics.
MiniZinc 2.10.1 can provide supplementary acceptance checks for complete models
and model/data pairs. Compiler acceptance alone does not prove that formatting
preserves meaning.

The developer [save-path benchmark](scripts/README.md) records release latency,
peak child RSS and CPU/allocation evidence separately from behavior tests. Local
reports stay under ignored `target/benchmarks/` by default.

The developer [full corpus command](scripts/README.md#full-corpus-correctness-check)
checks the complete published Challenge archive and local model/data tree without
rewriting originals. Its [baseline](scripts/corpus-baseline.md) records coverage
and current failures; detailed private reports default to ignored `target/corpus/`.

## License

Zincite is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
