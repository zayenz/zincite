# Zincite

Zincite is a Rust project for MiniZinc source tooling. `zincite-fmt` formats a
scalar and collection model subset using the shared `zincite-syntax`
parser and concrete syntax tree. The tree retains source spelling, comments,
whitespace and byte ranges. `zincite-lint`, the planned static analyser, is not
implemented yet.

`zincite_syntax::lex` owns UTF-8 source text and exposes read-only source,
tokens and lexical diagnostics. Token ranges cover every byte exactly once,
including comments, whitespace and erroneous input. Slice the source with a
token's half-open byte range to read its original spelling.

The lexer recognizes the lexical forms in the
[MiniZinc 2.10.1 specification](https://docs.minizinc.dev/en/2.10.1/spec.html),
including quoted names, numerals, plain strings and Unicode operators. It
reports malformed lexical input and resumes at a reliable boundary. Strings
containing interpolation are retained as single tokens with an explicit
unsupported diagnostic. Their expressions are not tokenized yet. Lexing does
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

## Formatter usage

Install the command from this checkout:

```sh
cargo install --path crates/zincite-fmt
zincite-fmt model.mzn
zincite-fmt < model.mzn
zincite-fmt - < model.mzn
zincite-fmt --help
```

The command accepts one UTF-8 file or stdin and writes the complete formatted
model to stdout. No argument means stdin. It never rewrites the input file.
Successful formatting exits 0. Syntax, unsupported-input, usage and I/O errors
exit 2, with diagnostics on stderr and no formatted source on input errors.
Syntax diagnostics include the path, line/column and byte range.

The temporary grammar supports:

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
- All unary and binary operators from the pinned specification, including
  backtick operators, Unicode spellings and half-open or one-sided ranges.
- Parenthesized expressions and ordinary or quoted calls, including nested calls,
  named arguments, empty argument lists and trailing commas.
- Generator calls such as `forall(i in Indices)(i > 0)` and `sum(i in Indices)(i)`,
  including supported nested expressions, annotations and array access.
- Declaration and expression annotations. Annotation literals use the same
  identifier and call syntax as other expressions.
- Constraints with an expression and an optional direct `:: "label"`.
- `solve satisfy`.

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
an added comma. These width decisions currently apply only to generator headers
and bodies, using Unicode scalar columns and four-column tab stops. General
expression wrapping and configurable layout settings remain unimplemented.
Matrices use one logical row per line; column
alignment and width-based row wrapping are not implemented yet. It normalizes
editable spacing and line endings to LF, adds a final newline and preserves blank-line groups while
collapsing excess blank layout lines.

Field access, structured types/values, generic type-inst variables,
control expressions, interpolated strings and other item families
remain unsupported and produce diagnostics. Range-type bounds follow the
numeric-expression grammar: their parentheses may contain numeric operators,
identifiers and calls; call
arguments and annotations use the general expression grammar.

Parsing checks syntax only: it does not resolve includes or names, require a
solve item, infer types or check semantic validity. For example, accepting a
named argument does not establish that a callable has that parameter, and
accepting a range expression does not establish that its bounds have valid types.
Written indices and cardinalities are retained without evaluation, domain inference,
contiguity checks or rectangularity checks. The pinned grammar permits an empty
index tuple `()` as a key, although the installed MiniZinc 2.10.1 compiler rejects
it. That compiler also crashes on a mixed binder/plain-index declaration during
`--model-check-only`; Zincite accepts these forms from the specification.

Use the libraries independently of the command:

```rust,ignore
let parsed = zincite_syntax::parse("int: count=2; constraint true; solve satisfy;");
let formatted = zincite_fmt::format(&parsed)?;
```

The formatter returns the parse diagnostics on error. It produces no partial
formatted source. Before a stable release, formatting may change between
versions.

## Scope

The initial target is MiniZinc 2.10.1, covering model (`.mzn`) and data (`.dzn`)
files. The formatter will support explicit files and stdin, with stdout, check
and in-place modes. Data mode, check/write modes and configuration are not
implemented yet. The first lint rules will check naming conventions and
advise on missing constraint labels.

Formatting will preserve comments and source meaning, with layouts chosen to
keep later diffs small. The default width is 120 columns with four-space
indentation; EditorConfig and explicit command-line options will control the
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

## License

Zincite is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
