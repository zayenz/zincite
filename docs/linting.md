# Linting MiniZinc with Zincite

Ordinary `zincite-lint` analysis reports modelling advice and preserves files.
Explicit `--fix` applies eligible edits as described below. The parser targets
MiniZinc 2.10.1 model and data syntax. Syntax support does not establish complete
semantic validity, model feasibility or solver performance.

```sh
cargo build --release -p zincite-lint
./target/release/zincite-lint model.mzn
./target/release/zincite-lint --list-rules
./target/release/zincite-lint --explain hidden-optionality
./target/release/zincite-lint --rules family:suspicious,partial-expression model.mzn
```

The default selects `naming` and `missing-constraint-label`. `thesis` keeps its
fourteen original rules; `all` selects all 26 currently available rules. IDs,
`family:NAME` and `preset:NAME` can be combined with commas. Expansion keeps the
first occurrence of each ID. `default`, `thesis` and `all` remain accepted legacy
selectors. `--list-rules` gives family, availability, fix support and membership.

Families are `correctness`, `suspicious`, `modelling`, `performance` and `style`.
Selection does not change warnings into errors. Exit status is 0 without warnings
or errors, 1 with unsuppressed warnings, and 2 for usage, settings, input, syntax,
directive or dependency errors. Limitations alone retain status 0 or 1.
Independent input files continue after errors. A rule's `Completed`,
`Inapplicable`, `Limited` or `NotRun` library result describes its analysis;
command status 0 does not promise complete semantic coverage.

## Model context and source preservation

Semantic rules load the root and its include closure. Supply the library root
that matches the model:

```sh
./target/release/zincite-lint --rules all \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  -I project/includes model.mzn
```

`--stdlib-dir` overrides `MZN_STDLIB_DIR`; `-I` adds ordered include directories.
Default source-local rules do not load includes or the standard library.
Standalone `.dzn` files use assignment-only syntax and mark semantic rules
inapplicable. Plain stdin lacks model context; semantic selections report a
limitation. `--stdin-filepath PATH` sets language mode, diagnostic path and the
settings anchor, including for an unsaved file. Directories discover `.mzn` and
`.dzn` recursively, including hidden files, skip `.git`, and deduplicate canonical
paths. Ordinary analysis preserves original bytes, comments, spelling, BOM and
line endings.

## Per-root settings and typed options

Each root uses the nearest ancestor `zincite.toml`. Ancestor settings are not
merged. Includes use their root's settings. `--config PATH` applies one explicit
file to all roots; `--isolated` disables discovery. These switches are mutually
exclusive. Zincite validates every root's settings before analysis or writes.
`--show-settings` prints effective settings without reading model or stdin bytes.

```toml
[lint]
select = ["preset:review"]
extend-select = ["index-set-mismatch"]
ignore = ["naming"]
fixable = ["unused-generator-binding", "element-predicate"]
unfixable = ["element-predicate"]

[lint.presets.review]
select = ["family:suspicious", "family:performance"]
ignore = ["decision-variable-operator"]

[lint.presets.review.options.expensive-comprehension]
max-candidates = 50000

[lint.options.expensive-comprehension]
max-candidates = 25000

[lint.options.suspicious-shadowing]
ignore-names = ["i", "j"]
```

Absent `select` uses default; `select = []` selects none. Zincite expands the
selection, extends it, then removes ignored IDs. Personal presets are flat and
cannot include another personal preset. Options merge defaults, the selected
personal preset, then root overrides. `max-candidates` must be a positive integer
and defaults to 1,000,000; threshold advice requires a known upper bound strictly
above it. Symbolic structure advice does not establish a threshold exceedance. `ignore-names` contains exact binding names and defaults to empty.

`--rules` replaces configured selection, extensions and exclusions after
validating the configuration. It retains effective typed options and fix
restrictions. Choosing a personal preset selects its option bundle. `fixable`
and `unfixable` restrict edits independently of diagnostics; exclusions win.

## The ten added rules

| ID | Family | Supported advice and important boundary |
| --- | --- | --- |
| `index-set-mismatch` | correctness | Written index ranges, offsets and enum/dimension identities incompatible with declared index sets. Candidate traversal is distinct from an exact bad index; no executed-value or runtime-error claim. |
| `hidden-optionality` | suspicious | Core length comparisons consuming a decision-filtered/membership comprehension, including supported aliases, where slot capacity can differ from present values. Capacity use and all-present/empty/inactive cases stay quiet. |
| `partial-expression` | suspicious | Proved failed nonzero/index/nonempty/presence obligations or demonstrated incompatible candidate indices. Unknown-only requirements stay quiet; guards, assertions, defaults and nearest Boolean context retain their distinct meanings. |
| `suspicious-shadowing` | suspicious | A binding concealing a visible enclosing value binding, with exact exceptions. Disjoint scopes and ambiguous identity do not supply a guessed rename. |
| `vacuous-constraint` | suspicious | Proved empty quantifiers, rejecting filters, tautological constraints and contradictory conditions. Optional present counts and upstream partiality/assertion abort remain explicit; no model-feasibility or removal advice. |
| `unused-generator-binding` | modelling | A membership name unused throughout its valid scope. Evaluation, multiplicity and later-generator/filter/annotation uses remain relevant; only supported comprehension forms can be fixed. |
| `global-constraint-opportunity` | modelling | Complete pairwise disequality or matching whole-array occurrence bounds with actual coverage, types and totality. Advice names `all_different` or open four-argument `global_cardinality`; no speedup or automatic edit. |
| `expensive-comprehension` | performance | Known candidate upper bounds and symbolic ordered dimensions. Earlier-prefix advice additionally needs retained guards, raw totality, nonempty crossed domains and safe evaluation. No runtime estimate, hoisting fix or count invented from symbolic data. |
| `missing-input-precondition` | modelling | Supported general callable-body operations requiring matching array dimensions/membership, nonempty present values, nonzero division/modulus or positive integer-input logarithms. Defaults/global assumptions do not validate formal inputs; opaque meaning stays unsupported. |
| `suspicious-domain` | suspicious | Complete integer RHS contracts against explicit scalar destinations. Exact/disjoint contradictions differ from conservative overlapping interval advice. Sparse members, immutable locals and enforced equality retain identity; defaults, unknowns and overflow prove no conflict. |

Rules can report located limitations for unsupported required facts. Zincite does
not run a solver or infer arbitrary callee meaning, symbolic algebra, feasibility
or optimization benefit. Current installed-library and corpus limitations are
recorded in [the expansion checkpoint](../scripts/lint-expansion-checkpoint.md).

## Suppression and explicit fixes

An exact standalone directive suppresses its own rule throughout the next
complete top-level item:

```minizinc
% zincite-lint: ignore unused-generator-binding
int: total = sum([2 | unused in 1..3]);
```

Consecutive directives suppress IDs independently. Unknown, misplaced, dangling
or malformed directives are errors. Joint indexing/partial-expression and
callable-precondition selections omit only the corresponding already emitted,
unsuppressed hazard. Other dimensions and nested hazards remain reportable.

```sh
./target/release/zincite-lint --rules unused-generator-binding,element-predicate \
  --stdlib-dir "$MZN_STDLIB_DIR" --diff model.mzn
./target/release/zincite-lint --rules unused-generator-binding,element-predicate \
  --stdlib-dir "$MZN_STDLIB_DIR" --fix model.mzn
```

These two rules advertise conditional Safe fixes. Unused names become `_` only in
supported array/set/indexed comprehensions; generator-call shorthand remains
advice-only. An element call becomes a parenthesized equality/index access only
with resolved standard identity, complete equivalent body, compatible types and
proved scoped membership/raw totality. Unsupported annotation effects, user
lookalikes and unproved facts withhold the edit.

`--diff` reports original diagnostics and previews eligible groups without
writes. `--fix` applies one pass to explicit regular user files, then reanalyses
all roots. Directories, symlinks, stdin and include-only/system files cannot be
written. Conflicting atomic groups are all omitted; independent groups survive.
The full candidate must parse, and the original regular file must still match
before replacement. Safe eligibility describes the supplied transformation;
`--unsafe-fixes` explicitly admits groups with a stated reason. No current rule
supplies an Unsafe transformation. Never infer unconditional fix availability
from a diagnostic or from `Sometimes` metadata.

## Library use

`LintSettings::from_toml` parses explicit text without filesystem lookup.
`resolve` produces `LintOptions`; `resolve_selection` performs the CLI replacement
contract, and `resolve_fixes` independently resolves `FixOptions`. Call
`load_model` with `ModelOptions`, then `analyze_model`. Public typed fact producers
can also be used independently of lint selection. The model analysis pipeline
builds each selected shared fact layer once per root.

`prepare_fixes` takes original `SourceSnapshot` bytes, supplied findings and fix
options, and returns a full candidate plus group metadata for inspection.
`replace_fixed_file` performs the explicit regular-file replacement contract;
`write_fix_diff` previews. Diagnostic locations and fix edit coordinates remain
original-file coordinates, including BOM; callers must not shift them twice.
The library does not discover CLI settings or write files during analysis.
