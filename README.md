# Zincite

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/zincite-logo-light.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/zincite-logo-dark.svg">
  <img alt="Zincite — orange crystal Z logo" src="assets/zincite-logo-dark.svg" width="420">
</picture>

Zincite provides source tools for MiniZinc model (`.mzn`) and data
(`.dzn`) files:

- **`zincite fmt`** (or **`zincite-fmt`**) formats source, checks formatting, and writes changes in place.
- **`zincite lint`** (or **`zincite-lint`**) is a linter for MiniZinc models and data.
- **`zincite query`** (or **`zincite-query`**) inspects items, expressions, calls and annotations, emits JSON/count reports, and explicitly edits data assignments.

The tools are written in Rust and share a parser that preserves source spelling,
comments, whitespace and source locations. The commands do not require a MiniZinc
compiler or solver.

Zincite is under development, targeting MiniZinc 2.10.1. Some valid syntax remains
unsupported, and formatting may change before a stable release. See
[current limitations](#current-limitations) before using it across a codebase.

## Installation

Install from a checkout with a current stable Rust toolchain:

```sh
git clone https://github.com/zayenz/zincite.git
cd zincite
cargo install --locked --path .
```

The root command provides `zincite fmt`, `zincite lint` and `zincite query`. To install the
standalone commands instead, use:

```sh
cargo install --locked --path crates/zincite-fmt
cargo install --locked --path crates/zincite-lint
cargo install --locked --path crates/zincite-query
```

Both invocation forms use the same options, defaults and exit codes. Run
`zincite --help` for the command list.

The `0.0.0-pre` packages on crates.io are empty name reservations. Install from
source to use the tools.

## Formatting

```sh
# Print formatted source to stdout.
zincite fmt model.mzn

# Check files or a directory without changing them.
zincite fmt --check model.mzn data.dzn
zincite fmt --check models/

# Format explicit files in place.
zincite fmt --write model.mzn data.dzn
```

With no input argument, or with `-`, the formatter reads stdin. Multiple files
require `--check` or `--write`; directories support `--check` only. Directory
checks include hidden and Git-ignored `.mzn` and `.dzn` files and skip `.git`.

Formatting preserves comments and literal spelling. It uses four-space
indentation and a default maximum width of 120 columns, expands `forall` bodies,
aligns matrix columns, and sorts adjacent include groups with `globals.mzn`
first. Long literals and preserved comments may exceed the width.

Syntax or formatting-directive errors leave the affected file untouched.
`--write` preserves file permissions and refuses symlinks. Other independent
files are still processed when one fails.

### Editor integration

Send the editor buffer to stdin and use its path for language mode and
EditorConfig lookup:

```sh
zincite fmt --stdin-filepath path/to/model.mzn < path/to/model.mzn
```

Replace the buffer with stdout only when the command succeeds. A `.dzn` path
selects data syntax. Plain stdin without `--stdin-filepath` uses model syntax
and default formatting settings.

### Configuration

The formatter reads `.editorconfig` files. For example:

```ini
root = true

[*.{mzn,dzn}]
indent_style = space
indent_size = 4
max_line_length = 120
end_of_line = lf
insert_final_newline = true
```

It also supports `tab_width`, `trim_trailing_whitespace` and `charset` (`utf-8`
or `utf-8-bom`). Set `max_line_length = off` to disable width-based wrapping.
Comments, literal contents and skipped regions retain their original text.

Command-line options override EditorConfig:

```sh
zincite fmt --indent-size 2 --max-line-length 80 model.mzn
```

Run `zincite fmt --help` (or `zincite-fmt --help`) for all options.

### Keeping layout unchanged

Place `% zincite-fmt: skip` on its own line before a top-level item to preserve
that item's layout. Use standalone `% zincite-fmt: off` and `% zincite-fmt: on`
comments around a region of complete top-level items to preserve the region.
Skipped source must still parse; unmatched or misplaced markers are errors.

## Linting

```sh
zincite lint model.mzn data.dzn
zincite lint models/
zincite lint --rules naming model.mzn
zincite lint --stdin-filepath model.mzn < model.mzn
```

Use `--rules` with a comma-separated list to select rules. To suppress a rule
throughout the next top-level item, place a standalone comment before it:

```text
% zincite-lint: ignore missing-constraint-label
```

Unknown rules and malformed, misplaced or dangling suppression comments are
errors. Ordinary analysis writes diagnostics to stderr and preserves source.
Use `--diff` to preview eligible fixes or `--fix` to apply them. See the
[linting guide](docs/linting.md) for fix selection and restrictions.
It accepts files, directories or stdin, with the same directory discovery and
`.dzn` selection as the formatter. Run `zincite lint --help` (or `zincite-lint --help`) for details.

## Querying items

```sh
# Print constraints in written order, with their attached comments.
zincite query 'filter(kind("constraint"))' model.mzn

# Count assignments in a data file.
zincite query 'items | filter(kind("assignment")) | count' data.dzn

# Print the first assignment named capacity from stdin.
zincite query --stdin-filepath data.dzn 'filter(name("capacity")) | head(1)' < data.dzn

# Preview a changed capacity without writing the data file.
zincite query --diff 'filter(name("capacity")) | set_value("20")' data.dzn

# Explicitly remove the selected assignment from one data file.
zincite query --write 'filter(name("unused")) | remove' data.dzn
```

Inspect nested calls and solve annotations without loading a MiniZinc library:

```sh
zincite query 'filter(kind("constraint")) | expressions | call_names | tally | json' model.mzn
zincite query 'filter(kind("solve")) | subtree | annotation_names | unique | json' model.mzn
zincite query 'expressions | range(20,80) | json' model.mzn
```

Supply one query and at most one file or `-`; omitted input reads stdin.
An empty query or `items` emits the exact document. `filter` supports `kind`
and `name` predicates, parentheses and Boolean `not`, `and`, `or` in that
precedence order. `head(n)` keeps the first `n` selected items. `count` emits a
decimal count; `emit` emits original source, and source emission is implicit.
`emit` ends the pipeline; `count` may be followed by `json`.

Selections use top-level items in written order and compare identifier identity,
including quoted names. `filter` and `head` emit fragments containing selected
items and attached comments; separated section comments stay in document output.

`set_value("EXPRESSION")` replaces selected assignment right-hand sides;
`remove` deletes selected assignments and attached comments. These stages require
`.dzn` data mode and return the entire validated candidate. One edit stage may be
followed only by `emit_document`, which is also implicit. Comments before and
after a replaced expression remain; internal expression comments cause a located
placement error. Separated section comments and untouched bytes remain in the
candidate. Removal keeps next-item directives with their targets and rejects
unbalanced formatter regions.

Default editing prints the candidate and leaves files unchanged. `--diff` previews
it; `--write` replaces the explicit regular file after checking the original bytes
again and preserving permissions. The modes are mutually exclusive; writes
reject stdin and symlinks. Edits change instance meaning and establish syntax
only. Queries check syntax without loading includes or evaluating data. Query errors
exit 2 with empty stdout. See the [query reference](docs/reference.md#query-library)
for kind strings, comment rules and limits, or run `zincite query --help`.

## Exit codes

| Code | `zincite fmt` / `zincite-fmt` | `zincite lint` / `zincite-lint` |
| --- | --- | --- |
| `0` | Formatting succeeded; in check mode, no changes needed | No warnings |
| `1` | Check mode found formatting changes | Unsuppressed warnings |
| `2` | Input, syntax, configuration, directive or usage error | Input, syntax, directive, usage or dependency error |

`zincite query` / `zincite-query` exits 0 on success and 2 on query, input,
usage or I/O errors. An empty fragment selection succeeds and emits no source;
its count is 0.

Errors take precedence when checking multiple files. Diagnostics include a file
path and source location where applicable.

## Current limitations

- Syntax coverage targets MiniZinc 2.10.1, but corpus checks still identify valid
  inputs that are rejected and large inputs that time out. See the
  [syntax reference](docs/reference.md#syntax-support) and
  [corpus checkpoint](scripts/formatter-corpus-checkpoint.md).
- Parsing checks syntax. Formatting and the two default lint rules do not load
  includes, resolve names, type-check models or assess solver performance.
- Naming conventions (`naming`) and constraint labels (`missing-constraint-label`)
  are enabled by default. The `thesis` preset selects fourteen modelling rules;
  `all` selects all 26 implemented rules. Semantic rules use an explicit model
  and include context. Unsupported required facts produce located limitations;
  rule availability does not promise complete coverage of every input.
- Files must be UTF-8. Data files accept top-level assignments. JSON data and
  solver output formats are outside the supported input formats.

## Libraries

The commands have reusable Rust libraries:

| Crate | Purpose |
| --- | --- |
| `zincite-syntax` | Lexer, parser, source locations and a concrete syntax tree |
| `zincite-fmt` | Formatting a parsed file, with configurable layout |
| `zincite-lint` | Lint rules, diagnostics and explicit model/include context |
| `zincite-query` | Source inspection, JSON/count reports and explicit data assignment edits |

See the [syntax and library reference](docs/reference.md) for APIs and detailed
language coverage. Generate local API documentation with
`cargo doc --workspace --no-deps --open`.

## Development

Read [AGENTS.md](AGENTS.md) for repository guidance and the
[project brief](.zdev/base/brief.md) for detailed behavior and planned work.
Run the workspace checks with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The [developer scripts](scripts/README.md) document corpus checks and performance
measurements.

## License

Zincite is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
