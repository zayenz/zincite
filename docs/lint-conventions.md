# Naming, labels and index conventions

These rules make names, constraint intent and numeric array indices easier to
read, point out standard-library calls that MiniZinc has deprecated, suggest
assertions for input checks and flag Boolean operators whose grouping is easy
to misread. Select the group with `--rules preset:conventions`.
See [linting](linting.md) for configuration, suppressions and diagnostic output.

| Rule | Family | Severity | Default | Automatic fix |
| --- | --- | --- | --- | --- |
| [`naming`](#naming) | `style` | Warning | No | No |
| [`missing-constraint-label`](#missing-constraint-label) | `style` | Warning | No | No |
| [`array-index-start`](#array-index-start) | `style` | Warning | No | No |
| [`deprecated-call`](#deprecated-call) | `suspicious` | Warning | Yes | No |
| [`prefer-assertion`](#prefer-assertion) | `style` | Warning | No | No |
| [`ambiguous-precedence`](#ambiguous-precedence) | `suspicious` | Warning | No | No |

## naming

Checks names against Zincite's MiniZinc conventions. Values, functions,
predicates, fields and generator bindings use `snake_case`. Types, enums,
constructors and explicitly declared parameter sets use `UpperCamelCase`.

```minizinc
set of int: SHIFTS = 1 .. 7;
array[SHIFTS] of var 0 .. 10: staffCount;
```

<!-- zincite-lint: naming -->

Here, `Shifts` and `staff_count` communicate the roles of the two declarations:

```minizinc
set of int: Shifts = 1 .. 7;
array[Shifts] of var 0 .. 10: staff_count;
```

Set-valued decision variables still use `snake_case`. Quoted identifiers and
`_` are exempt; one leading underscore is allowed for intentionally unused
bindings. The rule checks declarations whose role is clear from syntax. It
does not check references or assignment names in standalone `.dzn` files.
Renaming is manual because other files may use the same names.

## missing-constraint-label

Recommends a string annotation explaining why a constraint belongs in the model.

```minizinc
array[1 .. 7] of var 0 .. 10: staff;
array[1 .. 7] of int: required;
constraint forall (day in 1 .. 7) (
    staff[day] >= required[day]
);
```

A label can describe the modelling requirement rather than repeat the formula:

```minizinc
constraint :: "Every day has enough staff to cover demand"
forall (day in 1 .. 7) (
    staff[day] >= required[day]
);
```

The label must be a direct string annotation in the constraint header. A string
inside the expression or a function call does not count. Equality items written
without `constraint` are also checked. Missing labels are readability advice;
Zincite leaves their wording to the modeller.

## array-index-start

Suggests starting nonempty numeric array indices at 1, following MiniZinc's usual
array convention.

```minizinc
array[0 .. 6] of int: required;
```

If the numbers are only positions, `array[1..7]` avoids having to remember a
different offset. Changing the index set requires reviewing data and accesses
as well as the declaration.

An offset can also be meaningful. An array indexed by years, for example, may
reasonably start at 2026. Keep that representation and [suppress the
rule](linting.md#suppression-and-explicit-fixes) when appropriate. Enum indices
are exempt. Unknown bounds and unconstrained `int` indices do not receive this
advice.

## deprecated-call

Reports a call to a standard-library predicate or function that MiniZinc marks
with `mzn_deprecated`. The message quotes the MiniZinc version that deprecated
it and the library's explanation, usually a link to the documentation.

```minizinc
include "globals.mzn";

array[1 .. 5] of var 1 .. 3: shift;
constraint at_most(2, shift, 3);
```

<!-- zincite-lint: deprecated-call -->

The integer form of `at_most` has been deprecated since MiniZinc 2.4.0. Its
documentation recommends a count instead:

```minizinc
constraint count (s in shift) (s = 3) <= 2;
```

The rule follows the overload the call selects. The set form of `at_most` is
not deprecated, so a call with set arguments stays quiet. So does a call to
your own predicate with the same name. When Zincite cannot select one
overload and a deprecated candidate remains, it records an analysis limitation
instead of a finding. Replacing the call is manual.

## prefer-assertion

Reports a constraint item whose expression depends only on parameters. Such a
constraint usually checks the input data. When it fails, MiniZinc reports only
that the model is inconsistent. An `assert` names the failed condition instead.

```minizinc
int: n;
array[1 .. n] of var 0 .. 9: digit;
constraint n > 0;
```

<!-- zincite-lint: prefer-assertion -->

Write the check as an assertion with a message:

```minizinc
constraint assert(n > 0, "n must be positive");
```

A constraint item that already is an `assert` call stays quiet, also when
written infix, parenthesized or annotated, as does one that depends on a decision variable or
contains a `let` with local constraints. Constant conditions and deliberate
infeasibility checks over parameters also warn; suppress the rule for those.
When Zincite cannot determine whether the expression depends on decisions, it
records an analysis limitation instead of a finding. The rule checks top-level
constraint items only, not constraints in `let` expressions. Rewriting the
constraint is manual.

## ambiguous-precedence

Reports two unparenthesized Boolean operator patterns whose grouping is easy to
misread. The [MiniZinc 2.10.1 operator
table](https://docs.minizinc.dev/en/2.10.1/spec.html#operators) defines the
grouping:

- `/\` binds tighter than `\/` and `xor`, so `a \/ b /\ c` means
  `a \/ (b /\ c)`. The rule reports `/\` and `\/` or `xor` at one level.
- `->` and `<-` share one precedence level and group from the left, so
  `a -> b -> c` means `(a -> b) -> c`. The rule reports a chain of two or more
  of these operators.

```minizinc
var bool: rain;
var bool: wind;
var bool: stay_in;
constraint rain \/ wind /\ stay_in;
```

<!-- zincite-lint: ambiguous-precedence -->

The warning points at the operator and shows the grouping MiniZinc uses. Add
parentheses to state the grouping you mean:

```minizinc
constraint (rain \/ wind) /\ stay_in;
```

Parenthesized forms such as `a -> (b -> c)` and `(a -> b) -> c` stay quiet, and
so does `a /\ b -> c`, where conjunction binding tighter than implication
matches the usual reading. Unicode spellings such as `∨`, `∧` and `→` count as
the same operators. Other operators, `not`, `<->` chains and backquoted infix
calls are not checked. The rule needs no model context or standard library.
Adding parentheses is manual; the formatter never adds them.
