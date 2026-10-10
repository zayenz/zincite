# Formatting on save

Build the release formatter from the repository root:

```sh
cargo build --release -p zincite-fmt --bin zincite-fmt
```

Configure your editor's external formatter with the absolute path to
`target/release/zincite-fmt` and these arguments:

```text
--stdin-filepath /absolute/path/to/the/edited/model.mzn
```

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
values. Formatting may change between versions before a stable release.

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

A generic adapter's relevant operation is:

```python
# buffer_bytes and destination_path come from the editor.
original = buffer_bytes
try:
    result = subprocess.run(
        [formatter_path, "--stdin-filepath", destination_path],
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

`formatter_path` is the built executable, not `cargo run`. The save command
checks syntax and formats one buffer. It does not load includes, run semantic
lint, invoke MiniZinc or discover files in the workspace. No editor extension,
persistent process or formatting cache is required.

## Measured behavior

The [save performance checkpoint](../scripts/save-performance.md) records
50 fresh processes per case and configuration, complete stdout capture,
error-buffer preservation and separate native child RSS checks. The original
base-048 representative cases passed their budgets; the later public wide matrix
prevented an overall acceptance claim.

The current base-081 recheck uses the existing scalar-matrix renderer and syntax
storage with no additional source change. Fifty fresh saves per configuration
measure 89.184/89.121 ms p95 for the 885,733-byte public wide input. Separate
native stdin saves measure 51.406250/51.375000 MiB for default/nested settings,
passing the unchanged 100 ms / 64 MiB gate. Every retained representative case
within 1 MiB passes its own latency/RSS budget. The dense 966,669-byte control
measures 87.134/87.118 ms p95 and 47.796875/47.812500 MiB. Complete output and
status match the retained baseline; focused matrix/table fidelity and scope-drop
checks pass.

The checkpoint preserves the earlier failed campaigns, unsuccessful experiments,
separate CPU/allocation evidence and first-use observations. Current first-use
controls show no material launch delay, but do not explain the historical host
startup cause or establish controlled cold-cache performance. Large inputs remain
supported; interactive budgets cover inputs up to 1 MiB. Fresh full no-rules
corpus validation remains deferred under the user's explicit sweep restriction.
The older corpus report has different source pins and cannot prove current
full-corpus behavior. These save results do not resolve corpus or batch-memory
limits.
