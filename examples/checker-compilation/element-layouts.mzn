include "globals.mzn";
include "checker_annotations.mzn";

% Decisions
var 1 .. 9: entry;
var -3 .. -1: index;
var -2 .. -1: row;
var 4 .. 6: column;
array[-3 .. -1] of var int: values = array1d(-3 .. -1, [1, entry, 7]);
array[-2 .. -1, 4 .. 6] of var int: matrix = array2d(-2 .. -1, 4 .. 6, [1, 2, 3, 4, entry, 6]);
var 1 .. 9: selected :: is_defined_var;
var 1 .. 9: selected_matrix :: is_defined_var;

% Constraints
% The compiler must carry source offsets and both rectangular axes into FlatZinc.
constraint :: "Select from offset array"
array_var_int_element_nonshifted(index, values, selected) :: defines_var(selected) :: checker_constraint("offset", [1]);
constraint :: "Select from rectangular matrix"
array_var_int_element2d_nonshifted(row, column, matrix, selected_matrix) :: defines_var(selected_matrix)
    :: checker_constraint("matrix", [2]);

solve satisfy;
