# Array indices and comprehensions

These rules check which indices an array accepts, what a filtered comprehension
contains and whether an array constructor is needed. Select them with
`--rules preset:arrays`. See [linting](linting.md) for model context and fix options.

| Rule | Default | Automatic fix |
| --- | --- | --- |
| [`index-set-mismatch`](#index-set-mismatch) | Yes | No |
| [`hidden-optionality`](#hidden-optionality) | No | No |
| [`prefer-array-comprehension`](#prefer-array-comprehension) | Yes | Sometimes |

## index-set-mismatch

Reports accesses or traversals whose indices conflict with an array's declared
index set. Equal array lengths do not imply equal indices.

```minizinc
array[0 .. 6] of int: required;
array[1 .. 7] of var 0 .. 10: staff;
constraint forall (day in 1 .. 7) (
    staff[day] >= required[day]
);
```

<!-- zincite-lint: index-set-mismatch -->

The last iteration uses `required[7]`, but `required` ends at index 6. Decide
whether both arrays should use the same days or whether an explicit mapping is
needed. Using the correct array's `index_set` helps avoid repeated bounds, but
does not by itself make two arrays compatible.

The rule also checks enum identities and supported neighbour offsets such as
`day + 1`. A guard restricting traversal to valid indices can protect an access.
Unknown data alone does not prove a mismatch, so `x[k]` with an unassigned `k`
stays quiet. Neither does a broad estimate of possible indices. A warning
identifies incompatible candidates; it does not establish that the access will
be evaluated in a solution.

The rule compares bounds written as one parameter or named size plus or minus
a constant symbolically. With an unassigned `int: n;`, the rule reports
`forall (i in 1 .. n) (x[i] < x[i + 1])` over `array[1 .. n]`: whenever the
traversal is active and nonempty, its last iteration uses index `n + 1`. The
rule reports only when an outside index occurs for every value of `n` that
makes the traversal nonempty; it does not claim that every access is outside.
Bounds that combine different parameters, products, `min` or `max`, named or
data-dependent sets, generators that depend on an earlier generator, and
filters or guards the rule cannot resolve stay quiet.

## hidden-optionality

Warns when a comparison with `length` appears to count selected values in a
comprehension filtered by a decision variable.

```minizinc
array[1 .. 7] of var bool: open;
constraint length([day | day in 1 .. 7 where open[day]]) = 3;
```

The comprehension has optional elements because `open[day]` is decided by the
solver. `length` counts its seven slots, including absent values. If the intent
is to open on three days, count the decisions directly:

```minizinc
constraint sum (day in 1 .. 7) (bool2int(open[day])) = 3;
```

The same concern applies to comprehensions over decision sets. Parameter-only
filters do not introduce this hidden optionality. The rule considers supported
integer comparisons and array aliases; it does not flag every optional array or
every use of `length`. Zincite asks the modeller to confirm the intended count
and provides no automatic replacement.

## prefer-array-comprehension

Suggests a direct comprehension when a standard `array1d`, `array2d` or `array3d`
wrapper only restores the indices already described by its generators.

```minizinc
set of int: Days = 1 .. 7;
array[Days] of int: required = array1d(Days, [4 | day in Days]);
```

For this one-based index set, the wrapper can be omitted:

```minizinc
array[Days] of int: required = [4 | day in Days];
```

Other index sets need explicit keys. For `Days = 0..6`, the corresponding
comprehension is `[day: 4 | day in Days]`. Multidimensional comprehensions retain
the ordered index tuple, for example `[(day, shift): value | day in Days,
shift in Shifts]`.

A Safe fix is available only when Zincite can preserve indices, values and
comments. Filters, dependent generators, unknown or shadowed constructors and
unsupported annotations prevent the rewrite. Multidimensional axes must be
proved nonempty. Formatting alone never performs this change; preview lint
fixes with `--diff` before applying them with `--fix`.
