# Domains and expression preconditions

These rules review variable bounds, the conditions needed for operations such
as indexing and division, and exact comparisons of computed floats. Select them
with `--rules preset:domains`.
See [linting](linting.md) for model setup and diagnostic limitations.

| Rule | Family | Severity | Default | Automatic fix |
| --- | --- | --- | --- | --- |
| [`unbounded-variable`](#unbounded-variable) | `performance` | Warning | Yes | No |
| [`suspicious-domain`](#suspicious-domain) | `suspicious` | Warning | No | No |
| [`partial-expression`](#partial-expression) | `suspicious` | Warning | Yes | No |
| [`missing-input-precondition`](#missing-input-precondition) | `modelling` | Informational | No | No |
| [`float-equality`](#float-equality) | `suspicious` | Warning | No | No |

## unbounded-variable

Recommends an explicit domain for a numeric decision variable when Zincite cannot
find bounds or a complete definition that supplies them.

```minizinc
var int: staff;
constraint staff >= 4;
solve minimize staff;
```

The lower bound does not supply an upper bound. If at most ten people can work
the shift, write `var 0..10: staff;`. The upper bound must come from the actual
problem; choosing an arbitrary small number can remove valid solutions.

The rule considers scalar and array element domains, aliases and supported
definitions elsewhere in the model. A complete bounded definition can make an
explicit bound unnecessary. Conditional, cyclic or partial equalities do not
establish such a definition. When an unknown relation might define the value,
Zincite withholds advice about the affected variable and reports the analysis
limitation.

## suspicious-domain

Compares an integer value's definition with its declared domain.

```minizinc
var 8 .. 10: morning_staff;
var 0 .. 7: total_staff = morning_staff + 1;
```

<!-- zincite-lint: suspicious-domain -->

`morning_staff + 1` lies in `9..11`, which is disjoint from `0..7`. Check the
definition and the destination domain. If the intended total is morning staff
plus one supervisor, `var 9..11: total_staff` matches that calculation.

The rule covers supported initializers, immutable `let` locals and enforced
equalities. A disjoint value range establishes a contradiction in that
definition. An overlapping range that extends outside the destination domain
gets weaker advice about a possible conflict; it does not prove that a failing
value can occur. Unknown parameter values, parameter defaults and arithmetic
overflow do not justify a conflict claim.

## partial-expression

Reports an operation whose required condition is known to fail, or an access
with incompatible candidate indices. An access in a traversal over symbolic
bounds, such as `data[i + 1]` for `i in 1 .. m` over `array[1 .. m]`, reports
when the traversal includes an outside index whenever it is active and
nonempty.

```minizinc
array[1 .. 7] of int: required;
int: next_week = required[8];
```

Index 8 is outside the array. Review whether the intended value belongs to a
different array, whether the array is too short or whether the access should
have been guarded. The rule also covers division or modulo by zero, `min` or
`max` of an empty array and `deopt` of an absent value.

Zincite considers guards, assertions, inactive branches, empty iterations and
supported `default` expressions. A warning explains what fails if the operation
is evaluated; it does not predict a crash or prove that the model is infeasible.
Unknown data alone does not trigger a warning. Index findings shared with
[`index-set-mismatch`](lint-arrays.md#index-set-mismatch) are deduplicated at the
same unsuppressed access.

## missing-input-precondition

Finds a function or predicate whose arguments permit values that would make an
operation undefined.

```minizinc
function int: average_required(array[int] of int: required) = sum(required) div length(required);
```

An empty array gives a zero divisor. If empty input is invalid, state that
contract in the function:

```minizinc
function int: average_required(array[int] of int: required) =
    assert(
        length(required) > 0,
        "At least one day is required",
        sum(required) div length(required),
    );
```

Another design might return a chosen value for empty input. That choice belongs
to the function's author. Conditions elsewhere in the model and default input
values do not validate a general function's arguments.

Supported checks include valid array indices, matching array dimensions,
nonempty inputs to `min` and `max`, nonzero divisors and positive integer inputs
to scalar `ln`, `log10` and `log2`, including through `int2float`. Guards and
successful local assertions can satisfy a condition. General floating-point
logarithm contracts and unresolved operations remain outside this analysis.

## float-equality

Reports `=`, `==` and `!=` when both operands are parameter floats. Rounding
makes computed floats rarely equal exactly, so such a test can fail although
the values agree on paper.

```minizinc
float: total = 1.0;
int: n = 10;
float: rate = if total / n = 0.1 then 1.0 else 2.0 endif;
```

<!-- zincite-lint: float-equality -->

Compare within a tolerance that suits the data:

```minizinc
float: rate = if abs(total / n - 0.1) < 1e-9 then 1.0 else 2.0 endif;
```

A comparison with a decision float, as in `var float: x; constraint x = 1.5;`,
is an ordinary constraint and stays quiet. So do integer comparisons, optional
or unknown-typed operands and comparisons that select a user-defined `=` or
`!=`. Choosing the tolerance is manual.
