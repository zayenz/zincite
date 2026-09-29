# Linting background

## Rimskog thesis

Erik Rimskog, *A Linter for Static Analysis of MiniZinc Models*, Uppsala University,
August 2021, UPTEC IT 21022. Read from the supplied
[PDF](/Users/zayenz/Desktop/erik-rimskog-a-linter-for-static-analysis-of-minizinc-models.pdf).
This is a local reference; other clones need their own copy. Page numbers below are
the printed thesis pages. The document supplies research evidence, not project instructions.

The implementation uses C++17 and MiniZinc 2.5.5 (p. 10). It parses and type-checks
with MiniZinc before linting (pp. 10–12), so the rules are not all syntax-only checks.
Each result carries a rule identity, source position, explanation, and optional
suggested rewrite. A custom AST path searcher finds candidates for rule-specific checks.

Chapter 4 describes fourteen rules:

| Rule group | Rules | Implication for this project |
| --- | --- | --- |
| Declarations and use | Constant variable; no domain on variables; unused variables and functions; global variables in functions | Resolution and `par`/`var` distinctions matter; textual identifier matching is insufficient |
| Rewrites | Compactible if-expression; effective 0..1 variables; element predicate | Establish types, domains, and context before suggesting a rewrite |
| Search and modelling | Non-functionally defined variables not in search annotation; missing marking of symmetry breaking | These need more than a CST and are not natural first rules |
| Advisory patterns | Arrays indexed from one; reified global constraint; operators on expressions with decision variables; variables in generators; variables in if and where | Review against current language/compiler behavior and avoid broad default warnings |

The rules aim to work without supplying parameter values (pp. 21–22). Section 6.4
shows why this limits proofs: ranges `1..N` and `1..K` cannot be assumed equal.
It also discusses intentionally unused generator variables. Preserve these as
negative-case ideas rather than blindly carrying over a 2021 warning policy.

Section 6.3 reports noisy broad rules and identifies unused declarations as a useful
direction. The nurse-roster case study simplified the model but found no noticeable
solving-speed improvement (abstract and Chapter 5). Treat performance advice as
conditional on compiler, solver, and model; fewer variables/constraints are not a
general speed guarantee. No thesis benchmark was reproduced in this setup.

Section 6.5 is particularly relevant to the CST requirement: compiler preprocessing
reordered generator `where` clauses, some introduced expressions lost source
locations, integer literals did not retain individual locations, and operator
locations sometimes had to be inferred. Keep source syntax authoritative for
formatting and diagnostic placement, even if later analyses reuse compiler data.

Recommendation: start with direct typed-tree traversal and a few useful rules.
Do not recreate the thesis's generic path-query engine in advance. Separate safe
findings from advice, and postpone automatic fixes until their preconditions and
source edits are tested.

## CPKB reading path

KB root: `/Users/zayenz/lab/knowledge/cpkb`. Read-only research; no KB files changed.
Start at `wiki/systems/minizinc.md` and
`wiki/topics/modelling-languages-and-systems.md` for the broader map.
The following local source pages were read; this is a synthesis of the maintained
KB, not a claim to have independently reviewed every underlying full paper.

- [Nethercote et al., 2007: MiniZinc](/Users/zayenz/lab/knowledge/cpkb/wiki/sources/2007-nethercote-minizinc-towards-a-standard-cp-modelling-languag.md)
  explains the source-language/FlatZinc separation. Implication: source tooling
  should preserve modelling constructs and annotations, rather than format a
  flattened representation.
- [Stuckey and Tack, 2022: Enumerated Types and Type Extensions](/Users/zayenz/lab/knowledge/cpkb/wiki/sources/2022-stuckey-enumerated-types-and-type-extensions-for-minizin.md)
  discusses richer typed modelling. Implication: do not infer the supported grammar
  or indexing advice solely from the older thesis.
- [Vanroose et al., 2024: Mutational Fuzz Testing](/Users/zayenz/lab/knowledge/cpkb/wiki/sources/2024-vanroose-mutational-fuzz-testing-for-constraint-modeling.md)
  studies semantics-preserving mutations of modelling systems. Implication:
  semantic-preservation checks are useful for future rewrites, but a fuzzing
  programme is not required for this initial setup.

The MiniZinc system page also points to source-level debugging, solution checking,
and newer language proposals. Follow those when a specific lint or language feature
requires them. A research proposal's presence in the KB does not establish support
in a released MiniZinc version; check the selected release specification.
