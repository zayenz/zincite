# Support and analysis limits

Zincite targets MiniZinc 2.10.1 `.mzn` models and assignment-only `.dzn` data.
The [syntax reference](reference.md#syntax-support) lists accepted constructs
and differences between the specification and compiler. JSON data and solver
output are not supported input formats. Code and literals must be UTF-8;
non-UTF-8 bytes in comments are preserved.

## Versions and Rust support

Zincite 0.3.0 supports the current stable Rust toolchain. No separate minimum
Rust version is promised. The command-line tools and library crates share the
same release version. Formatting and interfaces may change in the 0.x series;
review the [release notes](../CHANGELOG.md) when upgrading.

## What each command checks

Parsing checks the written syntax. It does not resolve includes, check types
or require a solve item. This allows the formatter to work on a model fragment,
but a formatted fragment may still fail MiniZinc's type checks.

Linting and semantic queries can read includes and establish facts such as
which declaration a name refers to or which indices an array accepts. They
do not evaluate arbitrary MiniZinc functions or establish that a model has a
solution. Use MiniZinc for compilation and solving.

Data edits change the instance. Zincite reparses the candidate and, for
model-backed construction, checks supported types and array shapes. Those
checks do not establish satisfiability.

## Unknown facts and incomplete analysis

An unknown fact is sometimes the expected result. For example, a parameter
`int: divisor;` may get its value from a data file. A source-only check cannot
assume that it is nonzero. An unknown fact must not be used to justify an
automatic fix or turned into a known fact by negating a query filter.

A limitation means information needed by the analysis was unavailable or an
operation is outside its supported analysis. Missing standard-library context
is one example. Zincite reports a location and reason so you can distinguish
that case from a check that completed without a finding.

| Result | How to read it |
| --- | --- |
| Lint warning | A selected rule found something to review. The rule ID and message explain the concern. |
| Lint limitation | Part of the analysis could not be completed. Use `--show-limitations` for details. |
| Query with `complete: false` | Available results are included, but the `limitations` list explains what is missing. |
| Input or query error | Fix the reported syntax, configuration, dependency or query problem before using the result. |

Lint exits 1 for warnings and 2 for errors. Limitations alone do not change
that status, so exit 0 does not mean all semantic checks completed. Semantic
queries exit 1 for incomplete results and 2 for errors. Keep the completeness
fields with a query result if another program will use it. See the
[exit-code table](cli.md#exit-codes) for all commands.

A missing row in a semantic query is not necessarily a negative answer.
Filters keep only proven matches. If the type of a declaration is unknown,
its absence from a query for integer declarations does not establish that
it has some other type.

## Performance

Format on save uses a fresh formatter process. The project targets less than
100 ms at the 95th percentile for ordinary editing cases; large generated
inputs can take longer. [Performance measurements](../scripts/save-performance-current.md)
live in the repository reports and identify the builds, machines and inputs
tested. See [format on save](format-on-save.md) for editor setup.

## Coverage reports and planned work

The repository keeps [corpus checks](../scripts/corpus-acceptance.md),
[lint coverage](../scripts/lint-expansion-checkpoint.md) and
[query acceptance results](../scripts/query-acceptance.md), with the revisions
and limitations tested. The [developer scripts](../scripts/README.md) explain
how to reproduce those checks.

The [project brief](../.zdev/base/brief.md) and design plans describe unfinished
work as well as current contracts. Use the command guides and references for
implemented behavior; a planned capability is not available merely because
it appears in a design document.
