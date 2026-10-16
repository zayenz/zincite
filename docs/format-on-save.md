# Formatting on save

Configure your editor to send the current buffer to `zincite fmt` and replace
it with the formatted output only when the command succeeds. This also works
for files that have not been saved yet.

After [installing Zincite](getting-started.md), use the installed executable
and these arguments in your editor's external-formatter settings:

```text
fmt --stdin-filepath /absolute/path/to/the/edited/model.mzn
```

The command name is `zincite`. If your editor needs an absolute executable
path, use the path reported by `command -v zincite`. If you installed the
standalone `zincite-fmt` instead, use that executable and omit `fmt`.

Send the current in-memory buffer as UTF-8 on stdin. Use the actual destination
path, even for a new file that has not been saved yet. A `.dzn` path selects data
syntax; other paths select model syntax. The path also selects the EditorConfig
lookup starting directory and appears in diagnostics. No input argument is
needed: the command reads stdin by default. Plain stdin without
`--stdin-filepath` uses model syntax and default settings without filesystem
configuration lookup.

The default style uses four spaces, LF, a final newline and a 120-column limit.
EditorConfig supplies indentation, tab width, editable line endings, width,
final-newline and trailing-whitespace settings, and UTF-8/BOM selection.
Supported command-line style overrides take precedence; see `--help` for their
values and the [formatting style gallery](formatting-style.md) for examples.
Formatting may change between versions in the 0.1 series.

## Editor recipe

1. Take the edited buffer and its destination path at the start of the save.
2. Start one fresh formatter process, passing the path as one argument. Send the
   complete buffer on stdin and capture stdout and stderr separately.
3. Wait for process completion and complete stdout capture. On exit status 0,
   replace the buffer with the captured stdout, then save it. An already formatted
   buffer produces the same bytes. Successful output may be empty for empty input.
4. On nonzero status, timeout, launch failure or incomplete output capture, keep
   the original buffer and show stderr or the process failure through the editor's
   usual diagnostic UI. Follow the editor's existing policy for saving that
   unchanged buffer.

Do not stream stdout into the buffer, merge stderr into source, or treat a
nonempty stdout fragment as success. Syntax, formatting-directive, configuration,
usage and I/O errors exit 2. Syntax, directive and configuration failures produce
no formatted stdout. Status 1 belongs to `--check`, so use the default stdout mode
for this recipe. `--write` writes disk files and does not format an unsaved buffer.

For an editor adapter written in Python, the buffer replacement can look like
this. The helper functions belong to the editor integration:

```python
import subprocess

# buffer_bytes and destination_path come from the editor.
original = buffer_bytes
try:
    result = subprocess.run(
        [zincite_path, "fmt", "--stdin-filepath", destination_path],
        input=original, capture_output=True, timeout=30, check=False,
    )
except (OSError, subprocess.TimeoutExpired) as error:
    show_process_error(error)             # keep original
else:
    if result.returncode == 0:
        replace_buffer(result.stdout)    # complete output, including empty output
    else:
        show_diagnostics(result.stderr)  # keep original
```

`zincite_path` is the installed executable. Launching `cargo run` on every save
would also invoke Cargo. Formatting a buffer does not load includes or run lint
checks; configure linting as a separate editor action if you want that feedback.

## Performance

Use a release build for editing. The project targets saves below 100 ms at the
95th percentile for its ordinary editing cases; large generated data can take
longer. The [performance reports](../scripts/save-performance-current.md)
contain the measured inputs, timings and methodology.
