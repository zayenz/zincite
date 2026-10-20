# Release notes

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
