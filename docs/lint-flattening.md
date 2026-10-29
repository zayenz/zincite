# Comprehensions and flattening

These rules identify source structures that may create substantial work when
MiniZinc translates a model for a solver. Select them with
`--rules preset:flattening`. A finding is a reason to inspect the formulation;
it does not measure flattening time or predict solving time.
See [linting](linting.md) for configuration and model context.

| Rule | Default | Automatic fix |
| --- | --- | --- |
| [`expensive-comprehension`](#expensive-comprehension) | No | No |
| [`decision-variable-operator`](#decision-variable-operator) | No | No |
| [`decision-variable-generator`](#decision-variable-generator) | No | No |
| [`decision-variable-condition`](#decision-variable-condition) | No | No |
| [`reified-global`](#reified-global) | No | No |

## expensive-comprehension

Estimates comprehension candidates from generator sizes and examines supported
filters or repeated aggregates that could be evaluated earlier.

```minizinc
array[1 .. 2000] of var 0 .. 10: staff;
constraint forall (first, second in 1 .. 2000 where first < second) (
    staff[first] + staff[second] >= 4
);
```

The two generators have 4,000,000 candidates before the filter. That exceeds the
default threshold of 1,000,000. Check whether the relationship really needs every
pair, and inspect MiniZinc's flattening behaviour before deciding to reformulate.
The bound does not establish that the compiler enumerates all candidates.

Configure the threshold with a positive integer in `zincite.toml`:

```toml
[lint.options.expensive-comprehension]
max-candidates = 25000
```

Known candidate counts or upper bounds warn only when strictly above the
threshold. When generator sizes depend on unknown input data, Zincite can give
informational advice without a numeric estimate. That advice alone does not fail
lint.

Moving a filter or aggregate earlier can avoid repeated work, but it must
preserve the result and keep expressions defined. Zincite suggests this only
for supported parameter-only calculations whose guards and generator
dependencies remain valid. It does not reorder the model automatically.

## decision-variable-operator

Points out power, integer division, modulo and division with
decision-dependent operands.

```minizinc
var 0 .. 40: hours;
var 1 .. 8: shift_length;
var int: full_shifts = hours div shift_length;
```

Both operands are decisions, so integer division must be represented in the
constraint model. Review whether `shift_length` should be fixed input, or whether
a small table of allowed combinations would express the problem better. Keep
the division if it is the clearest suitable formulation.

The checked operators are `^`, `div`, `mod` and `/`, including quoted call
spellings such as `'div'(x, y)`. The rule deliberately departs from the thesis
by leaving `xor`, disjunction, implication in either direction, equivalence and
`not` alone: on decisions they are ordinary modelling, not a sign of a costly
formulation. Parameter-only operands are exempt. The finding does not imply that
the operator is incorrect or that a proposed alternative will solve faster.

## decision-variable-generator

Reports a generator whose source or assignment expression depends on decisions.

```minizinc
var set of 1 .. 7: open_days;
var int: day_number_sum = sum (day in open_days) (day);
```

Which days contribute is decided by the solver. MiniZinc has to represent that
selection, rather than simply iterate over fixed input. Review whether the set
must be a decision. For some models, a fixed day set with Boolean membership
decisions gives a clearer formulation; that change needs a review of all uses.

Aliases, calls and nested generators are considered. Merely using a decision
in the body does not trigger this rule when the generator source is fixed.
Zincite provides no automatic reformulation or performance guarantee.

## decision-variable-condition

Reports `if` or `elseif` conditions and generator `where` filters that depend on
decisions.

```minizinc
array[1 .. 7] of var bool: open;
array[1 .. 7] of var 0 .. 10: staff;
var int: total_staff = sum (day in 1 .. 7 where open[day]) (staff[day]);
```

The filter is part of the constraint model because `open[day]` is unknown until
solving. Review whether `open` represents a real decision or should be input
data. If it is a decision, account for the filtered comprehension's optional
elements when using it elsewhere. The
[`hidden-optionality`](lint-arrays.md#hidden-optionality) rule covers a specific
counting pitfall.

Parameter-only conditions are exempt, even when a branch contains decisions.
The rule offers advice without a rewrite; replacing a filter or conditional can
change definedness and model meaning.

## reified-global

Reports an included standard Boolean global used as a value in a
decision-dependent Boolean context, such as an implication or equivalence.

```minizinc
include "all_different.mzn";
array[1 .. 3] of var 1 .. 3: assigned_shift;
var bool: distinct_required;
constraint distinct_required -> all_different(assigned_shift);
```

The global is conditional on `distinct_required`. That can require a different
encoding from an always-enforced `all_different`. Review whether the condition
is necessary and supported efficiently by the chosen solver. Removing it would
strengthen the model, so it requires a modelling decision.

Always-enforced uses, including supported conjunctions and `forall` bodies, are
exempt. Parameter-only actual arguments and implicit builtins are also exempt.
The rule resolves the standard callable's identity rather than matching a name
alone. It gives no solver-speed guarantee.
