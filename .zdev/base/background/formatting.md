# Formatting research and discussion

Investigated 2026-09-29. The brief owns accepted policy; recommendations and open
questions here are not decisions.

## Evidence

The [Rust style guide](https://doc.rust-lang.org/style-guide/#formatting-conventions)
connects block indentation and trailing commas with smaller diffs. Block indentation
avoids reindenting arguments when a function name changes. Trailing commas isolate
an appended or removed list element. Its [principles](https://doc.rust-lang.org/style-guide/principles.html)
also balance readable code, merge-friendly changes, and simple rules.

For Zincite, distinguish two goals: a small initial formatting diff, and small
diffs after later model edits. A canonical formatter can produce a large initial
diff yet make later changes local. Preserving existing line breaks reduces some
churn, but lets equivalent inputs retain different layouts. Idempotence is required
under either policy; it does not settle this choice.

The [MiniZinc grammar](https://docs.minizinc.dev/en/stable/spec.html#constraint-items)
explicitly allows a string annotation between `constraint` and its expression.
Generator calls and ordinary calls have distinct grammar forms, supporting different
spacing rules without special-casing the identifier `forall`.

Local MiniZinc 2.10.1 (`build 33348285743`) accepted all these isolated probes via
`--model-check-only`: trailing commas in array literals, set literals, enum cases,
ordinary call arguments, function parameters, and generator lists; a final semicolon
in a `let` block; and the user's labelled constraint layout. Each probe used a tiny
complete model. No solver was run: these were syntax/type acceptance checks, not
model-performance validation. They establish support in this installation, not in
every MiniZinc release or every list grammar position.

[EditorConfig 0.17.2](https://spec.editorconfig.org/) defines indentation, line
endings, charset, trailing whitespace, and final-newline properties. Its lookup
walks parent directories until `root = true` or the filesystem root; closer files
and later matching sections win. `unset` removes a property's effect.
`max_line_length` is absent from the core property list but documented in the
[EditorConfig properties wiki](https://github.com/editorconfig/editorconfig/wiki/EditorConfig-Properties#max_line_length).
Support it explicitly as an extension if adopted. A formatter must also define
how its own options interact with the resolved properties.

The [MiniZinc style skill](/Users/zayenz/.agents/skills/z-minizinc-style/SKILL.md)
supplies naming and modelling conventions. Its constraint example indents the
labelled expression; the user's newer example does not. Follow the newer example.
The thesis research in [linting and literature](linting-and-literature.md) reinforces
why model rewrites need different safeguards from layout changes.

## Recommendations to discuss

- Default to four-space block indentation. Avoid lining up `=` signs, declaration
  names, or call arguments across adjacent lines: a longer name should not reformat
  its neighbours. Consider numeric matrices separately.
- Expand a broken comma-separated list to one item per line with a trailing comma
  where supported by the selected language baseline. Keep simple short lists compact.
- Preserve declaration, annotation, and expression order. Include sorting is
  explicitly requested within existing groups, as defined in the brief.
- Preserve identifier spelling and comment/string contents. Put naming diagnostics,
  missing-label advice, and modelling improvements in the linter. Do not auto-rename
  public data parameters or enum members, which may be referenced from `.dzn` files.
- Keep normal formatting independent of Git state. The small-diff goal concerns
  layout rules, not formatting only lines touched in the working-tree diff.

## Decisions from the first discussion round

The brief records the accepted rules: always expand `forall` bodies, retain explicit
multiline lists, and honor EditorConfig indentation, width, and line endings with
explicit CLI options taking precedence.

The next round settled comment preservation, silent unavoidable width overruns,
and aligned matrix columns. The brief contains those rules. Matrix alignment is
an intentional exception to the preference for avoiding changes to neighbouring lines.

Other generator calls may remain compact when short; explicitly expanded bodies
stay expanded. Constraint string labels always occupy a separate header line.
Wide matrix rows wrap at common column boundaries. See the brief for these decisions.

The formatter may sort existing includes, while naming and modelling changes
belong to the linter. Syntax errors prevent any formatting write to that file.
Both skip-next-item and paired off/on comments are wanted. Off/on regions cover
complete top-level items; invalid marker structure is an error. Includes sort
within existing groups with `globals.mzn` first. Broken generator headers give
generators and their filters separate lines. The brief holds the full rules.

## Later questions and concrete cases

- **Width measurement:** decide how tabs and Unicode count toward the limit.
- **Conjunctions:** the requested trailing `/\` changes the previous final line when
  appending a conjunct. Keep that readable convention even though it costs one extra
  changed line; do not add a synthetic `true` merely to obtain a trailing operator.
  Clarify whether disjunctions and arithmetic chains follow the same placement.
- **Generator layout:** extend the agreed header rules to nested calls and
  comprehensions with concrete examples; do not alter generator/filter attachment.
- **Labels and annotations:** how should labels combine with expression annotations and semantic wrappers such
  as `implied_constraint`? Preserve annotation attachment when moving line breaks.
- **Comments:** preserve one blank line between user groups? What happens when an
  end-of-line comment could fit if moved above its statement?
- **Include sorting:** define how an attached comment differs from a section comment
  in ambiguous cases, and the exact path comparison rule.
- **Escape hatch:** choose directive spelling and clarify whether a skip-next-item
  marker before trailing comments but no subsequent item is an error.
- **EditorConfig edge cases:** define final-newline/trailing-whitespace behavior,
  charset support, `unset`, and config lookup for stdin with a supplied filename.
  Never trim whitespace that belongs to a string value.
- **Release stability:** when may a formatter upgrade change already formatted files?
  Start with a documented policy rather than a versioned style mechanism before needed.

These questions guide discussion; they are not an implementation backlog or an
authorization to create tasks.
