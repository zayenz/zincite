# Linting MiniZinc with Zincite

Use linting to review a model's conventions and look for concerns such as
mismatched indices, shadowed names or operations such as division by zero. Each finding names
a rule and points to a source location. Ordinary runs leave the files alone;
explicit `--fix` applies eligible edits as described below.

After [installation](getting-started.md), start with a model you know:

```sh
zincite lint model.mzn
zincite lint --list-rules
zincite lint --explain hidden-optionality
zincite lint --rules preset:arrays,preset:domains model.mzn
```

The [staffing walkthrough](first-model.md#linting-constraint-labels)
shows a small example with two label warnings and a revision that addresses
them. Passing those checks does not establish that the model has a solution.

## Choose the checks

The default checks naming, constraint labels, array-construction wrappers,
index compatibility, shadowing and partial expressions. Their rule IDs are
`naming`, `missing-constraint-label`, `prefer-array-comprehension`,
`index-set-mismatch`, `suspicious-shadowing` and `partial-expression`.

Use `--rules` to select individual IDs or a preset. For example,
`--rules naming,missing-constraint-label` runs only the two checks in the
staffing walkthrough. `--rules preset:arrays` selects the array-related checks
below. You can combine presets and IDs with commas; a rule selected twice runs
only once.

| Preset | Rules |
| --- | --- |
| [`conventions`](lint-conventions.md) | `naming`, `missing-constraint-label`, `array-index-start` |
| [`scope`](lint-scope.md) | `suspicious-shadowing`, `unused-declaration`, `unused-generator-binding`, `global-variable-in-function` |
| [`arrays`](lint-arrays.md) | `index-set-mismatch`, `hidden-optionality`, `prefer-array-comprehension` |
| [`domains`](lint-domains.md) | `unbounded-variable`, `suspicious-domain`, `partial-expression`, `missing-input-precondition` |
| [`formulation`](lint-formulation.md) | `constant-variable`, `compact-if`, `effective-zero-one`, `element-predicate`, `global-constraint-opportunity`, `prefer-enum`, `vacuous-constraint` |
| [`flattening`](lint-flattening.md) | `expensive-comprehension`, `decision-variable-operator`, `decision-variable-generator`, `decision-variable-condition`, `reified-global` |
| [`search`](lint-search.md) | `search-coverage`, `unmarked-symmetry-breaking` |

`--rules default` selects the default rules, `--rules thesis` selects the
fourteen rules based on Erik Rimskog's thesis, and `--rules all` selects all
28 rules. The presets above organise them by modelling concern. Use
`--list-rules` to see their membership and `--explain RULE` for a rule's
meaning and limits.

## Reading diagnostics

A warning includes the rule ID, source path and location. For an included file,
`[root PATH]` identifies the model being analysed: the same include can have
different findings when used by different models.

CLI diagnostics show a source excerpt with the affected range underlined, using
`annotate-snippets`. Excerpts come from the analysed input, including stdin and
included files; diagnostics without available source retain their text location.

Warnings produce exit status 1. Input or configuration errors produce 2.
Informational advice alone produces 0. For example, `expensive-comprehension`
can point out a product of symbolic generator sizes without claiming that it
exceeds a numeric threshold or will be slow in practice.

Analysis can also be incomplete. Zincite prints the number of limitations for
each affected model; use `--show-limitations` to read their locations and
reasons. Limitations do not change the warning-based exit status, so a run with
status 0 may still need attention. The [support guide](support.md) explains
the difference between an unknown fact and incomplete analysis.

## Rule families

Use a family to select a kind of concern, for example
`zincite lint --rules family:suspicious model.mzn`. Inspect individual rules
with `--list-rules` or `--explain RULE`:

| Family | What it checks |
| --- | --- |
| `correctness` | Inconsistent source relationships, such as incompatible array indices. |
| `suspicious` | Constructs that may be unintended, such as shadowing or hidden optional values. |
| `modelling` | Input assumptions, formulation choices and search coverage. |
| `performance` | Source structure that may create expensive work during flattening. These checks do not predict solver runtime. |
| `style` | Naming and source conventions. |

Families describe the concern, not its severity or whether it can be fixed
automatically. A correctness finding is still reported as a warning.
[Fix support](#suppression-and-explicit-fixes) is listed separately.

## Model context and source preservation

Checks that need types or declaration uses read the model and the files it
includes. Point Zincite at the matching MiniZinc standard library and any
project include directories:

```sh
zincite lint --rules all \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  -I project/includes model.mzn
```

`--stdlib-dir` overrides `MZN_STDLIB_DIR`; `-I` adds ordered include directories.
The default rules need this context for array comprehensions, indices, shadowing
and partial expressions. If required information is unavailable, Zincite reports
a limitation instead of guessing the result. Selecting only `naming,missing-constraint-label`
keeps analysis local to the source.
Standalone `.dzn` files use assignment-only syntax and mark semantic rules
inapplicable. Plain stdin lacks model context; semantic selections report a
limitation. `--stdin-filepath PATH` sets language mode, diagnostic path and the
starting directory for settings lookup, including for an unsaved file. Directories discover `.mzn` and
`.dzn` recursively, including hidden files, skip `.git`, and deduplicate canonical
paths. Ordinary analysis preserves original bytes, comments, spelling, BOM and
line endings.

## Project settings

Each root uses the nearest ancestor `zincite.toml`. Ancestor settings are not
merged. Includes use their root's settings. `--config PATH` applies one explicit
file to all roots; `--isolated` disables discovery. These switches are mutually
exclusive. Zincite validates every root's settings before analysis or writes.
`--show-settings` prints effective settings without reading model or stdin bytes.

For example, this `zincite.toml` adds scope checks to the defaults and permits
shadowing the short loop names `i` and `j`:

```toml
[lint]
select = ["default", "preset:scope"]

[lint.options.suspicious-shadowing]
ignore-names = ["i", "j"]
```

Without `select`, the defaults apply; `select = []` selects no rules.
`extend-select` adds rules and `ignore` removes them. An explicit `--rules`
replaces all three selection settings, while keeping rule options and fix
restrictions.

For a named review configuration, replace the example above with:

```toml
[lint]
select = ["preset:review"]

[lint.presets.review]
select = ["preset:arrays", "preset:domains", "expensive-comprehension"]

[lint.presets.review.options.expensive-comprehension]
max-candidates = 25000
```

This reviews arrays and domains and warns when a known upper bound on
comprehension candidates exceeds 25,000. The default threshold is 1,000,000.
The threshold must be a positive integer; it is not a runtime estimate.

A personal preset can include built-in presets, families and rule IDs, but
not another personal preset. Its name must not duplicate a built-in name.
Options in `[lint.options.RULE]` override options from the selected preset.
`ignore-names` matches exact binding names.

To restrict automatic edits, use `fixable` and `unfixable` in `[lint]`. They
accept rule IDs and families; `unfixable` wins if both select the same rule.
These settings affect edits, not which diagnostics are reported.

## Rule reference

Every rule has an example, advice on what to review and its analysis limits in
one of the group pages below. The tables on those pages identify default checks
and automatic fix support. An example illustrates the named rule; it may also
trigger other rules when you select a broader preset.

| Group | What to review |
| --- | --- |
| [Conventions](lint-conventions.md) | Names, constraint labels and numeric index origins. |
| [Scope](lint-scope.md) | Hidden names and dependencies, unused declarations and generator bindings. |
| [Arrays](lint-arrays.md) | Index compatibility, optional elements and array-construction wrappers. |
| [Domains](lint-domains.md) | Variable bounds, conflicting definitions and conditions required by expressions. |
| [Formulation](lint-formulation.md) | Constant decisions, Boolean formulations, globals, identity sets and vacuous constraints. |
| [Flattening](lint-flattening.md) | Comprehension size and decision-dependent operations, generators, conditions and globals. |
| [Search](lint-search.md) | Search coverage and possible symmetry-breaking intent. |

For a single check, use its ID, for example `zincite lint --rules
hidden-optionality model.mzn`. `zincite lint --explain hidden-optionality` gives
the command-line explanation of the same rule.

### Integer identity sets and enums

The [`prefer-enum` reference](lint-formulation.md#prefer-enum) explains when
integer IDs may be clearer as enum members, with an example and the implications
for external data and output consumers.

## Suppression and explicit fixes

An exact standalone directive suppresses its own rule throughout the next
complete top-level item:

```minizinc
% zincite-lint: ignore unused-generator-binding
int: total = sum([2 | unused in 1..3]);
```

Use one directive per rule. Unknown rule IDs, directives inside expressions
and directives without a following item are errors.

```sh
zincite lint --rules unused-generator-binding,element-predicate \
  --stdlib-dir "$MZN_STDLIB_DIR" --diff model.mzn
zincite lint --rules unused-generator-binding,element-predicate \
  --stdlib-dir "$MZN_STDLIB_DIR" --fix model.mzn
```

`--diff` previews eligible changes without writing. `--fix` applies one pass
and then runs analysis again, so its diagnostics describe the resulting files.
It accepts explicit regular files, not directories, symlinks or stdin. Files
read only through an include are not implicit write targets.

For these two rules, fixes are conditional. An unused generator name can become
`_` in supported comprehension forms. A standard `element` call can become an
indexing equality when Zincite can establish compatible types and valid indices.
An unproved condition or relevant annotation prevents that edit.

The rule list reports fix support as `always`, `sometimes` or `none`. That is
separate from the safety of a particular edit. Normal `--fix` applies only
Safe edits. `--unsafe-fixes` would also permit edits marked Unsafe, but no
current rule provides one. `prefer-enum` and missing constraint labels remain
modelling decisions for the author.

Overlapping edits are omitted and explained. Independent edits can still apply.
Before replacing a file, Zincite checks that the candidate parses and that
the original file has not changed since it was read.

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
