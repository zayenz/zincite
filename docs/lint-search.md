# Search and symmetry breaking

These rules review explicit search annotations and constraints that might be
intended to break symmetry. Select them with `--rules preset:search`.
See [linting](linting.md) for standard-library setup and analysis limitations.

| Rule | Family | Severity | Default | Automatic fix |
| --- | --- | --- | --- | --- |
| [`search-coverage`](#search-coverage) | `modelling` | Informational | No | No |
| [`unmarked-symmetry-breaking`](#unmarked-symmetry-breaking) | `modelling` | Informational | No | No |

## search-coverage

Looks for decisions that an explicit search neither names nor determines through
supported unconditional definitions.

```minizinc
var 0 .. 10: morning_staff;
var 0 .. 10: evening_staff;
constraint morning_staff + evening_staff >= 8;
solve :: int_search([morning_staff], input_order, indomain_min, complete) satisfy;
```

The search names `morning_staff`, but that does not determine `evening_staff`.
Review whether both should be searched, whether one should be defined from the
other or whether solver-selected search is more appropriate. Naming both is one
possible explicit search:

```minizinc
solve :: int_search([morning_staff, evening_staff], input_order, indomain_min, complete) satisfy;
```

A solve item without a search annotation leaves the search to the solver, so
the rule reports nothing for it. Annotations such as `warm_start` and
`constraint_name` are not searches.

Zincite follows supported nested `seq_search` annotations, simple annotation
aliases, whole-array views and unconditional definition dependencies. A computed
value does not determine all of its inputs: searching a total alone does not
necessarily determine the individual staff counts.

Unknown annotations, opaque relations and unsupported definitions can prevent a
coverage claim. A finding does not certify that search is incomplete, and silence
does not certify completeness. Solver behaviour and the model's intended search
strategy still need review.

## unmarked-symmetry-breaking

Asks whether a standard lexicographic, precedence or monotonicity constraint is
intended to break symmetry.

```minizinc
include "increasing.mzn";
array[1 .. 3] of var 1 .. 3: assigned_shift;
constraint increasing(assigned_shift);
solve satisfy;
```

If the array positions represent interchangeable workers, ordering their
assignments might choose one representative of equivalent solutions. Mark that
intent with MiniZinc's wrapper:

```minizinc
constraint symmetry_breaking_constraint(increasing(assigned_shift));
```

If the order is a requirement of the actual problem, keep it as an ordinary
constraint and suppress the advice. The rule cannot establish that the workers
are interchangeable. Incorrectly marking a required constraint as symmetry
breaking can change the problem when symmetry breaking is disabled.

Existing standard `symmetry_breaking_constraint` wrappers mark nested uses.
User functions with matching names do not stand in for the standard constraints
or wrapper. Zincite neither adds the wrapper nor removes the constraint
automatically.
