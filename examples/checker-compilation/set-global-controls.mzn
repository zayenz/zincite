include "globals.mzn";
include "checker_annotations.mzn";

% Editable sets share a finite universe; each public relation has its own label.
var set of 1 .. 4: first :: checker_value("first", [1]);
var set of 1 .. 4: second :: checker_value("second", [2]);
var set of 1 .. 4: combined :: is_defined_var;
var set of 1 .. 4: common :: is_defined_var;
var 1 .. 4: minimum_value :: is_defined_var;
var -10 .. 10: total :: is_defined_var;

constraint :: "Public union"
(combined = array_union([first, second])) :: defines_var(combined) :: checker_constraint("union", [1, 2]);
constraint :: "Public intersection"
(common = array_intersect([first, second])) :: defines_var(common) :: checker_constraint("intersection", [1, 2]);
constraint :: "Public partition"
partition_set([first, second], 1 .. 4) :: checker_constraint("partition", [1, 2]);
constraint :: "Public disjoint"
disjoint(first, second) :: checker_constraint("disjoint", [1, 2]);
constraint :: "Public minimum"
(minimum_value = min(first)) :: defines_var(minimum_value) :: checker_constraint("minimum", [1]);
constraint :: "Public weighted set"
sum_set([3, 1, 2, 4], [-1, 5, 0, 2], first, total) :: defines_var(total) :: checker_constraint("weights", [1]);
solve satisfy;
