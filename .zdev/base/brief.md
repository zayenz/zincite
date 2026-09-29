# MiniZinc tooling foundations

## Objective

Establish the Rust syntax foundation shared by zincite-fmt and zincite-lint.

Zincite will provide MiniZinc development tools, starting with a formatter
(`zincite-fmt`) and a static analyser (`zincite-lint`). The `base` area establishes the
shared parsing and source representation and the smallest useful path to both
tools. This shaping pass records background and decisions; it does not start
implementation or create tasks.

## Settled decisions

- Implement in Rust, organised as a Cargo workspace with reusable crates.
- Share a concrete syntax tree (CST) between formatting and linting. Keep source
  spelling, comments, whitespace, and source locations available to consumers.
- Use `zincite` as the project name and crate prefix, with executable names
  `zincite-fmt` and `zincite-lint`.
- Use zdev project records. `base` follows configured trunk `main`, as requested.
- Keep the existing checkout directory `mzn-tools`; its filesystem name need not
  match the project name.
- Build Zincite's own Rust parser, CST, formatter, and linter. Keep the project
  independent of Shackle: use it as a research and design reference only, with
  no Shackle code reuse, crate dependencies, or planned integration.
- Prefer simple modules and direct tree traversal. Add crates, generic interfaces,
  semantic machinery, and tests when an actual consumer or risk justifies them.

## Proposed approach

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
initial requirements. Do not copy Zirium's MLIR dialect machinery or port the
thesis's entire rule catalogue. Do not promise solver speedups from syntactic
rewrites. A source-preserving parse and a formatter are different operations.

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
untouched. Skipping does not exempt source from syntax checking. Directive
spelling remains to be settled.

Keep short generator headers compact. When a header exceeds the width limit, use
block indentation with one generator per line and each `where` filter on a following
line indented beneath its generator. Preserve generator/filter order and attachment.

Read [formatting background](background/formatting.md)
for evidence, recommendations, and the remaining policy questions.

## Open questions

1. Parser design: choose the smallest useful lexer, parsing approach, and CST
   representation for exact source coverage, typed traversal, and error recovery.
2. Language baseline: which MiniZinc release, and should the first release also
   format `.dzn` files? The current stable handbook inspected was 2.10.1; the
   thesis's 2.5.5 is historical context, not a proposed compatibility target.
3. First lint rules: which useful checks can run on syntax alone, and which justify
   scoped name/type analysis within Zincite? Define default severities and
   suppression behavior with the chosen rules.
4. Formatting: resolve skip-directive spelling, expression annotations, and the
   remaining detailed layout cases using the background questions.
   EditorConfig newline/whitespace details, CLI check mode, and file rewriting
   also remain to be shaped.
5. Project license and release packaging remain undecided.

## Testing

Existing checks only for this setup. It adds no code or test harness. Validate
zdev records and document links.
For implementation, prefer a small set of behavior checks: exact source/token
coverage (not merely returning a saved input string), recovery past an error,
formatter idempotence and comment preservation, and positive/negative cases for
each selected lint. Include shadowing or parameter-dependent cases when relevant.
Do not create a benchmark campaign or broad test matrix before there is a measured
need. Compiler acceptance can supplement formatting checks; it is not a proof of
semantic equivalence.

## Background

- [Syntax design references](background/syntax-and-reuse.md): read before designing
  the parser, CST representation, or crate boundaries.
- [Linting and literature](background/linting-and-literature.md): read before
  selecting lint rules, semantic prerequisites, or rewrite claims.
- [Names](background/names.md): naming decision and registry checks; read before
  publishing packages.
- [Formatting](background/formatting.md): read when shaping layout rules,
  EditorConfig support, or the formatter/linter boundary.

## Validation

Run `zdev check base --format json` after changing these records. The setup is
ready for discussion when this brief indexes the reusable research, the naming
decision is recorded, and all decisions above are distinguishable from proposals.
Create tasks only after the user requests that next interaction.
