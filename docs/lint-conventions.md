# Naming, labels and index conventions

These rules make names, constraint intent and numeric array indices easier to
read. Select the group with `--rules preset:conventions`.
See [linting](linting.md) for configuration, suppressions and diagnostic output.

| Rule | Family | Severity | Default | Automatic fix |
| --- | --- | --- | --- | --- |
| [`naming`](#naming) | `style` | Warning | No | No |
| [`missing-constraint-label`](#missing-constraint-label) | `style` | Warning | No | No |
| [`array-index-start`](#array-index-start) | `style` | Warning | No | No |

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
