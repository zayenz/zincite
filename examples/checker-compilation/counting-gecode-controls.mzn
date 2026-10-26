include "globals.mzn";
include "checker_annotations.mzn";

% These native forms expose array definition annotations and fixed window roles.
predicate gecode_global_cardinality(
    array[int] of var int: values,
    array[int] of int: cover,
    array[int] of var int: tallies,
);
predicate gecode_among_seq_int(
    array[int] of var int: values,
    set of int: membership,
    int: length,
    int: lower,
    int: upper,
);
predicate gecode_among_seq_bool(array[int] of var bool: values, bool: membership, int: length, int: lower, int: upper);

array[1 .. 3] of var 1 .. 3: entries :: checker_value("entries", [19]);
array[1 .. 3] of var bool: flags :: checker_value("flags", [19]);
var 0 .. 3: ones :: is_defined_var;
var 0 .. 3: twos :: is_defined_var;
array[1 .. 2] of var 0 .. 3: counts = [ones, twos];

% The count array follows cover order; windows count each position.
constraint :: "Count cover values"
gecode_global_cardinality(entries, [1, 2], counts) :: defines_var(counts) :: checker_constraint("counts", [19]);
constraint :: "Bound adjacent ones"
gecode_among_seq_int(entries, {1}, 2, 0, 1) :: checker_constraint("integer_windows", [19]);
constraint :: "Require a false flag per window"
gecode_among_seq_bool(flags, false, 2, 1, 2) :: checker_constraint("boolean_windows", [19]);

solve minimize ones;
