include "globals.mzn";
include "checker_annotations.mzn";

% Decisions: the second task may have zero duration.
array[1 .. 2] of var 0 .. 8: starts :: checker_value("starts", [1, 2]);
array[1 .. 2] of var 0 .. 3: durations :: checker_value("durations", [1, 2]);

% A zero-duration task may touch an endpoint but cannot sit inside the other task.
constraint :: "Strict schedule"
disjunctive_strict(starts, durations) :: checker_constraint("strict", [1, 2]);
solve satisfy;
