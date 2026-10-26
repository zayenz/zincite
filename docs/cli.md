# Command reference

`zincite fmt`, `zincite lint` and `zincite query` use the same options as
`zincite-fmt`, `zincite-lint` and `zincite-query`, respectively. Installed help
lists the accepted options. `zincite --version` reports `0.3.0` and a build
revision when available:

```sh
zincite --version
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

A `create`/`replace` batch requires `.dzn` input, `--model` for target
declarations and an explicit final `emit;`. A single edit pipeline, such as
`filter_elements`, follows its own input and model requirements. Read-only
programs reject `--diff` and `--write`. See the [query guide](queries.md) for
reports and the [data-edit guide](data-edits.md) for both edit forms.

Program options `--max-work`, `--max-results`, `--max-items` and `--max-nesting`
limit evaluation and output. They are not available for positional pipelines.
An exceeded limit fails the request; it does not silently truncate the result.
The [program reference](reference.md#query-programs) defines the default limits.

Query output is buffered until evaluation and rendering succeed. If a later
statement fails, the command produces no partial report on stdout.

## Candidate checking from a checkout

`zincite check` and `zincite-check` share one CLI:

```sh
zincite check [--format text|json] model.fzn candidate.dzn
zincite-check [--format text|json] model.fzn candidate.dzn
zincite check --export prepared.json model.fzn
zincite-check --load [--format text|json] prepared.json candidate.dzn
```

`--export` prepares the source and creates a new JSON file, refusing existing
paths. `--load` validates that artifact and checks without parsing FlatZinc.
Both entry points support both options. Reports go to stdout; input and
execution errors go to stderr without partial stdout. The checker reads literal
Boolean, integer and finite integer set assignments, checks every available domain
and supported relation, and reports independent completeness. Supported linear,
full-reification and set definitions derive outputs from known inputs before
checking. The admitted relations include one strict disjunctive global with fixed
nonnegative durations. Checking starts no compiler or solver.
The [checker guide](../crates/zincite-check/README.md)
defines the admitted syntax, predicate coverage, metadata and exact integer JSON
encoding, including set endpoints. These commands are available from the checkout,
outside the published 0.3.0 packages.

## Explicit writes

Formatter `--write` and query `--write` require explicit regular files and
refuse symlinks. Lint uses `--fix` with its own [fix eligibility and write
restrictions](linting.md#suppression-and-explicit-fixes). Formatter `--check` and
`--write` are mutually exclusive; query `--diff` and `--write` are mutually
exclusive. A failing formatter input is not written; other independent files
continue. Query write validates the complete candidate and checks the original
bytes again before replacing the file. Writes retain file permissions.

## Exit codes

| Code | Formatter | Linter | Query | Checker (checkout) |
| --- | --- | --- | --- | --- |
| `0` | Formatting succeeded; check found no changes | No unsuppressed warnings or errors | Complete result | Valid, complete candidate or successful export |
| `1` | Check found formatting changes | Unsuppressed warnings | Incomplete semantic report or data candidate | Invalid or incomplete report |
| `2` | Input, syntax, configuration, directive, usage or I/O error | Input, syntax, settings, directive, usage, dependency or I/O error | Query, input, model, usage or I/O error | Usage, I/O, preparation, loading, assignment or arithmetic error |

Errors take precedence across formatter/linter inputs. A query's empty fragment
selection succeeds with no source; its count is 0. Lint limitations alone retain
status 0 or 1. Status 0 does not establish complete semantic coverage.
