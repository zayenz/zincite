# Release notes

## 0.5.0

Linting changes its defaults so that a failing run points at a likely mistake.

- Change the default lint rules to `prefer-array-comprehension`, `index-set-mismatch`, `suspicious-shadowing`, `partial-expression`, `hidden-optionality`, `vacuous-constraint`, `unbounded-variable`, `unused-declaration`, `pairwise-self-comparison` and `deprecated-call`. `naming` and `missing-constraint-label` now run only when selected; add them with `select = ["default", "preset:conventions"]` in `zincite.toml` or `--rules default,preset:conventions`.
- Record a fixed severity for each rule. Advice that proposes another formulation now reports `info` findings, which do not fail a run: `constant-variable`, `compact-if`, `effective-zero-one`, `reified-global`, `search-coverage`, the three `decision-variable-*` rules, `unmarked-symmetry-breaking`, `global-constraint-opportunity`, `missing-input-precondition` and `unused-declaration`. `--list-rules` and `--explain` show the severity.
- Move `array-index-start`, `element-predicate`, `global-variable-in-function` and `unused-generator-binding` to the `style` family.
- Resolve initialized parameters and named sets in `index-set-mismatch`, `partial-expression` and `vacuous-constraint`, so `int: n = 3; array[1..n] ...; x[4]` reports like the literal form. Report symbolic offsets such as `forall (i in 1..n) (x[i + 1])` over `array[1..n]` when the access leaves the index set for every value of `n`.
- Add `pairwise-self-comparison`, a default correctness rule. It warns when an enforced `!=`, `<` or `>` compares two generator bindings over the same source and nothing excludes their equal pair, as in `forall (i, j in S) (x[i] != x[j])`. Such a constraint cannot be satisfied.
- Add `deprecated-call`, a default rule in the `conventions` preset. It warns when a call selects a standard-library callable marked `mzn_deprecated` and quotes the version and explanation. Zincite now reads the standard library's `.deprecated.mzn` companion files, as the MiniZinc compiler does.
- Add three opt-in rules: `prefer-assertion` (conventions) suggests `assert` for constraints over parameters only; `ambiguous-precedence` (conventions) warns about unparenthesized `/\` mixed with `\/` or `xor` and about chains of `->` or `<-`; `float-equality` (domains) warns when `=` or `!=` compares two parameter floats.
- Run `search-coverage` only when the solve item has a search annotation.
- Report unused generator names only through `unused-generator-binding`, not also through `unused-declaration`.
- Stop `suspicious-shadowing` from reporting a function or predicate parameter that shares a top-level name.
- Limit `decision-variable-operator` to `^`, `div`, `mod` and `/`; Boolean connectives no longer report.
- Speed up the shared analysis behind the default rules. On two large Challenge models the five slowest default rules drop from about 62 s to 2.4 s and from 130 s to 8.7 s, with identical output.
- Count warnings and informational findings separately in the corpus scripts, and accept informational findings in the documentation example checks.

## 0.4.0

- Add `zincite check`, the standalone `zincite-check` command and a reusable Rust checker for MiniZinc 2.10.1 text FlatZinc and official FlatZinc-JSON.
- Fill auxiliary values through supported functional definitions and report every evaluable constraint violation, application metadata and available objectives. Report completeness separately; missing input is an ordinary report outcome.
- Cover the MiniZinc 2.10.1 standard library and the Gecode main snapshot from 2026-10-26, with checked i64 values, compact integer sets and configurable finite-float comparisons. Diagnose external-execution and search-history hooks explicitly.
- Export validated prepared models for repeated native or parser-free WebAssembly checks. Keep integer values exact across JavaScript boundaries.
- Add integration and architecture guides, offline compiler recipes and a graphical workshop example. Document compiler limits and measurements for the named native/browser build.

## 0.3.0

- Use Zirql 0.3.0 for shared program reading, preparation, scalar conversions, formatting, collection operations and JSON/Markdown reports.
- Retain native dictionaries through common report operations and typed functions, including empty schemas and native payloads.
- Align sorting, selection, cardinality, structural unique and tally with constructed values while preserving native identity and result order.
- Use shared function-call admission and suffix reading, reducing duplicate query machinery.
- Link the shared library reference from the MiniZinc guides. Model facts, completeness, source-preserving rendering and Data/edit restrictions remain in Zincite.

## 0.2.0

- Adopt Zirql 0.2 for general constructed dictionary keys and set elements,
  including nested collections and dictionary-valued tally results.
- Render general report dictionaries as tagged key/value entries while retaining
  ordinary string-key JSON objects. Update public collection/declaration adapters.
- Load model facts for enum types nested in dictionary signatures.
- Keep MiniZinc data edits restricted to representable data. General native sets
  and dictionaries remain outside the current host representation.

## 0.1.2

- Copy native enum members and collection elements with `clone_members`,
  `clone_elements`, `repeat_elements` and `append`, retaining comments and spelling
  while validating the complete candidate against model facts.
- Grow bounded constructor enums with `scale_enum` and document a complete wedding
  growth query with preview, save and write commands.
- Require Zirql 0.1.1 for shared evaluator performance improvements.

## 0.1.1

- Complete enum reductions through supported nested literal guest-group iteration,
  retaining explicit refusal for computed or unproved dependencies.
- Add a source-preserving wedding growth example with deterministic guest and
  group remapping, repeated capacities and bounded candidate output.
- Avoid repeated source-location construction in callable output lookup. On the
  retained Java-routing model, comprehensive linting improves from about 8.5 to
  3.2 seconds with identical diagnostic output.
- Clarify query guides and keep shared-language explanations self-contained.

## 0.1.0

Zincite provides a root command and three standalone commands for MiniZinc
2.10.1 models and assignment-only data files:

- `zincite fmt` formats source, checks files and directories, and writes explicit
  files. It preserves comments, spelling and protected regions, reads
  EditorConfig, and refuses stale or symlink writes.
- `zincite lint` offers six default rules, the fourteen-rule thesis preset and
  opt-in rule families. It reports warnings and analysis limitations separately;
  fixes require an explicit request and sufficient supporting facts.
- `zincite query` selects source and semantic facts, reuses saved results and
  computes bounded reports through Zirql. Explicit data edits validate complete
  candidates before writing. Later errors leave stdout empty.

Query expressions use Zirql's common reader. Both comprehension spellings,
shared tuple/list patterns and named or implicit predicates work across
constructed reports and native predicate positions. Scalar `filter_elements`
bodies retain the closed Data policy and source-preserving edit checks.

The syntax, formatting, lint and query libraries share version 0.1.0.
Current stable Rust is supported. Formatting and interfaces may change within
the 0.1 series. See the [guides](https://zayenz.se/zincite/) and
[support limits](https://zayenz.se/zincite/guides/support/) for supported syntax,
Unknown facts, incomplete results and write restrictions.

The earlier `0.0.0-pre` packages reserved names; 0.1.0 contains the tools.
