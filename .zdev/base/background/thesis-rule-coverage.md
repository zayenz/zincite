# Thesis rule coverage contract

Erik Rimskog, *A Linter for Static Analysis of MiniZinc Models*, Uppsala University,
2021, UPTEC IT 21022, Chapter 4, printed pages 21–31, is the required catalogue.
Read the [thesis](https://www.diva-portal.org/smash/get/diva2%3A1591018/FULLTEXT01.pdf)
or the user's local copy at
`~/Desktop/erik-rimskog-a-linter-for-static-analysis-of-minizinc-models.pdf` when
implementing the matching section. The planning pass read every rule and the
limitations in sections 6.3–6.5. The earlier
[research summary](linting-and-literature.md) supplies context, not a restriction
to the original two-rule scope.

The table defines Zincite's stable IDs and required observable detection families.
The thesis used MiniZinc 2.5.5's parser and type checker. Zincite owns its source
representation and builds only the semantic facts needed by these consumers.
A rule's presence in the registry is not implementation of that rule.

| Section | Zincite rule ID | Detection and necessary limits |
| --- | --- | --- |
| 4.1 | `array-index-start` | Advise on user numeric array index ranges whose lower bound is provably not one. Resolve named domains where possible. Enum, unknown and unconstrained `int` index sets must not be guessed; enum indices are not bad numeric offsets. |
| 4.2 | `compact-if` | Recognise a decision Boolean condition with integer branches where one branch is zero, in either order. Give conditional advice about a Boolean-to-integer formulation; no claim of faster solving and no unsafe suggestion for optional, partial or unknown-typed expressions. |
| 4.3 | `constant-variable` | Detect scalar/array `var` initialisers that are parameter expressions, unconditional equality to parameter expressions, and complete unfiltered `forall` element definitions over matching array index sets. Unknown parameter values remain symbolic; `1..N` and `1..K` are not equal without evidence. |
| 4.4 | `effective-zero-one` | Recognise both `a=1 -> b=1` and `a=0 -> b=0`, plus `sum(i in S)(a[i]=1)` on matching whole 0..1 arrays. Prove domains and constants for every instance, including simple statically evaluable arithmetic. Parameter-dependent or partial domains do not justify an unconditional rewrite. |
| 4.5 | `element-predicate` | Recommend indexing equality for a resolved standard-library three-argument `element` predicate. A user overload with that spelling is not evidence. |
| 4.6 | `reified-global` | Advise on resolved included standard-library Boolean predicates used in reified/half-reified contexts; exclude implicitly available builtins such as `forall`. Distinguish always-enforced constraints/conjunctions from disjunctions, implications and Boolean values. |
| 4.7 | `global-variable-in-function` | Report references from user callable bodies to top-level decision variables; distinguish shadowed parameters/locals and parameter-valued globals. Include predicates/tests as applicable, and do not mistake a callable name or field label for a variable use. |
| 4.8 | `unbounded-variable` | Advise on user integer/float decision declarations without an explicit domain, initializer or unconditional defining equality. Include array element declarations and aliases. Conditional equations and partial element coverage cannot establish a complete definition. |
| 4.9 | `search-coverage` | Advise on decision variables not covered by the solve search and not derivable from searched variables through unconditional definitions. Follow standard search annotations, nested `seq_search`, array views and user annotation aliases; follow callable output definitions, including standard predicates. Require whole-array coverage and handle dependency cycles conservatively. |
| 4.10 | `decision-variable-operator` | Advise on `^`, `div`, `mod`, `/`, `xor`, disjunction, `->`, `<-`, `<->` and `not` when relevant operands depend on decisions. Handle equivalent operator spellings. Parameter-only expressions do not match. Tabling is conditional advice, never a guarantee or an automatic rewrite. |
| 4.11 | `unmarked-symmetry-breaking` | Advise on resolved `lex2`, `lex_greater`, `lex_greatereq`, `lex_less`, `lex_lesseq`, `strict_lex2`, `seq_precede_chain`, `value_precede`, `value_precede_chain`, `increasing` and `decreasing` outside `symmetry_breaking_constraint`. These may be required model constraints; do not claim proven symmetry or recommend unconditionally disabling them. |
| 4.12 | `unused-declaration` | Traverse declaration dependencies from constraints, solve/output items and relevant annotations, including implicit output. Find unused variables/callables and unreachable cycles; report an unused enclosing declaration once rather than every nested binding. Resolve includes, scopes and overloads; exempt anonymous/intentionally unused bindings. A fragment without a complete root cannot establish export unreachability. |
| 4.13 | `decision-variable-generator` | Advise when a comprehension/generator domain or assignment expression depends on decisions, including aliases, calls and nested generators. A parameter-valued iteration binding alone is not a match. |
| 4.14 | `decision-variable-condition` | Advise when an if/elseif condition or generator `where` filter depends on decisions. The existence of a decision expression only in a branch/body is not a match. |

## Shared facts and conservative behavior

Track bindings, callable candidates, type/instantiation facts, simple symbolic
index sets and numeric domains without evaluating arbitrary model code or requiring
data. Distinguish unknown from both `par` and `var`. Support the language features
that occur in real corpus models: arrays, sets, enums, aliases, option values,
records/tuples, comprehensions, annotations and overloaded/polymorphic calls.
Do not infer the meaning of a standard predicate from its name alone.

Equality and search analysis share a context walk: top-level enforced constraints,
conjunctions, unfiltered universal quantification over proven domains, and resolved
transparent wrappers can establish a definition. Disjunction, implication branches,
reified expressions and opaque calls cannot simply be treated as unconditional.
Check partiality and optional values before suggesting equivalence. A self-reference
or mutual cycle is not a proof of a constant value or of search coverage.

For search coverage, use searched variables as seeds and add variables whose full
known dependencies are covered. Do not repeat the thesis's admitted approximation
that one assigned array element defines the whole array. Model-specific annotations
or expressions beyond the supported analysis receive an explicit limitation,
not a claim that the search is complete. This is advice about the thesis criterion,
not certification against the current year's Challenge competition rules.

All fourteen rules are available together through `--rules thesis`; this preset
selects only the thesis group. Existing defaults stay unchanged. Every implemented
rule supports the same next-item suppression and file-aware text diagnostic path.
Use a few focused positive/negative examples, especially known false-positive risks;
reuse corpus checks for broad integration. Do not reproduce the thesis's search
engine, benchmark campaign or C++ architecture.
