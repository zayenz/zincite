include "globals.mzn";
include "checker_annotations.mzn";

% Decisions use negative bin labels and a point rectangle inside or at a boundary.
array[1 .. 2] of var -2 .. -1: bins :: checker_value("bins", [1, 2]);
array[-2 .. -1] of var 0 .. 3: loads :: is_defined_var :: checker_value("loads", [1, 2]);
var 0 .. 4: second_x :: checker_value("x", [2]);

% Public wrappers expose load equality and uniform or individual capacities.
constraint :: "Bin loads"
bin_packing_load(loads, bins, [3, 0]) :: defines_var(loads) :: checker_constraint("loads", [1, 2]);
constraint :: "Uniform bin capacity"
bin_packing(3, bins, [3, 0]) :: checker_constraint("capacity", [1, 2]);
constraint :: "Individual bin capacities"
bin_packing_capa(array1d(-2 .. -1, [3, 3]), bins, [3, 0]) :: checker_constraint("capacities", [1, 2]);

% Strict diffn includes the zero-sized second rectangle.
constraint :: "Strict rectangle separation"
diffn([0, second_x], [0, 1], [2, 0], [2, 0]) :: checker_constraint("rectangles", [1, 2]);
solve satisfy;
