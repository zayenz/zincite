# MiniZinc tooling foundations

## Objective

Build source-preserving MiniZinc tools that process the complete published MiniZinc
Challenge corpus and the models and data under `~/minizinc/`, and offer all fourteen
lint rules in Erik Rimskog's thesis alongside Zincite's existing rules. Formatting
on save must be fast and economical enough to be unremarkable in normal editing.

Zincite will provide MiniZinc development tools, starting with a formatter
(`zincite-fmt`) and a static analyser (`zincite-lint`). The `base` area establishes the
shared parsing and source representation and the smallest useful path to both
tools. The first 21 tasks delivered the syntax foundation and usable formatter
and linter commands. The next bundle establishes corpus compatibility and expands
linting to the full thesis catalogue. The contracts below describe the existing
baseline; the Corpus and thesis expansion section explicitly extends them.

## Settled decisions

- Implement in Rust, organised as a Cargo workspace with reusable crates.
- Share a concrete syntax tree (CST) between formatting and linting. Keep source
  spelling, comments, whitespace, and source locations available to consumers.
- Use `zincite` as the project name and crate prefix, with executable names
  `zincite-fmt` and `zincite-lint`.
- Use zdev project records. `base` follows configured trunk `main`, as requested.
- The current checkout directory is `zincite`.
- Build Zincite's own Rust parser, CST, formatter, and linter. Keep the project
  independent of Shackle: use it as a research and design reference only, with
  no Shackle code reuse, crate dependencies, or planned integration.
- Prefer simple modules and direct tree traversal. Add crates, generic interfaces,
  semantic machinery, and tests when an actual consumer or risk justifies them.
- Target MiniZinc 2.10.1, including `.mzn` models and `.dzn` data files.
- Retain the existing naming and constraint-label rules as defaults. Add all
  fourteen thesis rules through the explicitly selected `--rules thesis` preset.
  Build name resolution and semantic facts only as those rules need them.
- Accept explicit file paths and stdin. Extend linting and formatter check mode
  to directories in the next bundle. Provide formatter stdout, check and explicit
  file-write modes, with text diagnostics. JSON output remains deferred.

## Implementation direction

Build a MiniZinc parser in Rust, informed by Zirium's source ownership and recovery
design. Own the source-preserving CST and expose it to both tools. Shackle's
grammar and tooling can inform design questions and edge cases, but are not
implementation candidates.

Start with `zincite-syntax`, `zincite-fmt`, and `zincite-lint` as candidate boundaries.
The syntax crate can initially contain source storage, parsing, typed views, and
diagnostics; separate crates for each are not prerequisites. Formatting and
linting should call library APIs, with thin command-line entry points. Add name
resolution and types only as required by the selected lint rules.

Observable success for the eventual foundation:

- A documented MiniZinc language baseline and explicit handling of unsupported
  syntax and malformed input.
- A shared parse result that retains the original source and exposes useful
  syntax, precise ranges, and diagnostics without requiring compilation or solving.
- Formatting preserves comments and meaning and reaches a stable result on a
  second pass for the supported syntax.
- Initial lint diagnostics identify a rule and source range, explain the concern,
  and distinguish proven facts from modelling advice.

## Boundaries

This is source tooling, not a new solver or a replacement MiniZinc compiler.
Full type checking, flattening, language-server support, Python bindings,
incremental reparsing, a query language, and automatic semantic rewrites are not
initial requirements. Do not copy Zirium's MLIR dialect machinery. The expanded
scope includes the thesis's entire fourteen-rule catalogue, with conservative
analysis and advisory wording where a conclusion is heuristic. Do not promise
solver speedups from syntactic rewrites. A source-preserving parse and a formatter are different operations.

## Formatting direction

Prefer formatting that keeps later Git diffs small. Use a default maximum line
length of 120, with most lines shorter; line length is one of very few configurable
style properties. Honor `.editorconfig` indentation style/size, tab width, line
endings, and the `max_line_length` extension. Explicit command-line options take
precedence. Default to four-space indentation and a 120-column limit.

Always expand `forall` bodies into block layout, including a single short
expression. Preserve explicitly multiline comma-separated lists even when they
would fit on one line; normalise them to one item per line with consistent
indentation. This intentionally allows compact and expanded layouts for equivalent
lists. Other generator calls, including `exists` and numeric `sum`, may stay
inline when short; retain their explicitly expanded bodies.

Preserve comment text, existing line breaks, and internal spacing, including
diagrams and section dividers; adjust surrounding indentation without reflowing
prose. Wrap surrounding code where possible, but allow unavoidable width overruns
for indivisible identifiers, string literals, URLs, and preserved comments without
a formatting warning or failure.

Format two-dimensional literals with one logical row per line and aligned columns.
This deliberately prioritises readable matrices over the smaller diffs that
unaligned cells would give. A change in cell width may realign other rows. When
aligned rows exceed the width limit, wrap every row at the same column boundaries,
preserving alignment and logical row boundaries. Indivisible cells retain the
ordinary unavoidable-overrun exception.

Always break after a constraint's string label, including for a short scalar
expression. Start the expression at the same indentation as `constraint`.

Use `snake_case` for parameters, variables, arrays, fields, functions, predicates,
and derived values. Use `UpperCamelCase` for enum/type names, enum constructors,
and named sets/domains. Never use `SHOUTY_CASE`. These conventions come from the
user's MiniZinc style skill. Naming checks belong in
`zincite-lint`; the formatter preserves identifiers.

The user's explicit constraint layouts override the skill's indentation example:

```minizinc
constraint :: "Some explanation"
forall (foo in Foo) (
    some_constraint(foo) /\
    some_other_constraint(foo)
);

constraint forall (foo in Foo) (
    some_constraint(foo) /\
    some_other_constraint(foo)
);
```

The labelled expression starts at the constraint's indentation level. The unlabelled
form keeps `constraint forall` together. The examples use a space before generator
arguments, no space before ordinary call arguments, and trailing conjunction
operators. Blank lines
between every source line in the chat are treated as message formatting, not a
request for double-spaced code.

The skill's modelling advice (constraint labels, input assertions, tight domains,
semantic wrappers, and global constraints) informs linting. Format existing code
without inventing labels, renaming symbols, inserting includes, or rewriting the
model. Sorting existing includes is explicitly in scope; grouping and comment
attachment follow these rules: sort adjacent include groups by path, with
`globals.mzn` first within its group. Move attached comments with their include.
Do not move includes across blank-line or section boundaries. Preserve other item
order and never insert an include.

For a file with syntax errors, report the errors and leave the entire file
untouched; formatting fails for that file. Support both a skip-next-item comment
and paired formatting-off/on comments. Off/on regions must start and end between
complete top-level items. Preserve skipped items verbatim. Nested or unmatched
markers, or boundaries inside expressions, are errors that leave the file
untouched. Skipping does not exempt source from syntax checking. Use standalone `% zincite-fmt: skip`, `% zincite-fmt: off`, and
`% zincite-fmt: on` comments. A skip without a following item is also an error.

Keep short generator headers compact. When a header exceeds the width limit, use
block indentation with one generator per line and each `where` filter on a following
line indented beneath its generator. Preserve generator/filter order and attachment.

Read [formatting background](background/formatting.md)
for evidence, recommendations, and the remaining policy questions.

## Language and command-line contract

The [MiniZinc 2.10.1 specification](https://docs.minizinc.dev/en/2.10.1/spec.html)
is the syntax authority. Cover its model and data syntax by the end of this
bundle, including structured types, interpolation, annotations and comprehensions.
Intermediate tasks may support a documented subset but must diagnose unsupported
input rather than silently accept it as opaque valid syntax. Parse individual
UTF-8 files without resolving includes, requiring a solve item, evaluating data,
or performing compiler semantic checks. Reject invalid UTF-8 without writing.
Treat `.dzn` as data input; do not pretend syntax-only checks establish the
semantic validity of data expressions.

Both tools accept explicit files or `-` for stdin; no input means stdin.
`--stdin-filepath` supplies the language mode and EditorConfig lookup path for
stdin; without it use model mode and no filesystem configuration lookup.
Reject mixed stdin and file inputs. Parsing an individual file never resolves
includes. The expansion below adds directory discovery and semantic lint include
loading without changing single-file parser behavior.

`zincite-fmt` defaults to stdout for one input. Require `--check` or `--write`
for multiple files. These modes are mutually exclusive; `--write` requires file
paths. `--check` writes nothing and exits 1 when any file would change. Successful
formatting exits 0; syntax, directive, configuration, usage or I/O errors exit 2
and take precedence over exit 1. Process independent files even if one fails;
never write a failing file. Complete each file's formatting before replacing it,
avoid truncation on failure, and retain its permissions. Symlink inputs may be
read, but refuse in-place writes through symlinks in this first version.

Expose explicit CLI overrides for indentation style, indentation size, tab width,
line endings and maximum line length, with help documenting accepted values.
Resolve EditorConfig according to its specification, including parent lookup,
section precedence and `unset`. Support `insert_final_newline` and
`trim_trailing_whitespace` on editable layout as well as the agreed indentation,
line-ending and width properties. Use UTF-8 input/output only; support a UTF-8 BOM
when requested and reject incompatible charset settings. Default to LF and one
final newline. Preserved comments, literal contents and skipped spans take
precedence over newline conversion, trimming and final-newline settings.
Unknown EditorConfig properties are ignored; invalid values of supported
properties report a configuration error. Support `max_line_length = off`.

Both tools report text diagnostics with path, line/column, precise byte range,
and a readable message. Lint diagnostics also identify the rule and severity.
Keep stdout for formatted source in the formatter; use stderr for its diagnostics.
Lint reports diagnostics on stderr and does not rewrite files. It exits 0 when
clean, 1 for unsuppressed warnings, and 2 for input, parse, directive or usage
errors. On parse errors, omit lint rules for that file and report syntax errors.

## Remaining layout defaults

These defaults complete the agreed formatting direction without adding style
configuration. Count Unicode scalar values as one column and expand tabs to the
next configured tab stop. Preserve literal spelling, parentheses and annotation
attachment; put broken binary operators at the end of the preceding line. Do not
add synthetic operands or semantic rewrites. Use trailing commas in expanded
lists only where permitted by the pinned grammar. Preserve blank-line group
boundaries while collapsing multiple blank layout lines to one outside protected
text. Keep end-of-line comments attached to their preceding code.

Within an include group, compare decoded include paths case-sensitively, with
`globals.mzn` first and stable ordering for equal paths. A directly preceding
standalone comment block without an intervening blank line belongs to the next
include. A separated comment is a group boundary. Formatting and lint suppression
directives and skipped spans are sorting barriers, so sorting cannot change the
item a directive controls. Preserved skipped regions include their original
whitespace; adjacent formatting must not swallow or duplicate it.

## Initial lint contract

Rule IDs are `naming` and `missing-constraint-label`. Both are warnings enabled by
default; unsuppressed warnings fail the command. Naming checks declarations and
bindings whose role is clear from syntax: ordinary values, callable names,
parameters, fields and generator bindings use snake_case; enum/type names,
constructors and explicitly declared parameter sets/domains use UpperCamelCase.
Set-valued decision variables use snake_case. Do not infer an identifier's role
from its uses or resolve type aliases for this first rule; skip classifications
that require semantic information. Do not check references or standalone `.dzn`
assignment targets against an unavailable declaration. Exempt anonymous `_`,
quoted identifiers and a single leading underscore used for an intentionally
unused binding; check the remaining spelling of that binding normally.

A constraint has a label when it has a direct string annotation in the constraint
header. Other expression annotations or strings inside a wrapper do not count.
The missing-label message is modelling advice, not a claim that the model is
incorrect or slower.

A standalone `% zincite-lint: ignore naming` or
`% zincite-lint: ignore missing-constraint-label` comment suppresses that rule
throughout the next top-level item, including nested bindings. Consecutive comments
may suppress both rules. Unknown rule IDs, malformed directives, misplaced
non-top-level directives and a directive without a following item are errors.
Do not add automatic fixes or a general suppression/configuration framework.

## Corpus and thesis expansion

This section extends the original single-file, syntax-only bundle. It is the
shared contract for the next tasks; completed task records remain historical.

### Coverage and acceptance

- Use the official [Challenge archive](https://github.com/MiniZinc/mzn-challenge),
  covering every published year (2008–2026 at planning time), all `.mzn` files,
  and all available `.dzn` instances. Keep the checkout external to this repository.
  Record the checked revision and per-year counts in each acceptance report; an
  absent year or unavailable instance is a coverage gap, not a passing check.
- Inventory every `.mzn` and `.dzn` under `~/minizinc/`, including ignored files,
  nested repositories, archive and software trees. Classify compiler negative
  tests, malformed experiments, vendored/generated duplicates and encoding errors
  explicitly. Do not exclude a failing valid model to improve the pass count.
  Local files stay read-only and private; reports use relative paths and summaries.
  Do not copy confidential models into fixtures or published planning records.
- Processing means lossless parsing, stable source-preserving formatting and lint
  execution without crashes or hangs. Valid target files must parse and format;
  deliberately invalid files must produce diagnostics and remain untouched.
  A warning is a successful lint run, not a failed corpus check. Zero warnings
  on third-party models is not a completion condition.
- MiniZinc 2.10.1 remains the baseline. Accept demonstrated compiler-supported
  source forms needed by the corpus even where the printed grammar is narrower;
  document each extension and retain meaningful CST nodes. Historical Challenge
  syntax belongs in compatibility work. A future extension cannot silently pass
  as opaque syntax. Keep `.dzn` assignment-only by default; declarations in a
  mislabelled data file are diagnosed, not silently reinterpreted as model code.
- Check the full corpus with a small explicit developer runner, not a benchmark
  platform. It must enumerate all inputs, continue after individual failures,
  distinguish parse/format/lint/compiler outcomes and account for unavailable
  dependencies. Use temporary outputs; never format the external originals.
  Compare token spellings, comments and structure as well as second-pass output;
  allow only the existing include sorting and permitted layout punctuation changes.
- Compiler checks supplement preservation checks. Exercise complete model/data
  pairs and include trees, with the formatted include tree staged together.
  Do not guess pairs by taking a Cartesian product of nearby files. Use known
  project invocations or simple explicit entries, report missing pairings, and
  do not require solving. Historical compiler incompatibilities remain visible.

### Commands and rule selection

Add deterministic recursive directory inputs for lint and formatter check mode.
Discover `.mzn`/`.dzn`, include ignored source files, skip Git metadata, avoid
symlink-directory cycles and deduplicate overlapping inputs. Do not recursively
write directories in this bundle. Explicit file writes retain their current
contract. Reading included files for semantic linting never formats them.

`--rules default` selects the current two rules; omitted `--rules` is equivalent.
`--rules thesis` selects exactly the fourteen thesis rules, and `--rules all`
selects both groups. A comma-separated list of stable rule IDs selects exactly
those rules. Reject unknown IDs and repeated `--rules`; keep the existing text
output and 0/1/2 exit semantics. Extend existing next-item suppressions to every
rule ID, including rules currently disabled. No plugin system, automatic fixes,
JSON output or general configuration framework is required.

### Semantic analysis

Keep the parser and formatter independent of semantic linting. Retain
`lint(&ParsedFile)` with its current defaults; expose selected rules and model
analysis through library APIs used by the CLI. Start with direct traversal and
small modules in `zincite-lint`; a new semantic crate needs an actual second
consumer. Do not build a generic AST query engine.

When selected rules need it, analyse each positional `.mzn` as its own root with
its include closure. Resolve relative includes from the including file, then
ordered `-I` directories, then an explicitly configured `--stdlib-dir` (or
`MZN_STDLIB_DIR`). Identify builtins and standard-library declarations separately
from user code. System library files supply facts but receive no user-style
warnings unless explicitly selected as inputs. Cache within one invocation only
when this avoids repeated parsing. Includes, diagnostics and suppressions retain
file identity and byte ranges. Missing/cyclic includes and unreadable dependencies
produce useful errors without aborting unrelated roots. Do not execute a compiler
or solver to provide ordinary lint results.

Lexical scopes, aliases, enums, callable overloads and `par`/`var` expression facts
must use declaration identity. Unresolved or ambiguous facts stay unknown; they
must never become guessed parameter values, fabricated domains or safe rewrite
claims. Implement the necessary subset of typing, not a second validating compiler.
A rule skipped because required facts are unavailable must be distinguishable
from a completed rule with no findings in the library result and corpus report;
report a concise CLI analysis limitation. It does not constitute full semantic
coverage at the final acceptance gate. Selected-analysis results separate findings,
per-rule execution/applicability/limitations, and actual errors. Render limitations
on stderr; retain exit 0 with no warnings, 1 with unsuppressed warnings, and 2
for input, parse, directive, usage or dependency errors, with 2 taking precedence.
A limitation alone does not change 0/1, but the corpus report cannot count it as
complete analysis. Known symbolic parameters need no supplied value: a rule can
complete and decline an unprovable candidate. Missing implementation support is
an analysis limitation, not an ordinary negative result.
Standalone `.dzn` gets syntax checks and applicable syntax rules, not invented
model bindings. Model analysis does not require data values; explicit model/data
pairs are used for compiler validation, not instance-specific rewrite proofs.

The thesis's unused-declaration analysis requires a complete model root. Do not
label exported declarations unused when linting an include-only fragment without
a solve item. Account for implicit/default output and annotation references.
Mark this rule inapplicable to such fragments, rather than calling missing
semantic information in a complete model inapplicable.

Read [the rule coverage contract](background/thesis-rule-coverage.md) before
implementing any thesis rule. All fourteen must have working library/CLI paths,
source locations, suppressions and focused positive/negative checks. Conservative
unknown results are allowed where static proof is impossible, but cannot be used
to leave a rule unimplemented. Advice must explain uncertainty and must not claim
that fewer constraints or a rewrite guarantees faster solving.

### Completion evidence

Keep focused unit/integration coverage: small counterexamples for scope,
shadowing, overloads, unknown parameters, reification and partial array coverage.
Add only regressions that expose a real corpus failure, using minimal synthetic
examples where possible. Run the whole external corpus at integration milestones
and final acceptance, not in every `cargo test`. Report totals, failure categories,
rule coverage and analysis limitations separately. Final acceptance requires no
unresolved Zincite failures on valid target syntax, formatter stability and
preservation on all valid inputs, and execution of all applicable thesis rules on
complete model roots with resolved dependencies. All exclusions need a reason;
missing corpora or unavailable model dependencies prevent a full-coverage claim.

## Performance and format-on-save

Speed and memory are required outcomes for both tools. The formatter's primary
interactive path is a fresh `zincite-fmt --stdin-filepath PATH` process receiving
the editor buffer on stdin and returning complete formatted source on stdout.
The editor replaces its buffer only on success; syntax/configuration/directive
errors leave it intact. Saving does not run lint rules, resolve includes, invoke
MiniZinc, or scan the workspace. Existing no-partial-output behavior remains.

Use these proposed acceptance budgets on the user's M1 Max with a release build:

| Representative edited model/data size | End-to-end p95 | Peak child RSS |
| --- | ---: | ---: |
| Up to 100 KiB | 50 ms | 32 MiB |
| Above 100 KiB, up to 1 MiB | 100 ms | 64 MiB |

Latency includes process startup, stdin/stdout transfer, EditorConfig lookup,
parsing and formatting. Measure repeated saves with a fresh process each time
and ordinary warm filesystem caches, including changed buffers and already
formatted buffers. Record first invocation/first-use separately; do not describe
it as cold filesystem I/O unless that was actually controlled. Report its cost
and investigate material delays. Do not substitute a library microbenchmark or
persistent-result-cache hit for the command budget.

The benchmark baseline task selects a small named case set from the real corpus
covering ordinary models, dense data, matrices, comments and nested expressions,
with exact sizes and a few synthetic growing families. Add newly discovered slow
shapes to that set instead of excluding them. Use at least 50 measured save
samples per small case for reported p95, reporting sample count and spread. The
budgets apply to each representative case, not a percentile pooled across files.
Include invalid edited buffers in responsiveness checks while retaining their
error status. Inputs over 1 MiB stay fully supported: measure representative
large cases and growth, report throughput and peak RSS, and require no unexplained
superlinear growth or unbounded accumulation across independent files. Establish
CPU/allocation attribution with a profiler when deciding a particular fix; RSS
alone is not an allocation count.

Keep a small self-contained performance driver and case description; reuse corpus
input discovery where useful. Record release build, hardware, commands, input
sizes and measurements sufficient to repeat a comparison. Keep external sources
read-only and local reports untracked by default. Do not build a benchmark service
or assert noisy wall-clock thresholds in `cargo test`. Performance changes need
before/after release measurements and the existing source-preservation, comment,
formatting-stability and diagnostic checks. Recheck after syntax and layout changes.
A confirmed regression or missed budget needs a bounded follow-up; no silent budget
relaxation or skipping the hard cases at final acceptance.

Measure whole-corpus formatter throughput and lint throughput/memory separately
from save latency. Once semantic linting exists, compare `default`, `thesis` and
`all`, including shared includes and many diagnostics, and expose incomplete
analysis in performance reports. Start with sequential processing and reuse of
needed facts within an invocation. Add bounded parallelism, persistent caching,
a daemon, incremental parsing or an LSP only through later evidence-backed work
if simpler changes cannot meet the goal. Format-on-save itself requires none of
those features or a new editor extension.

Read [performance evidence and Astral references](background/performance.md)
before performance work. Use Ruff's uncached responsiveness and shared syntax,
uv's distinct cold/warm measurements and dependency-aware reuse, and ty's selective
computation as inspiration. Keep Zincite's own language, layout and source model.

## License and release boundary

Use `MIT OR Apache-2.0` and include both license texts and Cargo metadata.
Document local Cargo installation and command usage. Publishing packages,
release automation, hosted CI and binary distribution are outside this bundle.
Before a stable release, formatting may change between versions; document this
without adding a versioned style system.

Parser algorithms, tree representation, dependencies and module boundaries are
implementation choices. Choose the simplest design that meets exact source
coverage, useful traversal and recovery. No further product choice is needed to
draft this bundle.

## Testing

Focused coverage: use a small set of behavior checks for implementation: exact source/token
coverage (not merely returning a saved input string), recovery past an error,
formatter idempotence and comment preservation, and positive/negative cases for
each selected lint. Include shadowing or parameter-dependent cases when relevant.
Use the small repeatable benchmark set described above for speed, memory and
scaling; do not expand it into a broad campaign or unit-test timing matrix.
Compiler acceptance can supplement formatting checks; it is not a proof of
semantic equivalence.

## Background

- [Performance and Astral references](background/performance.md): read for save
  latency, memory, batch throughput and measured optimization work.

- [Corpus planning evidence](background/corpus-coverage.md): read when building
  the corpus runner or fixing syntax and formatter compatibility.
- [Thesis rule coverage](background/thesis-rule-coverage.md): the full rule mapping,
  prerequisites and soundness limits for semantic lint tasks.

- [Syntax design references](background/syntax-and-reuse.md): read before designing
  the parser, CST representation, or crate boundaries.
- [Linting and literature](background/linting-and-literature.md): read before
  selecting lint rules, semantic prerequisites, or rewrite claims.
- [Names](background/names.md): naming decision and registry checks; read before
  publishing packages.
- [Formatting](background/formatting.md): read when shaping layout rules,
  EditorConfig support, or the formatter/linter boundary.

## Validation

Run `zdev check base --format json` after changing these records. For code, run
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace`. Keep behavior checks small and aligned with each
task; no test is required solely for scaffolding, metadata or documentation.
Use MiniZinc 2.10.1 acceptance checks on a few complete representative models
and model/data pairs as supplementary evidence; do not require a solver run.
