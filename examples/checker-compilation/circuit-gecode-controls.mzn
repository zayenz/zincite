include "globals.mzn";
include "checker_annotations.mzn";

predicate gecode_circuit(int: offset, array[int] of var int: entries);
predicate gecode_circuit_cost(
    array[int] of int: matrix,
    array[int] of var int: entries,
    var int: total,
);
predicate gecode_circuit_cost_array(
    array[int] of int: matrix,
    array[int] of var int: entries,
    array[int] of var int: edge_costs,
    var int: total,
);

array[1 .. 3] of var 5 .. 7: positive_successors :: checker_value("positive_successors", [1, 2, 3]);
array[1 .. 2] of var 0 .. 1: successors :: checker_value("successors", [4, 5]);
array[1 .. 2] of var -4 .. 7: costs :: is_defined_var;
var -4 .. 14: array_total;
var -4 .. 14: scalar_total :: is_defined_var;

% Native cost calls use zero-based successors and a fixed row-major matrix.
constraint :: "Native offset circuit"
gecode_circuit(5, positive_successors) :: checker_constraint("native_circuit", [1, 2, 3]);
constraint :: "Native edge costs"
gecode_circuit_cost_array([0, 7, -4, 0], successors, costs, array_total) :: defines_var(costs) :: checker_constraint(
    "native_edges",
    [4, 5],
);
constraint :: "Native scalar cost"
gecode_circuit_cost([0, 7, -4, 0], successors, scalar_total) :: defines_var(scalar_total) :: checker_constraint(
    "native_total",
    [4, 5],
);
solve satisfy;
