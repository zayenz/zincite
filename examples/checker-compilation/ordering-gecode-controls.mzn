include "globals.mzn";
include "checker_annotations.mzn";

predicate gecode_sort_permutation(
    array[int] of var int: x,
    array[int] of var int: y,
    array[int] of var int: indexes,
    int: offset,
);
predicate gecode_bool_int_channel(array[int] of var bool: x, var int: value, int: offset);
predicate gecode_arg_min_int(array[int] of var int: x, int: offset, var int: index, bool: break_ties);

% These direct Gecode calls preserve the pinned positional contracts.
array[1 .. 3] of var 1 .. 3: input :: checker_value("input", [1, 2, 3]);
array[1 .. 3] of var 1 .. 3: sorted :: is_defined_var;
array[1 .. 3] of var -2 .. 0: permutation :: is_defined_var;
array[1 .. 3] of var bool: flags :: is_defined_var;
var 0 .. 2: selected :: checker_value("selected", [4]);
var 5 .. 7: first_min :: is_defined_var;

% Permutation indexes map each input position into the sorted array.
constraint :: "Sort with permutation"
gecode_sort_permutation(input, sorted, permutation, -2) :: defines_var(sorted) :: checker_constraint("sort", [1, 2, 3]);
constraint :: "Define permutation"
gecode_sort_permutation(input, sorted, permutation, -2) :: defines_var(permutation) :: checker_constraint(
    "permutation",
    [1, 2, 3],
);
constraint :: "Select one position"
gecode_bool_int_channel(flags, selected, 0) :: defines_var(flags) :: checker_constraint("channel", [4]);
constraint :: "Find first minimum"
gecode_arg_min_int(input, 5, first_min, true) :: defines_var(first_min) :: checker_constraint("arg_min", [1, 2, 3]);
solve satisfy;
