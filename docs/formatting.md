# Format MiniZinc files

Use the formatter to give models a consistent layout while keeping comments,
identifier names and literal spelling. Preview a file before writing it, or
check a directory as part of CI. The [staffing walkthrough](first-model.md#formatting-the-model)
shows the input and output together.

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

See the [formatting style gallery](formatting-style.md) for before-and-after examples
of constraints, comprehensions, matrices and comments.

Syntax or formatting-directive errors leave the affected file untouched.
`--write` preserves file permissions, refuses symlinks and checks that the
source has not changed since it was read before replacing it. Other independent
files are still processed when one fails.

## Format an editor buffer

Use the [format-on-save recipe](format-on-save.md) for stdin invocation, buffer
replacement and failure handling. The [support page](support.md#performance)
links to performance measurements.

## Configuration

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

## Keeping layout unchanged

Use the [formatting directive examples](formatting-style.md#formatting-directives)
to preserve a complete item or region. The [formatter reference](reference.md#formatter-library)
defines protected ranges and invalid marker placement.

See the [command reference](cli.md) for exit codes and the
[formatter library reference](reference.md#formatter-library) for Rust use.
