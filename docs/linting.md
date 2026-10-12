# Linting MiniZinc with Zincite

Ordinary `zincite-lint` analysis reports diagnostics and preserves files.
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

The default selects `naming`, `missing-constraint-label` and `prefer-array-comprehension`. `thesis` keeps its
fourteen original rules; `all` selects all 28 currently available rules. IDs,
`family:NAME` and `preset:NAME` can be combined with commas. Expansion keeps the
first occurrence of each ID. `default`, `thesis` and `all` remain accepted legacy
selectors. `--list-rules` gives family, availability, fix support and membership.

Families are `correctness`, `suspicious`, `modelling`, `performance` and `style`.
Selection does not change severity. Symbolic Cartesian advice from
`expensive-comprehension` is informational (`info`); a proved threshold
exceedance is a warning. Informational findings alone retain status 0.
Unsuppressed warnings produce 1. Usage, settings, input, syntax, directive or
dependency errors produce 2, taking precedence over warnings and information.
Limitations alone retain status 0 or 1.
Independent input files continue after errors. A rule's `Completed`,
`Inapplicable`, `Limited` or `NotRun` library result describes its analysis;
command status 0 does not promise complete semantic coverage.

Both `zincite-lint` and `zincite lint` label model diagnostics with
`[root PATH]` as well as the actual source path and original range. A shared
include can have different findings under different roots. By default each root
with limitations reports their exact count and says analysis is incomplete.
Use `--show-limitations` to see every located limitation. This changes only
presentation; library findings, limitations and rule outcomes remain available
in full.

## Choose a category and inspect fix support

Use a family to select a kind of concern, then inspect individual rules with
`--list-rules` or `--explain RULE`:

| Family | Reader's question |
| --- | --- |
| `correctness` | Is a supported source relationship inconsistent, such as an index-set mismatch? |
| `suspicious` | Does the code contain a likely unintended construct, such as shadowing or hidden optionality? |
| `modelling` | Is there a modelling concern, such as an unused binding or missing input precondition? |
| `performance` | Does supported structure suggest expensive work? This does not estimate runtime or promise a speedup. |
| `style` | Does the code depart from naming or layout-related modelling conventions? |

A correctness-family finding is still a warning. Advisory wording asks for a
modelling decision, and a rule with no fix may still diagnose a concrete concern.
Use the [fix guide](#suppression-and-explicit-fixes) to distinguish Safe,
conditional and diagnostic-only support before applying changes.

## Model context and source preservation

Semantic rules load the root and its include closure. Supply the library root
that matches the model:

```sh
./target/release/zincite-lint --rules all \
  --stdlib-dir /Applications/MiniZincIDE.app/Contents/Resources/share/minizinc \
  -I project/includes model.mzn
```

`--stdlib-dir` overrides `MZN_STDLIB_DIR`; `-I` adds ordered include directories.
The default loads model context for `prefer-array-comprehension`, using source and
callable signatures without expanding selected standard bodies. Selecting only
`naming,missing-constraint-label` keeps analysis local to the source.
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

## Additional rules

| ID | Family | Supported advice and important boundary |
| --- | --- | --- |
| `prefer-array-comprehension` | style | Standard `array1d`, `array2d` and `array3d` wrappers around matching independent comprehensions. Safe explicit fixes preserve body text and Cartesian axis order. Plain 1D requires proved contiguous `1..n`; other 1D keeps its key and 2D/3D keep ordered tuple keys, with every axis proved nonempty. Filters, dependent generators, annotations and unknown callable/set identity stay unchanged. Comments in removed wrapper text withhold a fix. |
| `prefer-enum` | style | Opt-in advisory recommendation for finite integer identity sets. Requires supported member/index use throughout a complete model; numeric member observations and representation-sensitive output withhold advice. No automatic fix. |
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

Rules can report located limitations for unsupported required facts. Zincite
invokes neither a compiler nor a solver and does not infer arbitrary callee
meaning, symbolic algebra, feasibility or optimization benefit. Current installed-library and corpus limitations are
recorded in [the expansion checkpoint](../scripts/lint-expansion-checkpoint.md).

## Definitions and domain advice

`value_safety` and `definitions` retain Unknown for typed nonoptional integer
selections and array comprehensions whose membership, iteration or values are
unproved. `domains` does the same for opaque union comprehension operands,
while keeping named set identity, union operands and useful closed minima.
These facts need no body interpreter to certify admission. Unknown proves no
member, bound, whole-array definition or safe edit.

`unbounded-variable` still advises on simple bare numeric declarations and
understood conditional or partial definitions. Complete anchored definitions
and explicit scalar or matching per-element ranges suppress that advice.
An opaque initializer or relation withholds advice on its conservatively
referenced targets; an independent declaration can still warn. When references
cannot delimit the affected targets, the owner retains the necessary wider
veto. Supported opacity can complete without a finding. Known source failures
and unsupported required operations remain located limitations.

`constant-variable` and semantic fixes still require Supported safety and a
complete enforced definition. Search retains Unknown for relevant opaque
relations and enclosing lets, instead of warning from a missing output proof.
Unknown boundaries supply no search coverage; original searched seeds and
complete definition guarantees keep their stronger requirements. Required
typing, overload selection, closed arithmetic errors, known-empty strict
extrema, known out-of-axis selections, sibling failures and cycles remain
explicit. A refusal of a stronger output proof is distinct from a source
failure; diagnostic text does not select rule behavior.

## Integer identity sets and enums

Select `prefer-enum`, `family:style` or `all` to enable this advisory rule.
It remains outside `default` and `thesis`, and it offers no automatic fix.
The finding points to the set name and asks the modeller to consider an enum;
it does not establish an equivalent migration or compatibility with external
instances or output consumers.

Initial candidates are named, nonempty parameter integer sets initialized by
written closed integer ranges or literal sets. Direct set/type/scalar aliases
retain declaration identity. Candidate-domain scalars and membership generators
supply member identities; matching integer bounds never connect distinct sets.
A candidate needs an actual supported member equality, membership or index use.
Unused sets, cardinality arithmetic and count-only iteration receive no advice.

The analysis checks all written user items, including unused initializers,
callable bodies, objectives and annotations. Resolved core scalar equality and
disequality, same-family membership, set equality/union/intersection/difference/
symmetric difference/subset/superset, `card`, and indexing are supported.
Direct indexed entries/comprehensions retain their member keys as array axes.
Candidate array axes stay separate from independent Boolean/numeric payloads:
`cost[p] + 1` does not perform arithmetic on `p`. Core array payload aggregates
(`sum`, `product`, `min`, `max`, `forall`, `exists`, `length`) are independent;
`index_set` retains a one-dimensional candidate axis. Numeric member arithmetic,
aggregation, literals/sentinels, ordering, extrema, conversions and concrete
integer callable contracts withhold advice.

Uniquely selected acyclic first-order user calls map positional/named arguments
and retained defaults into separate formal frames. Candidate-domain and generic
scalar/set/index contracts are supported; a generic signature still requires
checking its written body. Direct returned aliases, captured globals, locals
and annotations are inspected. A numeric literal supplied to a candidate-domain
parameter or returned from a candidate-domain function withholds advice. An
unavailable argument or result origin retains a located limitation for that
candidate. Independent payload/count results remain independent. Numeric result
contracts also withhold advice.
Recursive or bodyless calls, unresolved selection, separate assignments,
computed generator membership/results, candidate-member array/tuple/record
storage or projection, annotated member expressions and other affected standard
operations remain located limitations. Unrelated unknown operations do not veto
an independently classified candidate. A limited candidate can coexist with a
finding for another candidate.

Explicit member/set output and whole candidate-indexed array output withhold
advice, including `show` and string interpolation. Default output follows the
resolved `no_output`, `add_to_output` and `output` conventions, including written
initializers and anonymous array literals. Uncertain relevant markers withhold
advice. Independent scalar payload/count output is supported. Fragments without
a solve item and standalone data are inapplicable; multiple solve items or
incomplete dependencies cannot establish model-wide uses.

MiniZinc 2.10.1 compilation controls used Gecode with compilation only. Integer
and enum indexing and cardinality controls compiled. Replacing offset integer
members with enum ordinals in direct sums and a hidden integer function produced
implicit enum-to-int coercion warnings and failed numeric assertions. Default
integer-axis array output used `showDzn`; the enum control generated named index
strings and `show_indexed`, even with Boolean payloads. An ordering control
compiled, so the initial ordering exclusion is conservative. These controls
establish representation risks, not migration equivalence or solving behavior.

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

The catalogue distinguishes automatic fix support (`always` or `sometimes`)
from diagnostic-only rules (`none`). Advisory rules ask for a human modelling
decision; lack of a fix does not turn a correctness diagnostic into advice.
Fix availability is separate from family, warning severity and each edit's
safety. `prefer-enum` is advisory and diagnostic-only;
`prefer-array-comprehension` offers conditional Safe fixes described above.

The two rules in the command examples advertise conditional Safe fixes. Unused names become `_` only in
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
