# Command reference

`zincite fmt`, `zincite lint` and `zincite query` use the same options as
`zincite-fmt`, `zincite-lint` and `zincite-query`, respectively. Installed help
lists the accepted options:

```sh
zincite --help
zincite fmt --help
zincite lint --help
zincite query --help
```

## Input and output

Formatting and linting accept files, directories or stdin. With no input or `-`,
they read stdin; do not mix stdin with file inputs. `--stdin-filepath PATH`
selects language mode and the configuration lookup path. A `.dzn` path selects
data syntax; plain stdin uses model syntax. Directory discovery includes hidden
and Git-ignored `.mzn` and `.dzn` files and skips `.git`.

Formatter stdout mode accepts one input. Multiple files require `--check` or
`--write`; directories support `--check` only. Formatter diagnostics go to stderr.
Lint diagnostics go to stderr; ordinary analysis preserves files.

Queries accept one input file or `-`. With no input file, they read stdin.
Choose one way to supply the query:

| Form | Example |
| --- | --- |
| Positional pipeline | `zincite query 'filter(kind("constraint")) | count' model.mzn` |
| Inline program | `zincite query --program '2 + 3' model.mzn` |
| Program file | `zincite query --program-file report.zq model.mzn` |

`-f` is short for `--program-file`. These forms cannot be combined in one
invocation. Query diagnostics go to stderr and identify the query location;
errors in an input file retain that file's path and range.

Programs use `let` for input reads and calculations. A standalone expression
emits its result; its final semicolon is optional. Native selections print
source, while calculated values and records produce JSON. `report expression`
explicitly requests JSON for a native result. For example, `--program 'count'`
prints a number, while `--program 'report count'` wraps it in the JSON report
with completeness and limitation fields.

A data edit program requires `.dzn` input, `--model` for target declarations and
an explicit final `emit;`. Read-only
programs reject `--diff` and `--write`. See the [query guide](queries.md) for
reports and the [data-edit guide](data-edits.md) for transformations.

Program options `--max-work`, `--max-results`, `--max-items` and `--max-nesting`
limit evaluation and output. They are not available for positional pipelines.
An exceeded limit fails the request; it does not silently truncate the result.
The [program reference](reference.md#query-programs) defines the default limits.

Query output is buffered until evaluation and rendering succeed. If a later
statement fails, the command produces no partial report on stdout.

## Explicit writes

Formatter `--write` and query `--write` require explicit regular files and
refuse symlinks. Lint uses `--fix` with its own [fix eligibility and write
restrictions](linting.md#suppression-and-explicit-fixes). Formatter `--check` and
`--write` are mutually exclusive; query `--diff` and `--write` are mutually
exclusive. A failing formatter input is not written; other independent files
continue. Query write validates the complete candidate and checks the original
bytes again before replacing the file. Writes retain file permissions.

## Exit codes

| Code | Formatter | Linter | Query |
| --- | --- | --- | --- |
| `0` | Formatting succeeded; check found no changes | No unsuppressed warnings or errors | Complete result |
| `1` | Check found formatting changes | Unsuppressed warnings | Incomplete semantic report or data candidate |
| `2` | Input, syntax, configuration, directive, usage or I/O error | Input, syntax, settings, directive, usage, dependency or I/O error | Query, input, model, usage or I/O error |

Errors take precedence across formatter/linter inputs. A query's empty fragment
selection succeeds with no source; its count is 0. Lint limitations alone retain
status 0 or 1. Status 0 does not establish complete semantic coverage.
