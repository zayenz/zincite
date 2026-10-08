# MiniZinc tooling foundations

## Objective

Build source-preserving MiniZinc tools that process the complete published MiniZinc
Challenge corpus and the models and data under `~/minizinc/`, and offer all fourteen
lint rules in Erik Rimskog's thesis alongside Zincite's existing rules, then extend
semantic linting with the rule families, configuration and fixes described below. Formatting
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
- Add the root `zincite` command and the query/data-reduction subsystem described
  below. Query inspection may produce JSON; formatter and lint JSON remain deferred.

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
initial requirements. The Query and data transformation expansion below adds a
bounded query language and explicit instance transformations. Do not copy Zirium's MLIR dialect machinery. The expanded
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
files without resolving includes, requiring a solve item, evaluating data,
or performing compiler semantic checks. Code and literals use UTF-8. Accept
non-UTF-8 bytes confined to comments and preserve those bytes exactly, as required
by current compiler-accepted challenge inputs. Reject invalid UTF-8 elsewhere
without writing. Keep original byte coordinates and source bytes available to
library callers; an internal analysis view must not be presented as exact source.
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
line-ending and width properties. Use UTF-8 for code and literals while preserving
opaque comment bytes in output; support a UTF-8 BOM
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
The initial rules are diagnostic-only. The Linter expansion section below adds
bounded configuration and explicit fixes without changing ordinary lint runs.

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
- For corpus acceptance, first check whether the installed current MiniZinc
  system can parse and process the original input. Record its version and build;
  the current system is MiniZinc 2.10.1. Historical Challenge files rejected by
  that system are classified compiler failures, not required Zincite successes.
  Missing data, dependencies or a usable pairing are availability gaps, not
  evidence that the source is broken. Keep every input in the accounting.
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
  do not solve. Pre-check original known instances by compiling to FlatZinc with
  the Gecode backend (`minizinc --compile --solver gecode`), using finite limits
  and temporary output files. Compile the complete formatted instance with the
  same compiler, backend, data and settings. Compare generated FlatZinc bytes
  and record equal outputs or investigate differences; a byte difference alone
  does not prove a semantic change, and normalization must not hide one.
  Model-only checks may establish source validity when data is unavailable,
  but do not establish successful instance compilation. Record compiler,
  backend, unavailable-data and timeout outcomes separately. Historical compiler
  incompatibilities remain visible.

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
rule ID, including rules currently disabled. The Linter expansion section extends
selection, configuration and fixes; a plugin system and JSON output remain deferred.

### Semantic analysis

Keep the parser and formatter independent of semantic linting. Retain
`lint(&ParsedFile)` with its current defaults; expose selected rules and model
analysis through library APIs used by the CLI. Start with direct traversal and
small modules in `zincite-lint`; a new semantic crate needs an actual second
consumer. Keep lint fact production independent of the later query consumer;
do not make lint execution depend on a query engine.

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

## Linter expansion: semantic facts, rule families and fixes

Requested on 2026-09-30. Add the ten proposed detection families: index-set
mismatches, hidden optionality mistakes, partial-expression hazards, suspicious
shadowing, vacuous constraints/conditions, unused generator bindings, recognizable
global-constraint opportunities, expensive comprehensions, missing input
preconditions, and suspicious domains/intermediate bounds. Existing thesis
coverage remains required. The new work has its own integration task; base-050
continues to close the existing corpus/thesis bundle.

### Interpretation before policy

Semantic interpretation returns typed facts keyed by existing source/declaration
identity. Lint rules consume those facts to choose findings; fix producers consume
facts and findings to propose edits; a separate edit application path owns writes.
Keep these responsibilities in small modules in `zincite-lint`, not new crates,
a generic query language, a solver, or a plugin framework. Existing semantic
work (base-032 through base-036 and search analysis) must also keep fact production
separate from diagnostic policy within its implementation. Preserve those tasks;
new infrastructure extends their results rather than rebuilding name resolution.

Add separate infrastructure tasks for bounded numeric interpretation, guarded
truth/definedness, option presence/cardinality, and iteration/dependency facts.
Each has named rule consumers below and can be exercised without enabling a lint.
Known, disproven, unknown and unsupported conclusions remain distinguishable.
Missing parameter data is ordinary uncertainty, not a fabricated value; unsupported
analysis remains an explicit limitation. Analyse only facts required by selected
rules or fixes, once per model context. Rule parameters never change semantic
truth or turn an unknown precondition into a proved one.

Guards follow MiniZinc relational partiality and conditional semantics, not an
imperative short-circuit assumption. Bounds use checked arithmetic and conservative
intervals; unknown does not mean unsafe. Same-sized arrays need not share indices.
Iteration facts distinguish array capacity from present optional elements and
quantifier idempotence from arithmetic multiplicity. No rule may remove or reorder
partial expressions merely because a Boolean identity appears applicable.

### Rule catalogue, families and selection

Keep stable readable IDs, including the existing sixteen. Each rule records a
primary family, a short explanation, required semantic facts, availability and
fix capability (none, sometimes or always). Families are `correctness`,
`suspicious`, `modelling`, `performance`, and `style`; families classify purpose,
not severity or fix safety. Assign a rule once, and document the membership.
Keep current warning severity and exit status policy. Broad heuristic advice stays
opt-in; preserve the two-rule `default` and fourteen-rule `thesis` presets.
`all` expands to every registered rule and still diagnoses unavailable selections.

Selectors accept exact IDs, `family:NAME`, `preset:NAME`, and legacy
`default`/`thesis`/`all`. Configuration uses `select`, `extend-select`, and `ignore`:
expand the base selection, union extensions, then remove ignored rules; ignore
wins within a layer. Unknown or ambiguous selectors are errors. An explicit
`--rules` replaces configured selection and its exclusions, retaining configured
rule options and fix restrictions. Existing comma-separated IDs and exact-ID
next-item suppressions remain compatible; do not add broad suppression directives.
Provide `--list-rules` and `--explain RULE` from the same catalogue so users can
inspect families, availability, caveats, options and conditional fix support.

### Configuration, personal presets and parameters

Use `zincite.toml` with a `[lint]` table. For each positional root, use the nearest
ancestor configuration and apply it to that root's include closure; do not merge
ancestor files. `--config PATH` supplies one explicit file and `--isolated`
disables discovery; reject their combination. Stdin discovers configuration only
through `--stdin-filepath`, unless an explicit file is supplied. Keep formatter
EditorConfig independent. Plain library calls receive settings explicitly and
perform no ambient configuration lookup.

Personal/project presets live under `[lint.presets.NAME]`. Each contains
`select`, optional `extend-select`/`ignore`, and optional typed `options`.
Preset members may name rules, families or built-in presets; custom presets
cannot include other custom presets. Reject reserved names and more than one
custom preset in an effective selection. This deliberately avoids inheritance
and ordering rules for competing parameter bundles. Merge defaults, selected
preset options, then `[lint.options.RULE_ID]` overrides. Validate unknown keys,
types and ranges even for disabled rules; give a path and setting name in errors.

Start with two useful parameters: `suspicious-shadowing.ignore-names` (exact
binding names, default empty) and `expensive-comprehension.max-candidates`
(positive integer, default 1000000). The latter controls only warnings for a
known candidate-count upper bound above the threshold; symbolic estimates remain
advice about structure, not measured runtime. A high upper bound is explicitly a
potential expansion, not proof the compiler enumerates it. Keep options typed
per rule and add parameters only for a concrete decision. Do not expose arbitrary
expressions, a configuration DSL or a generic per-rule options map in core logic.
Resolved settings, expanded IDs and option values are inspectable through
`--show-settings`; record preset membership changes in user documentation.

### Fixes and source edits

Ordinary linting remains read-only. A diagnostic can carry an optional titled
fix consisting of source edits, safety and the established applicability
conditions. Rule-level fix support is not a promise that every finding is fixable.
A safe fix preserves the solution set, objective values, output, definedness,
annotations and comments. It need not preserve solving time or search order.
An unsafe fix has a described behavioural uncertainty and needs explicit
`--unsafe-fixes`; this flag never enables fixes by itself. Unsupported semantic
facts cannot justify a safe fix. Do not invent constraint labels, automatically
rename public symbols or silently delete comments.

`--fix` applies safe fixes to explicit regular files; `--diff` previews eligible
edits without writing. The modes are mutually exclusive. `--unsafe-fixes` expands
eligible fixes only in either mode. `[lint].fixable` (default all) and `unfixable`
(default empty) independently restrict fixes using rule/family selectors, with
unfixable winning; they do not change diagnostic selection or upgrade fix safety.
Reject directory or stdin inputs in fix/diff modes for this first increment;
include-only dependencies and standard-library files are never implicit write
targets. Read-only directory linting remains supported.

Edits refer to the exact original bytes, including BOM offsets. Replacements
use UTF-8; preserve untouched opaque comment bytes. Apply a
finding's edits as one unit. If candidate fixes overlap, omit the conflicting
fixes and explain the conflict; independent fixes may proceed. Reparse each full
candidate before replacing its source, compare original bytes again before
writing, retain permissions and refuse symlinks. Preserve untouched bytes,
comments and line endings. A failing file stays unchanged and independent files
continue. Apply one pass, then reanalyse; report remaining/new findings rather
than iterating silently. In fix mode, exit 0 when the resulting inputs have no
warnings, 1 for remaining warnings, and 2 for actual errors. Diff mode uses the
original diagnostic exit status. Validate mode/configuration errors before any
writes. Suppressed findings never supply edits.

First real fix producers target unused generator names (rename only a proven
unused binding to `_` where syntax permits) and standard `element` calls
(indexing equality only with proven matching types, indices and totality).
Retain multiplicity, guards and annotations. Offer no fix when these obligations
cannot be established; keep advice useful independently. Do not manufacture an unsafe fix merely to exercise the flag: a producer needs
a useful suggested change with a specific, documented risk.

### Evidence and focused validation

Ruff is inspiration for selection, discoverability and edit safety, not a Python
compatibility target. Its established rule-prefix selection and safe/unsafe fixes
are described in [the linter guide](https://docs.astral.sh/ruff/linter/).
Its purpose-based categories are currently documented as preview behaviour;
Zincite's families are its own stable vocabulary. Use
[configuration](https://docs.astral.sh/ruff/configuration/) for precedence and
[settings](https://docs.astral.sh/ruff/settings/) for typed rule options and
independent fix selection. Zincite intentionally retains readable IDs, a single
nearest configuration, flat personal presets and a small set of options.
Ruff's [semantic model](https://github.com/astral-sh/ruff/blob/main/crates/ruff_python_semantic/src/model.rs)
stores bindings, scopes and resolved references; its
[fix representation](https://github.com/astral-sh/ruff/blob/main/crates/ruff_diagnostics/src/fix.rs)
carries applicability with edits. Reuse those responsibility boundaries, not
Ruff's crate structure or Python-specific machinery.

The earlier [lint research](background/linting-and-literature.md) and the
MiniZinc 2.10.1 [option-type documentation](https://docs.minizinc.dev/en/stable/optiontypes.html)
and [effective modelling guidance](https://docs.minizinc.dev/en/stable/efficient.html)
inform optionality, bounds and generators. CPKB's maintained summaries of
*Compiling Conditional Constraints* (Stuckey and Tack, 2019), *Globalizing
Constraint Models* (Leo et al., 2022), and *Solver-Aided Expansion of Loops to Avoid
Generate-and-Test* (Dewally and Akgün, 2025) motivate guarded partiality, narrow
global patterns and expansion-cost advice respectively. These inspire detection
families, not claims that every formulation needs rewriting.

Keep the existing focused testing level. Infrastructure checks demonstrate useful
facts without diagnostics, especially unknown and guarded cases. Rule checks use
a few positive/negative public examples and suppression/selection. Fix checks
cover an actual semantic transformation, comments/annotations, stale or overlapping
edits and failure leaving the original intact. MiniZinc checks supplement explicit
before/after semantic reasoning; compilation alone does not prove equivalence.
Reuse the corpus runner on temporary copies for integration, reconcile limitations
and sample false positives, and measure added lint cost against the existing
baseline. No broad test matrix or new benchmark platform is required.

## Query and data transformation expansion

Agreed on 2026-10-08. Add source inspection and explicit data transformations,
including reducing an enum and its dependent instance data to make smaller cases.
These extend the earlier scope exclusions; formatter and ordinary lint behavior
remain governed by their existing contracts. Existing corpus/thesis acceptance
tasks retain their original boundary and do not acquire query acceptance work.

### Commands and ownership

Add a package named `zincite` at the Cargo workspace root. Its binary dispatches
`fmt`, `lint`, and, when implemented, `query` directly to callable CLI entry points.
Retain `zincite-fmt`, `zincite-lint`, and add `zincite-query`; both invocation forms
use the same argument processing, settings, output and exit semantics. Adapt help
to the invocation. Root help lists implemented subcommands; no subcommand shows
help, while an unknown subcommand is a usage error. Keep formatter stdout as its
default; lint is diagnostic-only unless explicitly asked to fix.

Use a reusable `zincite-query` library with a thin binary and root subcommand.
Traverse the existing CST and consume existing semantic APIs. Depending on
`zincite-lint` for its public model/fact APIs is sufficient initially. Move shared
code only when the new consumer actually needs it; keep lint-fix safety policy
separate from general source-edit validation. No generic backend interface,
plugin system, second parser, compiler or permanent mutable query document.

The query command accepts one query argument and one input file or `-`; omitted
input means stdin. `--stdin-filepath` supplies mode and a diagnostic label, as
for the existing tools. `--model PATH` supplies optional model declarations for
data interpretation; use the existing `-I`/`--stdlib-dir` include rules when a
query requires semantic context. Syntax queries require no library installation.
Model/include files supply facts and are never implicit transformation targets.
Multiple input files, directory inputs and query-program files are deferred.

### Initial query language

Implement one pipeline of fixed stages and bounded predicates. `items` starts
with top-level items in written order; it is the initial selection. `filter`
supports `kind("assignment")`, `name("capacity")`, parentheses, and Boolean
`not`, `and`, `or` in that precedence order. Names compare identifier identity
without changing retained spelling, including quoted names. `head(n)` truncates
the computed selection. `count` returns a number. `emit` reproduces selected
source; a read-only pipeline without an emitter implicitly emits its selection.

Extend this with expression selection, `children`, `subtree`, half-open byte-range
selection within the input, `names`, `call_names`, `annotation_names`, `text`,
`unique`, `tally`, and `json`. Nodes retain file/range identity; navigation
preserves source order and duplicates, while `unique` explicitly removes duplicate
nodes. Counts describe static written occurrences, never execution frequency.
JSON inspection contains kind, available names, file, range and original text;
projected strings, counts and histograms use their corresponding JSON shapes.
This format is inspection output, not a claim of round-trippable MiniZinc data.
Expression fragments need not be complete standalone models.

Treat retained line comments after an item as attached to that item. A preceding
standalone comment block without a blank-line boundary attaches to the following
item. Keep separated section comments in document output; fragment output includes
only selected items and their attached comments. Comments attached to removed
entries disappear with those entries; retained comments and untouched bytes stay
unchanged. Directives must not become attached to a different surviving item:
remove next-item directives with their target and diagnose transformations that
would unbalance paired formatter directives.

Keep results native in the library until rendering. Query errors identify query
byte ranges; input/semantic limitations identify source file/ranges. Bound query
nesting, evaluation work and collection expansion, report exceeded limits, and
never silently truncate results. Reuse Zirium's limit approach without importing
its entire language. Bindings, arbitrary callbacks, fixed-point programs, semantic
diff, Markdown reports and general expression evaluation are deferred.

### Data edits and structured values

`set_value("EXPRESSION")` replaces selected assignment right-hand sides; `remove`
removes selected assignments. Selection does not edit source. An editing pipeline
ends with `emit_document` (also its implicit output), producing the complete
candidate. Allow one transformation stage per pipeline followed by its emitter;
do not evaluate later queries against stale CST ranges. `--diff` previews edits;
`--write` explicitly replaces the single regular file. These modes are mutually
exclusive and writes reject stdin and symlinks.

Use original byte coordinates including BOM offsets, preserve untouched
source, validate overlaps, reparse the complete candidate, compare originals
again before replacing, and preserve file permissions. No partial stdout or file
replacement on parse, query, I/O or candidate-validation errors. Explicit edits
change instance meaning; they must not be advertised as safe lint fixes.

Support a bounded structured view of literal scalar values, enum member names,
sets, records, tuples, arrays, array keys and nested literal values. Preserve the
distinction between a record field label, an enum reference and a string. Expose
record fields, collection elements and keys through query projections. Provide
explicit element filtering with equality and numeric comparisons, including
`filter_elements(gt(0))` for a selected literal array. Compact ordinary list
indices only as part of an explicit filtering transformation; preserve enum keys.
Report unsupported computed expressions rather than guessing their values.

### Enum reduction and best effort

`reduce_enum("Guests", keep("A", "B"))` keeps named members;
`reduce_enum("Guests", keep_first(20))` keeps a deterministic prefix in the
original enum order. Retained names are not renamed. Support explicit member
lists assigned in `.dzn`; constructed/anonymous enum reductions can remain
unsupported with a located explanation. Empty retained selections produce the
corresponding candidate; do not invent minimum counts or modify model constraints.

Reduce dependent data using established membership, keys and model declarations:

- Remove explicitly enum-keyed entries, including whole records, for removed
  members. Model context supplies enum identity and positional-array alignment
  when these are not evident from data syntax alone.
- Remove corresponding slices along every supported enum-indexed array dimension,
  including nested arrays inside records; retain rectangularity and other axes.
- Prune removed members from supported set values recursively. Drop newly empty
  set entries from variable-length group arrays; retain singleton groups. Empty
  record fields or fixed-index array cells cannot be dropped without changing
  their type/coverage obligations, so retain or report them as appropriate.
- Compact supported integer group indices and update supported dependent array
  accesses through an explicit old-to-new mapping. Enum ordinals follow the
  reduced enum declaration. Do not reinterpret arbitrary integers as guest IDs.
- Preserve unrelated enums, records, scalar settings, strings and comments on
  surviving entries. Never use textual name replacement to identify dependencies.

Best effort returns a candidate plus located unresolved dependencies and a clear
complete/incomplete result. Known dangling scalar references, unsupported computed
indices and unavailable required type information stay visible. Do not delete a
whole unrelated assignment or record merely because a required field is unresolved.
Reindexing must not claim completion if dependent references remain unresolved.
Reparse incomplete candidates too. CLI status is 0 for completed queries/edits,
1 for an incomplete best-effort candidate, and 2 for actual errors, with diagnostics
on stderr. Incomplete candidates may be inspected on stdout or as a diff; in-place
replacement requires a complete result. None of these outcomes proves model
satisfiability or full compiler type validity.

Implement data reduction before general semantic query navigation. Later model
queries may expose reference-to-declaration navigation, declaration uses, bounded
transitive reference traversal and available type/instantiation facts. Reuse
binding identity, overload resolution and unknown/unsupported outcomes from the
linter; a negated unknown predicate must not turn uncertainty into a match.
These are static source relationships, not MiniZinc flattening/data-dependency
instrumentation. Ordinary linting does not require data values or query execution.

### References and focused acceptance

Research inspected `mzn-analyse` develop at
`d8de855d5fc52f71bfc44e40a66dba90a2bc0d49` and Zirium at
`00f5cc6c1001cb60ac6dadb980d309e57449a702`. Read
[mzn-analyse's pass dispatch](https://github.com/MiniZinc/mzn-analyse/blob/d8de855d5fc52f71bfc44e40a66dba90a2bc0d49/mzn-analyse.cpp)
for filtering/inspection precedents. Adapt operations, not its compiler AST or
code. FlatZinc instrumentation, objective-term expansion, slackification,
discretisation, model replication and include inlining remain deferred.
Zirium's local `~/projects/zirium/docs/query-language.md` and
`crates/zirium/src/query/{lexer,parser,model}.rs` provide pipeline, predicate,
ordering and diagnostic patterns. Small lexer/parser portions may be adapted
under their license; no Zirium crate dependency, SSA evaluator or dialect machinery.
[Ruff's command split](https://docs.astral.sh/ruff/tutorial/) informs the root
dispatcher; retain Zincite's existing defaults.

The local acceptance model is `~/MiniZinc/models/misc/seating/seating.mzn`.
Use `test_data/small.dzn` and
`instances/mixed/instance-guests-150-topics-10-tables-mixed.dzn` beneath that
directory, with temporary outputs. The model declares an enum-indexed array of
guest records and variable-length arrays of guest sets. Manual temporary
reductions passed MiniZinc 2.10.1 build 33348285743 `--instance-check-only`:
12 to 8 guests/records (same-table groups 2 to 1), and 150 to 110 guests/records
(same-table groups 23 to 22, different-table groups 14 to 13). These are feasibility
controls, not evidence that Zincite already implements reduction.

Keep the area's focused testing level. Add a few synthetic public-behavior checks
for retained source/comments, nested records/sets, keyed and positional alignment,
empty-group cleanup, rectangular axes, reindexing and an incomplete dependency.
Do not copy local guest data into fixtures or publish private sources. Run the
actual commands on the two known model/data pairs and check originals and reduced
candidates with `minizinc --instance-check-only`; solving is unnecessary. Inspect
removed-member absence, key coverage, retained values and ordering directly:
compiler acceptance alone does not establish the requested transformation.
Reuse existing performance scripts for representative growing data and repeat
the existing save checks only when shared syntax/layout code changes. No broad
test matrix, full query corpus campaign or new benchmark platform is required.

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
