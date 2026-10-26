# Checker design context

Read this before choosing the compiler recipe, lowering FlatZinc, designing
reports or extending the admitted predicate set. The [area brief](../brief.md)
owns scope and behavior.

## The supplied proposal

*A FlatZinc Checker in Rust for WebAssembly*, specification of a master thesis
at Uppsala University, 5 January 2021, four pages. The user supplied it as
`flatzinc-checker 2.pdf` and states that the project was never built. This plan
adapts its ideas; the document is not evidence of an existing implementation.

- Page 1 motivates checking manual frontend changes against the same model
  used by a remote optimisation service, avoiding handwritten duplicate checks.
- Page 2 requires Rust/native/Wasm, parsing generated checker-specific FlatZinc,
  Serde serialization, deriving omitted values, incomplete results, and all
  violated constraints in human- and machine-readable form using annotations.
  It suggests roughly 100 ms interactive checking and dependency analysis.
- Page 3 proposes reuse across nearby assignments, minimal sufficient inputs
  with potentially multiple channelled viewpoints, and explanations. It
  contrasts this with a full solver, custom MiniZinc solution checkers and
  solver-based MUS extraction, and suggests local-search evaluation as an
  inspiration. These are design/research directions, not required architecture.
- Page 4 is the historical 20-week academic schedule, not a new estimate.

## The author's clarifying email

The user supplied an email from the original proposal discussion after the
initial plan. It is authoritative clarification of the intended use:

- The main application path supplies literals that determine a single full
  assignment through functional determination or potentially propagation.
  The component does not choose a solution by search.
- The application maps graphical state, such as a roster or plant layout,
  to primary model values. A shift start may become `start[37] = 55`. The
  checker is general; domain-specific interaction belongs to the caller.
- Users must be able to keep infeasible candidate states and move between
  them, receiving all violations after an edit. The analogy is CBLS evaluation
  with the user choosing moves, not a requirement to build a local-search solver.
- Structured, controllable errors let the caller identify affected entities
  and present its own feedback. Human-readable labels alone do not meet this
  requirement, and a separate application-specific checking model defeats the
  intended reuse of the optimisation model.
- The email describes insufficient literals as useful developer diagnostics
  and an integration problem in its envisioned application. The user's later
  clarification broadens this: insufficient input is useful sometimes and
  must be a report, not an error. Callers decide how to use and present it.
- Native/Wasm size, structured errors and reporting every violation are the
  stated reasons MiniZinc solution checkers do not meet this use case.
- Global inverse consistency was suggested as potentially useful literature.
  The mail does not select an algorithm or make this a consistency service
  over arbitrary partial configurations.

The brief uses "candidate assignment" for a completed state, including an
infeasible one. Reports also support insufficient input, with unresolved values,
blocked checks and any known violations. Completeness is independent of constraint
satisfaction. Failure to complete is not an error or proof of unsatisfiability,
and reporting violations is not MUS extraction. Propagation is a possible
completion technique, not a reason to prune infeasible states or stop on contradiction.

## Existing Zincite seams inspected on 26 October 2026

- [`zincite-syntax/src/lib.rs`](../../../../crates/zincite-syntax/src/lib.rs)
  exposes `parse`, `ParsedFile`, `SyntaxNode`, tokens and ranges. It accepts
  declarations, predicates, calls, annotations and solve items, but explicitly
  checks syntax rather than FlatZinc validity, types or values. `FileMode::Data`
  accepts assignment items; it does not restrict their expressions to literals.
  Reusing this parser is a starting point, not verified full FlatZinc support.
- [`zincite-syntax/Cargo.toml`](../../../../crates/zincite-syntax/Cargo.toml)
  has no dependencies. An optional syntax dependency lets preparation reuse it
  while a serialized-model execution build omits it.
- [`zincite-lint/src/lib.rs`](../../../../crates/zincite-lint/src/lib.rs)
  exposes bounded static facts about bindings, domains and definitions. These
  facts are useful design references, but they do not form a concrete FlatZinc
  assignment evaluator. Reusing the whole linter would bring unrelated analysis
  into the runtime.
- [`zincite-query/src/lib.rs`](../../../../crates/zincite-query/src/lib.rs)
  exposes inspection and explicit source/data edits. Its values and Zirql
  programs serve query execution, not checking a flattened constraint model.
  Do not route constraint execution through this language merely to share code.
- [`src/main.rs`](../../../../src/main.rs) forwards `fmt`, `lint` and `query`
  to sibling crate CLI functions. A `check` dispatcher and sibling binary fit
  the current project. No checker command exists at this planning baseline.
- [`docs/support.md`](../../../../docs/support.md) distinguishes syntax,
  static analysis and compiler validity. Preserve that separation while adding
  a new checker guide when a working command exists.

## FlatZinc/compiler references

Use the primary
[MiniZinc 2.10.1 FlatZinc specification](https://docs.minizinc.org/en/2.10.1/fzn-spec.html)
when checking syntax, predicate signatures, declaration domains, variable
definitions and solver-library integration. FlatZinc permits fixed values and
aliases in variable declarations. `is_defined_var`/`defines_var` mark functional
relationships; their use still needs an implemented evaluation direction.
Solver-specific libraries select the constraints emitted during compilation.

Local inspection found MiniZinc **2.10.1, build 33348285743**, on `PATH` via
`/Applications/MiniZincIDE.app/Contents/Resources/minizinc`. Its `--help` confirms
compile-only `--compile`, `-O0`/`--no-optimize`, `--keep-paths`, `--globals-dir`
and explicit FlatZinc output paths. This is available tooling, not proof of
a suitable checker compilation recipe; no such recipe was executed in planning.

The installed library files `std/flatzinc_builtins.mzn` and
`std/stdlib/stdlib_ann.mzn` under that application's
`Contents/Resources/share/minizinc/` are useful primary sources for increment 1.
They specify arithmetic and element signatures and the meanings of
`constraint_name`, `mzn_constraint_name`, `mzn_path`, `is_defined_var` and
`computed_domain`. In particular, domain metadata can distinguish a semantic
restriction from a solver convenience; the recipe must establish what survives
and what the runtime checks. Do not remove a domain restriction by guessing.

## Questions the first experiment must settle

1. Can a small checker library preserve useful globals and functional directions
   while emitting only predicates the first runtime will support?
2. Which application identifiers, payloads, labels and paths survive compilation
   and decomposition? Can a caller associate violations with its own entities
   without parsing prose? What fallback feedback is available when a source
   constraint becomes a domain restriction?
3. Which decisions and array aliases survive `-O0`, and how can callers submit
   edits without depending on generated auxiliary names? What compilation
   rewrites still restrict those edits?
4. What happens to source constraints whose expressions are undefined for a
   supplied edit, and which extra flattened constraints encode that condition?
5. Does an infeasible edit still determine all auxiliary values and reveal all
   independent violations, including downstream of a failed domain check?

Answer these with a few complete examples and direct FlatZinc inspection.
Broader compiler instrumentation, arbitrary MiniZinc output reconstruction and
an independent flattening implementation are outside the initial plan.
