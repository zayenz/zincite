# Nested layout measurements

Measured 2026-10-15 for base-134 on an Apple M1 Max (10 cores, 64 GB), macOS
26.6.2, Rust 1.98.1. Singleton arrays and parentheses no longer add a separate
indented block around every wrapper. Conditional branches retain their own
indentation.

The formatter baseline is `9ca76745822af3656fdf7a491e43c90d29b3ab9e`. The
before executable also contains base-133's independent CLI write protection;
its formatter library is unchanged from that baseline. The after executable
contains the base-134 formatter change. Both are release builds. The build used
`CARGO_TARGET_DIR=/tmp/zincite-target-134 CARGO_BUILD_JOBS=2 cargo build --release -p zincite-fmt`.

Measured builds (SHA-256):

- Before: `c9324e68687bef2e0fb801a744655758ea7af7daad23e63de751fcb0ad42eff0`.
- After: `eccd2f03c92558b891c8df583b82b9ddb3d5ebc922a950ca607d7340fe4ddc9d`.
- After formatter source: `12a6b38beaadf7130c32a8396d26289c366d8274f09ca21cdc8db778e484a1ef`.

## Layout size and width

The conditional family is exactly the existing `bench-save.py` shape:

```python
"int: value=" + "if true then (" * depth + "1" + ") else 0 endif" * depth + ";"
```

The array families use `"any: x=" + "[" * depth + body + "]" * depth + ";"`.
The scalar body is `1`; the binary body is
`first_operand+second_operand+third_operand+fourth_operand+fifth_operand`.
Conditional cases use width 120; array stress cases use width 60. All use the
default four-space indentation. Widths below count columns; these inputs are ASCII.

| Case | Input bytes | Limit | Before bytes | After bytes | Before maximum width | After maximum width |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| fitting-scalar | 15 | 120 | 18 | 18 | 17 | 17 |
| fitting-conditional | 38 | 120 | 49 | 41 | 21 | 40 |
| nested-conditional-array | 63 | 40 | 98 | 74 | 21 | 30 |
| mixed-comprehension | 54 | 120 | 67 | 59 | 23 | 58 |
| comment-boundary | 32 | 120 | 42 | 42 | 29 | 29 |
| explicit-list | 18 | 120 | 25 | 25 | 12 | 12 |
| conditional-depth3 | 97 | 120 | 172 | 108 | 25 | 63 |
| binary-array-depth3 | 85 | 60 | 157 | 100 | 60 | 60 |
| scalar-array-depth3 | 15 | 60 | 18 | 18 | 17 | 17 |
| conditional-depth40 | 1,133 | 120 | 22,036 | 17,320 | 217 | 164 |
| binary-array-depth40 | 159 | 60 | 7,346 | 195 | 180 | 60 |
| scalar-array-depth40 | 89 | 60 | 6,612 | 93 | 162 | 60 |
| conditional-depth160 | 4,493 | 120 | 609,316 | 308,800 | 1,177 | 644 |
| binary-array-depth160 | 399 | 60 | 105,866 | 420 | 660 | 60 |
| scalar-array-depth160 | 329 | 60 | 103,212 | 337 | 642 | 60 |

Every before and after output in the table is stable on a second pass. The fitting
scalar case keeps all three dimensions. The comment boundary and explicit
multiline list retain the same output.

At depth 160, scalar arrays shrink from 103,212 to 337 bytes and binary arrays
from 105,866 to 420 bytes. Opening and closing chains split at width 60 without
adding one indentation level per array. Their output grows with the delimiters.

True conditional nesting still grows quadratically in output bytes: every deeper
branch carries its structural indentation. Removing wrapper blocks reduces the
depth-160 conditional output from 609,316 to 308,800 bytes, but the deepest branch
still starts at 640 columns. The measured maximum width is 644. This retains the
branch structure; it introduces no depth cap or special width exception.

At ordinary depth three, the conditional output changes from:

```minizinc
int: value = if true then
    (if true then
        (if true then
            (1)
        else
            0
        endif)
    else
        0
    endif)
else
    0
endif;
```

to:

```minizinc
int: value = if true then
    (if true then (if true then (1) else 0 endif) else 0 endif)
else
    0
endif;
```

The long binary array at width 60 changes from:

```minizinc
any: x = [
    [
        [
            first_operand + second_operand + third_operand +
                fourth_operand + fifth_operand,
        ],
    ],
];
```

to:

```minizinc
any: x = [[[first_operand + second_operand + third_operand +
    fourth_operand + fifth_operand]]];
```

## Fresh-process save measurements

Other workers paused builds and measurements for both runs. Each table row has
50 fresh subprocesses. Default settings and nested EditorConfig settings alternate
within each case. Before and after runs are sequential, with ordinary warm
filesystem caches. Latency includes process startup, stdin/stdout transfer,
configuration lookup, parsing and formatting.

All 800 measured saves succeeded. Already formatted ordinary input returned
identical bytes. The ordinary input is `tests/fixtures/integration.mzn`; its
formatted output is 1,356 bytes in both versions. `nested` and `nested-grown` are
the synthetic conditional stress cases at depths 40 and 160. They are outside
the ordinary editing latency target.

Each triple below is mean / median / p95 in milliseconds. Ranges give minimum
and maximum sample values; percentiles are per case and settings.

| Case / settings | Input bytes | Before mean / median / p95 | After mean / median / p95 | Before range | After range |
| --- | ---: | ---: | ---: | ---: | ---: |
| ordinary-changed / default-values | 1,163 | 4.55 / 4.50 / 5.06 | 4.36 / 4.27 / 4.79 | 4.20–5.58 | 3.99–5.00 |
| ordinary-changed / nested-editorconfig | 1,163 | 4.63 / 4.56 / 5.17 | 4.47 / 4.44 / 5.01 | 4.27–5.98 | 4.09–5.07 |
| ordinary-formatted / default-values | 1,356 | 4.49 / 4.43 / 4.91 | 4.95 / 4.64 / 6.43 | 4.02–5.79 | 4.25–8.80 |
| ordinary-formatted / nested-editorconfig | 1,356 | 4.54 / 4.52 / 5.04 | 4.95 / 4.72 / 6.52 | 4.11–5.31 | 4.12–7.59 |
| nested / default-values | 1,133 | 5.34 / 5.34 / 5.77 | 5.41 / 5.10 / 7.86 | 4.57–5.96 | 4.59–12.12 |
| nested / nested-editorconfig | 1,133 | 5.39 / 5.45 / 5.78 | 5.56 / 5.20 / 7.82 | 4.73–6.00 | 4.50–15.72 |
| nested-grown / default-values | 4,493 | 36.82 / 32.95 / 66.12 | 8.66 / 8.49 / 10.24 | 32.01–107.33 | 7.87–13.59 |
| nested-grown / nested-editorconfig | 4,493 | 35.49 / 33.08 / 66.70 | 8.71 / 8.56 / 9.68 | 32.05–73.06 | 7.96–12.46 |

The driver's first recorded ordinary invocation was 5.98 ms before and
5.58 ms after. These executables had already been exercised; these values are
not cold filesystem or first-ever execution measurements.

The first implementation attempt made depth-160 saves slower (97.26 ms median,
140.32 ms p95 with default settings). Full conditional previews repeated work
through nested bodies. A token-column lower bound now rejects oversized compact
candidates before rendering them. The final default median is 8.49 ms against
32.95 ms before. The earlier captures remain available below; they are excluded
from the final table.

The driver's sandboxed `/usr/bin/time -l` probes could not read
`sysctl kern.clockrate`. Separate authorized native probes succeeded. These are
single fresh-child peak resident measurements per case, in bytes:

| Case | Before peak RSS | After peak RSS |
| --- | ---: | ---: |
| ordinary-changed | 2,162,688 | 2,064,384 |
| ordinary-formatted | 2,162,688 | 2,113,536 |
| nested40 | 2,244,608 | 2,244,608 |
| nested160 | 3,751,936 | 3,211,264 |

## Validation and reproduction

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` pass. The formatter suite has 33 behavior checks;
the formatter CLI suite has seven. The focused checks compare reparsed structure,
second-pass output, compact/nonfitting arrays, nested conditionals, comments,
blank-line groups, explicit lists, tabs and a protected item. Existing checks retain
explicit/implicit comprehension correspondence and annotation attachment.

All 11 gallery before/after pairs match actual rendering and remain stable with
their stated settings. Three complete nested conditional, mixed comprehension
and comment-boundary models pass MiniZinc 2.10.1 `--model-check-only` before and
after formatting. No solver or corpus sweep was run.

Run from the checkout, substituting the before executable path if needed:

```sh
python3 scripts/bench-save.py --binary /tmp/zincite-base134-before-fmt --samples 50 --case ordinary-changed --case ordinary-formatted --case nested --case nested-grown --output /tmp/zincite-base134-before-save-accepted.json
python3 scripts/bench-save.py --binary /tmp/zincite-target-134/release/zincite-fmt --samples 50 --case ordinary-changed --case ordinary-formatted --case nested --case nested-grown --output /tmp/zincite-base134-after-save-accepted.json
/usr/bin/time -l /tmp/zincite-target-134/release/zincite-fmt --stdin-filepath /tmp/zincite-base134-controls/nested160.mzn < /tmp/zincite-base134-controls/nested160.mzn > /tmp/zincite-base134-controls/nested160.native.mzn
```

Local raw captures are not committed:

- `/tmp/zincite-base134-before-save-accepted.json` and `/tmp/zincite-base134-after-save-accepted.json`.
- `/tmp/zincite-base134-controls/layouts.json` and its paired source/output files.
- `/tmp/zincite-base134-controls/native-rss.json` and the native `*.time.txt` streams.
- `/tmp/zincite-base134-controls/compiler-checks.json` and the six checked models.
- Earlier implementation trial: `/tmp/zincite-base134-before-save.json` and `/tmp/zincite-base134-after-save.json`.
