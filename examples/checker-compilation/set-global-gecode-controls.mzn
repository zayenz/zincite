include "globals.mzn";
include "checker_annotations.mzn";
include "gecode_fzn.mzn";

% Native relations keep independent editable operands and scalar outputs.
var set of 1 .. 4: first :: checker_value("first", [1]);
var set of 1 .. 4: second :: checker_value("second", [2]);
var set of 1 .. 4: combined :: is_defined_var;
var set of 1 .. 4: common :: is_defined_var;
var set of 1 .. 4: hull_result :: is_defined_var;
var 1 .. 4: minimum_value :: is_defined_var;
var 1 .. 4: maximum_value;
var -10 .. 10: total :: is_defined_var;
var bool: is_maximum_value :: is_defined_var;
var bool: guard;

constraint :: "Native union"
(combined = array_union([first, second])) :: defines_var(combined) :: checker_constraint("union", [1, 2]);
constraint :: "Native intersection"
gecode_array_set_intersect([first, second], common) :: defines_var(common) :: checker_constraint(
    "intersection",
    [1, 2],
);
constraint :: "Convex hull"
gecode_set_convex_hull(first, hull_result) :: defines_var(hull_result) :: checker_constraint("hull", [1]);
constraint :: "Convex members"
gecode_set_convex(first) :: checker_constraint("convex", [1]);
constraint :: "Ordered sets"
gecode_array_set_seq([first, {}, second]) :: checker_constraint("sequence", [1, 3]);
constraint :: "Sequence union"
gecode_array_set_seq_union([first, {}, second], combined) :: checker_constraint("sequence_union", [1, 3]);
constraint :: "Fixed cardinalities"
gecode_set_atmost_one([first, second], 2) :: checker_constraint("cardinality", [1, 2]);
constraint :: "Native minimum"
gecode_set_min(first, minimum_value) :: defines_var(minimum_value) :: checker_constraint("minimum", [1]);
constraint :: "Native maximum truth"
gecode_set_max_reif(first, maximum_value, is_maximum_value) :: defines_var(is_maximum_value) :: checker_constraint(
    "maximum",
    [1],
);
constraint :: "Guarded maximum"
gecode_set_max_imp(first, maximum_value, guard) :: checker_constraint("guard", [1]);
constraint :: "Native weighted set"
sum_set([3, 1, 2, 4], [-1, 5, 0, 2], first, total) :: defines_var(total) :: checker_constraint("weights", [1]);
solve satisfy;
