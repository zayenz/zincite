# Command reference

`zincite fmt`, `zincite lint` and `zincite query` use the same options as
`zincite-fmt`, `zincite-lint` and `zincite-query`, respectively. Installed help
lists the accepted options. `zincite --version` reports `0.5.0` and a build
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

## Candidate checking

`zincite check` and `zincite-check` share one CLI:

```sh
zincite check [--format text|json] model.fzn candidate.dzn
zincite-check [--input-format fzn|fzn-json] [--format text|json] model candidate.dzn
zincite check --input-format fzn-json --export prepared.json model.fzn.json
zincite-check --load [--format text|json] prepared.json candidate.dzn
zincite check --load --float-absolute-tolerance 0 --float-relative-tolerance 0 prepared.json candidate.dzn
```

Both entry points support all these options. `--input-format` defaults to `fzn`;
`fzn-json` selects official compiler JSON for checking or export. `--export`
creates a new prepared artifact and refuses existing paths. `--load` validates
that artifact without parsing FlatZinc; the CLI still parses literal `.dzn`
assignments. `--load` cannot be combined with `--input-format fzn-json` or
`--export`. Reports go to stdout; input and execution errors go to
stderr without partial stdout. Assignments use literal `.dzn` values: Boolean,
checked i64, finite integer sets and finite binary64 floats.

The checker evaluates both pinned standard/Gecode profiles' emitted mathematical
operations, reports every available violation and records independent
completeness. Supported deterministic definitions derive outputs from supplied
inputs. It starts no compiler or solver. External-execution and search-history
hooks receive explicit diagnostics. The [checker integration guide](checker.md)
explains preparation, assignments and both offline compiler profiles. The
[predicate reference](../crates/zincite-check/README.md) gives exact semantics and
limits. An invalid report may also be incomplete; read `completeness` independently
of `status`.

`--float-absolute-tolerance NUMBER` and `--float-relative-tolerance NUMBER` accept
finite nonnegative settings. Each option changes only its named component; a
loaded model supplies the other saved component. Defaults are `1e-9` each;
zero/zero selects exact checking. Export saves both settings. Prepared artifacts,
Wasm assignments and reports encode all mathematical integers as exact decimal
strings, including set endpoints and application payloads. These commands are
available through `zincite check` and the standalone `zincite-check` package.

## Explicit writes

Formatter `--write` and query `--write` require explicit regular files and
refuse symlinks. Lint uses `--fix` with its own [fix eligibility and write
restrictions](linting.md#suppression-and-explicit-fixes). Formatter `--check` and
`--write` are mutually exclusive; query `--diff` and `--write` are mutually
exclusive. A failing formatter input is not written; other independent files
continue. Query write validates the complete candidate and checks the original
bytes again before replacing the file. Writes retain file permissions.

## Exit codes

| Code | Formatter | Linter | Query | Checker |
| --- | --- | --- | --- | --- |
| `0` | Formatting succeeded; check found no changes | No unsuppressed warnings or errors | Complete result | Valid, complete candidate or successful export |
| `1` | Check found formatting changes | Unsuppressed warnings | Incomplete semantic report or data candidate | Invalid or incomplete report |
| `2` | Input, syntax, configuration, directive, usage or I/O error | Input, syntax, settings, directive, usage, dependency or I/O error | Query, input, model, usage or I/O error | Usage, I/O, preparation, loading, assignment or arithmetic error |

Errors take precedence across formatter/linter inputs. A query's empty fragment
selection succeeds with no source; its count is 0. Lint limitations alone retain
status 0 or 1. Status 0 does not establish complete semantic coverage.
