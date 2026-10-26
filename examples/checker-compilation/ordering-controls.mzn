include "globals.mzn";
include "checker_annotations.mzn";

% Decisions and functional outputs use small exact domains.
array[1 .. 3] of var 1 .. 3: input :: checker_value("input", [1, 2, 3]);
array[1 .. 3] of var 1 .. 3: sorted :: is_defined_var;
var 1 .. 3: minimum :: is_defined_var;
var bool: contains_two :: is_defined_var;

% Sort exposes the multiset and scalar extrema expose useful completion.
constraint :: "Sort input"
sort(input, sorted) :: defines_var(sorted) :: checker_constraint("sort", [1, 2, 3]);
constraint :: "Find minimum"
(minimum = min(input)) :: defines_var(minimum) :: checker_constraint("minimum", [1, 2, 3]);
constraint :: "Detect two"
(contains_two <-> exists (i in 1 .. 3) (input[i] = 2)) :: defines_var(contains_two) :: checker_constraint(
    "member",
    [1, 2, 3],
);
constraint :: "Distinct input"
all_different(input) :: checker_constraint("distinct", [1, 2, 3]);
solve satisfy;
