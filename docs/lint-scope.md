# Names, scope and unused declarations

These rules find hidden dependencies and names that are unused or conceal other
names. Select them with `--rules preset:scope`.
See [linting](linting.md) for configuration and suppressions.

| Rule | Default | Automatic fix |
| --- | --- | --- |
| [`suspicious-shadowing`](#suspicious-shadowing) | Yes | No |
| [`unused-declaration`](#unused-declaration) | No | No |
| [`unused-generator-binding`](#unused-generator-binding) | No | Sometimes |
| [`global-variable-in-function`](#global-variable-in-function) | No | No |

## suspicious-shadowing

Reports a local name that hides a visible enclosing value binding. The diagnostic
identifies both declarations.

```minizinc
int: required = 4;
function int: shortfall(int: required, int: available) = max([0, required - available]);
```

Inside `shortfall`, `required` means the argument. A reader looking at the outer
declaration might expect it to mean the model's requirement. Choose names such
as `demand` and `available` for the arguments, or rename the outer declaration
to express its role more precisely.

Function parameters, `let` declarations and generators are checked. Names in
disjoint scopes and underscore-prefixed names are exempt. To allow common loop
names, configure exact exceptions:

```toml
[lint.options.suspicious-shadowing]
ignore-names = ["i", "j"]
```

## unused-declaration

Reports top-level declarations that a complete model does not use in its
constraints, solve item, output or annotations, even indirectly through other
declarations.

```minizinc
int: old_staff_limit = 12;
var 0 .. 10: staff;
constraint staff >= 4;
solve minimize staff;
```

`old_staff_limit` has no effect on this model. Check whether it is a leftover or
whether a constraint was supposed to use it. Removing it is a modelling decision;
Zincite does not delete declarations.

Uses through other declarations count. Explicit and default output also matter,
so a variable printed as part of the solution is not unused just because no
constraint refers to it. Underscore-prefixed names are exempt. Fragments without
a solve item are skipped; unknown reachable dependencies can prevent an unused
claim.

## unused-generator-binding

Finds a generator name that is never used by the remaining generators, filters,
body or annotations.

```minizinc
array[1 .. 7] of int: daily_capacity = [8 | day in 1 .. 7];
```

If seven copies of `8` are intended, make the unused name explicit:

```minizinc
array[1 .. 7] of int: daily_capacity = [8 | _ in 1 .. 7];
```

If capacity should depend on the day, the warning may instead reveal a missing
array access in the body. Removing the generator would change the number of
elements. Zincite's conditional Safe fix replaces only the name with `_` in
supported array, set and indexed comprehensions. Generator-call shorthand and
assignment generators receive advice without a fix. Names already starting
with `_` are exempt.

## global-variable-in-function

Reports a user function or predicate that reads a top-level decision variable
without receiving it as an argument.

```minizinc
var 0 .. 10: staff;
function var int: shortfall(int: required) = max([0, required - staff]);
```

The result depends on `staff`, although the call only mentions `required`.
Passing both values makes the dependency visible and lets the function be reused
for another shift:

```minizinc
function var int: shortfall(int: required, var int: available) = max([0, required - available]);
```

Parameter-valued globals do not trigger this rule. Locals or arguments that happen
to share a global's name are resolved to their actual declaration.
