include "globals.mzn";
include "checker_annotations.mzn";

predicate gecode_schedule_unary_optional(
    array[int] of var int: task_starts,
    array[int] of int: durations,
    array[int] of var bool: presence,
);
predicate gecode_schedule_cumulative_optional(
    array[int] of var int: task_starts,
    array[int] of int: durations,
    array[int] of int: usages,
    array[int] of var bool: presence,
    var int: capacity,
);
predicate gecode_cumulatives(
    array[int] of var int: task_starts,
    array[int] of var int: durations,
    array[int] of var int: usages,
    var int: capacity,
);
predicate gecode_cumulatives(
    array[int] of var int: task_starts,
    array[int] of var int: durations,
    array[int] of var int: usages,
    array[int] of var int: machines,
    array[int] of int: capacities,
    bool: upper,
);

% Decisions: presence controls the zero-duration second task.
array[1 .. 2] of var 0 .. 8: starts :: checker_value("starts", [1, 2]);
var bool: present :: checker_value("presence", [2]);

% Optional unary and cumulative checks agree on a zero task inside the first task.
constraint :: "Optional unary schedule"
gecode_schedule_unary_optional(starts, [3, 0], [true, present]) :: checker_constraint("optional_unary", [1, 2]);
constraint :: "Optional cumulative schedule"
gecode_schedule_cumulative_optional(starts, [3, 0], [2, 1], [true, present], 2) :: checker_constraint(
    "optional_resource",
    [1, 2],
);

% These two native overloads use signed usage and different resource horizons.
constraint :: "Concrete cumulative schedule"
gecode_cumulatives(starts, [3, 0], [2, -1], 2) :: checker_constraint("concrete_resource", [1, 2]);
constraint :: "Machine cumulative schedule"
gecode_cumulatives(starts, [3, 0], [2, -1], [0, 0], [2], true) :: checker_constraint("machine_resource", [1, 2]);
solve satisfy;
