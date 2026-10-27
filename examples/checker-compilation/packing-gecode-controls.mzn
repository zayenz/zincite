include "globals.mzn";
include "checker_annotations.mzn";
include "gecode_fzn.mzn";

% Decisions include a point rectangle and explicit endpoint values.
array[1 .. 2] of var -2 .. -1: bins :: checker_value("bins", [1, 2]);
array[-2 .. -1] of var 0 .. 3: loads :: is_defined_var :: checker_value("loads", [1, 2]);
var 0 .. 4: second_x :: checker_value("x", [2]);
var 0 .. 3: first_width :: checker_value("first_width", [1]);
var bool: second_present :: checker_value("presence", [2]);
var 1 .. 4: first_right :: is_defined_var :: checker_value("first_right", [1]);

% Load and coordinate relations name separate definition targets.
constraint :: "Native bin loads"
gecode_bin_packing_load(loads, bins, [3, 0], -2) :: defines_var(loads) :: checker_constraint("loads", [1, 2]);
constraint :: "Right endpoint equation"
(first_right = 1 + first_width) :: defines_var(first_right) :: checker_constraint("right_equation", [1]);

% Fixed and variable dimensions use the same strict separation rule.
constraint :: "Native rectangle separation"
gecode_nooverlap_fzn([1, second_x], [first_width, 0], [0, 1], [2, 0]) :: checker_constraint("mandatory", [1, 2]);
constraint :: "Optional fixed rectangle separation"
gecode_nooverlap_optional([1, second_x], [2, 0], [0, 1], [2, 0], [true, second_present]) :: checker_constraint(
    "optional",
    [1, 2],
);
constraint :: "Optional explicit rectangle separation"
gecode_nooverlap_flex_optional(
    [1, second_x],
    [first_width, 0],
    [first_right, second_x],
    [0, 1],
    [2, 0],
    [2, 1],
    [true, second_present],
) :: checker_constraint("flex", [1, 2]);
solve satisfy;
