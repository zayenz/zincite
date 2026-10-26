include "globals.mzn";
include "checker_annotations.mzn";

% Decisions
var bool: condition;
var bool: then_bool;
var bool: chosen_bool :: is_defined_var;
var 2 .. 3: chosen_int :: is_defined_var;
var set of 1 .. 3: chosen_set :: is_defined_var;
var 1.25 .. 2.5: chosen_float :: is_defined_var;

% Constraints
% Native calls expose each value type without a high-level conditional rewrite.
constraint :: "Choose Boolean"
gecode_bool_ite(condition, then_bool, false, chosen_bool) :: defines_var(chosen_bool) :: checker_constraint(
    "conditional",
    [1],
);
constraint :: "Choose integer"
gecode_int_ite(condition, 2, 3, chosen_int) :: defines_var(chosen_int) :: checker_constraint("conditional", [2]);
constraint :: "Choose set"
gecode_set_ite(condition, {1, 3}, {2}, chosen_set) :: defines_var(chosen_set) :: checker_constraint("conditional", [3]);
constraint :: "Choose float"
gecode_float_ite(condition, 1.25, 2.5, chosen_float) :: defines_var(chosen_float) :: checker_constraint(
    "conditional",
    [4],
);

solve satisfy;
