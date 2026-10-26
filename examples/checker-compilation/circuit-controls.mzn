include "globals.mzn";
include "checker_annotations.mzn";

% Source index sets exercise the public wrapper's offset and negative-index shift.
array[5 .. 7] of var 5 .. 7: positive_successors :: checker_value("positive_successors", [1, 2, 3]);
array[-2 .. 0] of var -2 .. 0: negative_successors :: checker_value("negative_successors", [4, 5, 6]);

% Every source node must belong to the same directed cycle.
constraint :: "Positive-index circuit"
circuit(positive_successors) :: checker_constraint("positive_circuit", [1, 2, 3]);
constraint :: "Negative-index circuit"
circuit(negative_successors) :: checker_constraint("negative_circuit", [4, 5, 6]);
solve satisfy;
