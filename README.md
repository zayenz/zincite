# Zincite

Zincite is a Rust project for MiniZinc source tooling. `zincite-fmt` formats a
small scalar model subset using the shared `zincite-syntax` parser and concrete
syntax tree. The tree retains source spelling, comments, whitespace and byte
ranges. `zincite-lint`, the planned static analyser, is not implemented yet.

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

- `bool`, `int`, `float` and `string` declarations, with optional `var` or `par`
  and an optional atom initializer.
- Assignments to ordinary or quoted identifiers.
- Atom values: identifiers, quoted identifiers, integer/float/string literals,
  `true`, `false`, `infinity` and anonymous `_`.
- Constraints with an atom expression and an optional direct `:: "label"`.
- `solve satisfy`.

Items use semicolon separators; the last semicolon is optional on input.
Comments may occur between tokens. Formatting preserves comment and atom
spelling, puts a labelled constraint's expression on the following line,
normalizes editable spacing and line endings to LF, and adds a final newline.
It preserves blank-line groups while collapsing excess blank layout lines.

Calls, operators (including signed numerals), parentheses, absent atoms,
general annotations, other types and other item families are unsupported and
produce diagnostics. Interpolated strings are also unsupported. Parsing checks
syntax only: it does not resolve includes or names, require a solve item, or
check types. For example, accepting `_` as an atom does not establish that a
particular initializer is valid MiniZinc.

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
