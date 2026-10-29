# Model formulation

These rules suggest ways to express a model's decisions and constraints more
directly, or point out expressions that cannot do what their surrounding code
suggests. Select them with `--rules preset:formulation`.
See [linting](linting.md) for selection and fix settings.

| Rule | Family | Severity | Default | Automatic fix |
| --- | --- | --- | --- | --- |
| [`constant-variable`](#constant-variable) | `modelling` | Informational | No | No |
| [`compact-if`](#compact-if) | `performance` | Informational | No | No |
| [`effective-zero-one`](#effective-zero-one) | `performance` | Informational | No | No |
| [`element-predicate`](#element-predicate) | `style` | Warning | No | Sometimes |
| [`global-constraint-opportunity`](#global-constraint-opportunity) | `modelling` | Informational | No | No |
| [`prefer-enum`](#prefer-enum) | `style` | Warning | No | No |
| [`vacuous-constraint`](#vacuous-constraint) | `suspicious` | Warning | Yes | No |
| [`pairwise-self-comparison`](#pairwise-self-comparison) | `correctness` | Warning | Yes | No |

## constant-variable

Suggests a parameter when a decision variable's entire value is determined by
input data or a parameter expression.

```minizinc
var int: days_per_week = 7;
```

There is no decision to make about `days_per_week`. Write `int: days_per_week = 7;`
if that is the intended meaning. Zincite can also follow supported enforced
equalities and complete, unfiltered `forall` definitions of array elements.

Zincite must establish the complete definition before recommending a parameter.
Conditional, cyclic and partial definitions do not receive this recommendation.

## compact-if

Suggests multiplication by a Boolean-to-integer conversion for an integer
conditional controlled by a decision Boolean, with one branch equal to zero.

```minizinc
var bool: open;
var 0 .. 10: staff;
var int: paid_hours = if open then
    8 * staff
else
    0
endif;
```

For this expression, the alternative is `bool2int(open) * (8 * staff)`. When the
zero branch is the `then` branch, the conversion uses `not open`.

Keep the form that best explains the model and measure any performance change
that matters. The rule gives no solving-speed guarantee. Optional values,
potentially partial expressions and unknown types prevent replacement advice:
multiplication can require a value even when the conditional would not select
that branch.

## effective-zero-one

Recognises selected implications and counts involving integers whose domain is
proved to be `0..1`.

```minizinc
var 0 .. 1: morning_open;
var 0 .. 1: afternoon_open;
constraint morning_open = 1 -> afternoon_open = 1;
```

For zero-one values, the implication can be written as
`morning_open <= afternoon_open`. This depends on the domains: it would not
express the same relation for arbitrary integers.

The rule also suggests `sum(flags)` for a proved whole-array traversal counting
the entries equal to 1. Domain and traversal conditions must hold for every
instance; a matching parameter default is insufficient. Partial traversals,
optional values and unknown bounds prevent advice. Zincite does not apply these
reformulations automatically or promise faster solving.

## element-predicate

Recommends an indexing equality for a resolved standard three-argument `element`
call.

```minizinc
include "element.mzn";
array[1 .. 3] of int: required = [4, 6, 5];
var 1 .. 3: day;
var 4 .. 6: selected_required;
constraint element(day, required, selected_required);
```

The constraint can be expressed directly:

```minizinc
constraint selected_required = required[day];
```

This example bounds `day` to valid indices. Zincite offers a Safe fix only when
it can establish the supported standard body, matching nonoptional types, valid
indices and defined operands. Unsupported annotations or unresolved conditions
prevent an edit. A user predicate with the same name is not treated as the
standard `element` predicate.

## global-constraint-opportunity

Recognises selected groups of constraints that may be clearer as a global
constraint.

```minizinc
array[1 .. 3] of var 1 .. 3: assigned_shift;
constraint forall (i, j in 1 .. 3 where i < j) (
    assigned_shift[i] != assigned_shift[j]
);
```

<!-- zincite-lint: global-constraint-opportunity -->

Every pair of assignments must differ. The standard global states that intent
directly:

```minizinc
include "all_different.mzn";
constraint all_different(assigned_shift);
```

The pairwise pattern must compare every pair of array positions. The compared
values must have compatible, nonoptional types. Extra filters, shifted accesses
or omitted pairs can change the requirement and prevent the suggestion.

The rule also recognises supported whole-array occurrence bounds as candidates
for the four-argument open `global_cardinality` constraint. Values outside its
cover remain unrestricted. Check that this matches the model's intent. Neither
suggestion is an automatic rewrite or a prediction of solver performance.

## prefer-enum

Suggests considering an enum when a finite integer set's members serve as
identities throughout a complete model.

```minizinc
set of int: Shifts = 1 .. 3;
array[Shifts] of int: required;
array[Shifts] of var 0 .. 10: staff;
constraint forall (shift in Shifts) (
    staff[shift] >= required[shift]
);
solve minimize sum(staff);
output [show(sum(staff))];
```

Here, the shift IDs select array entries. An enum such as
`enum Shift = {Morning, Afternoon, Night};` can give those identities names.
Converting the model also means changing array declarations and data indices.

Arithmetic or ordering on an ID can make its numeric representation significant.
Arithmetic on a retrieved payload, such as `required[shift] + 1`, does not change
the identity itself. The rule checks uses across the model, including calls,
objectives, annotations and output. It withholds advice for representation-sensitive
output and for unsupported or unresolved uses, and skips fragments without a
solve item and standalone data files.

Even when the rule suggests an enum, review external instance files and consumers
of model output. Zincite does not establish compatibility with those consumers
and offers no automatic enum migration.

## vacuous-constraint

Reports proved empty quantifiers, filters that reject every candidate and
constraints or conditions known to be always true or false.

```minizinc
array[1 .. 7] of var 0 .. 10: staff;
constraint forall (day in 1 .. 7 where day > 7) (
    staff[day] >= 4
);
```

No day passes the filter, so this constraint imposes no staffing requirement.
Check whether the bounds or filter are wrong. If the requirement applies to the
whole week, remove the filter.

An empty `forall` is true; an empty `exists` is false. Zincite reports these
identities only when evaluation leading up to them is defined. Facts established
under a guard remain conditional on that guard. Unknown parameter values do not
prove vacuity, and the rule does not remove constraints or claim that the entire
model is feasible or infeasible.

## pairwise-self-comparison

Reports an enforced `!=`, `<` or `>` between two generator bindings over the
same source when nothing excludes the pair where both bindings are equal.

```minizinc
array[1 .. 3] of var 1 .. 3: assigned_shift;
constraint forall (i, j in 1 .. 3) (
    assigned_shift[i] != assigned_shift[j]
);
```

<!-- zincite-lint: pairwise-self-comparison -->

The generator also produces pairs with `i = j`. The pair `i = j = 1` requires
`assigned_shift[1] != assigned_shift[1]`. No assignment satisfies that, so the
model has no solution. Exclude the equal pair with `where i < j` or
`where i != j`, or start the second generator after the first, as in
`forall (i in 1 .. 3, j in i + 1 .. 3)`. With `where i < j`, the constraint is
the complete pairwise disequality that
[`global-constraint-opportunity`](#global-constraint-opportunity) recognises.

The same applies to `forall (i in S, j in S)` when both generators range over
the same set. When the rule cannot tell whether the source is empty, the
finding says that the constraint cannot be satisfied whenever the source is
nonempty. A known-empty source stays quiet.

The rule reports only when both sides access the same one-dimensional array of
nonoptional integers, enums, Booleans or floats, indexed directly by the two
bindings. It stays quiet for a comparison inside a disjunction, implication or
condition, under any filter, between different arrays, in nested `forall`
calls, and whenever it cannot prove the required facts. No automatic fix is
offered.
