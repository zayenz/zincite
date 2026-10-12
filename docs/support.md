# Support and analysis limits

Zincite targets MiniZinc 2.10.1 `.mzn` models and assignment-only `.dzn` data.
Read the [syntax support reference](reference.md#syntax-support) for accepted
constructs and explicit exclusions. JSON data and solver output formats are
outside the supported inputs. Code and literals use UTF-8; opaque comment bytes
are preserved.

## Interpret a result

| Result | Meaning for the reader |
| --- | --- |
| Implemented behavior | The command or library provides the operation described in its reference. Coverage still follows that operation's stated boundaries. |
| Supported Unknown | Analysis admits a construct but cannot prove a stronger fact. Unknown proves no bound, member, safe edit or search coverage. It can complete without a lint finding. |
| Located limitation | A required fact, dependency or operation is unavailable or unsupported. Read the location and reason rather than treating the absence of a warning as proof. |
| Input failure | Malformed syntax, invalid settings, misplaced directives or unreadable input prevents the requested operation. The command reports an error. |
| Future work | Plans describe intended behavior and supply no current command guarantee. |

Semantic query reports keep completeness and limitations with available results;
[incomplete reports exit 1](cli.md#exit-codes). Lint has
[per-rule analysis outcomes](linting.md) and can report limitations with exit 0
or 1. Its [definitions and domain advice](linting.md#definitions-and-domain-advice)
describes current supported Unknown boundaries. These two reporting contracts
serve different consumers; inspect both the diagnostics and the status.

Parsing checks syntax without resolving includes or checking compiler semantics.
Formatting preserves source tokens and does not assess solver performance.
Semantic lint and queries load explicit model/include context for bounded facts.
They do not run a solver, evaluate arbitrary MiniZinc expressions or establish
feasibility. Data transformations change the instance and establish syntax only.

## Performance

The [format-on-save guide](format-on-save.md#measured-behavior) links the current
bounded checkpoint and historical results separately. The current default dense
case misses the 100 ms p95 budget; overall interactive acceptance remains open.
Named latency and memory checks do not establish full-corpus performance.

## Evidence and future work

The [corpus acceptance report](../scripts/corpus-acceptance.md),
[lint expansion checkpoint](../scripts/lint-expansion-checkpoint.md) and
[query acceptance report](../scripts/query-acceptance.md) retain checked revisions
and their remaining limits. Historical reports do not prove behavior of a later
build. The [developer scripts](../scripts/README.md) document how to reproduce
checks.

The [semantic processing plan](semantic-processing-plan.md) and
[callable analysis plan](callable-analysis-plan.md) are developer design material.
The [area brief](../.zdev/base/brief.md) and task records track intended and
unfinished work. An open semantic task is not an implemented capability.
