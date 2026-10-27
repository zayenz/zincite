include "globals.mzn";
include "checker_annotations.mzn";
include "gecode_fzn.mzn";

% Decisions: the second task is a point; the first duration can change.
array[1 .. 2] of var 0 .. 8: starts :: checker_value("starts", [1, 2]);
var 0 .. 3: task_duration :: checker_value("duration", [1]);
var bool: second_present :: checker_value("presence", [2]);
array[1 .. 2] of var 0 .. 11: ends :: is_defined_var :: checker_value("ends", [1, 2]);

% End equations are separate from every native resource predicate.
constraint :: "First end follows duration"
(ends[1] = starts[1] + task_duration) :: defines_var(ends[1]) :: checker_constraint("first_end", [1]);
constraint :: "Second end equals its start"
(ends[2] = starts[2] + 0) :: defines_var(ends[2]) :: checker_constraint("second_end", [2]);

% Mandatory and optional resources expose the point task's interior conflict.
constraint :: "Explicit unary schedule"
gecode_unary_flex(starts, [task_duration, 0], ends) :: checker_constraint("unary_flex", [1, 2]);
constraint :: "Optional explicit unary schedule"
gecode_unary_flex_optional(starts, [task_duration, 0], ends, [true, second_present]) :: checker_constraint(
    "optional_unary_flex",
    [1, 2],
);
constraint :: "Explicit cumulative schedule"
gecode_cumulative_flex(starts, [task_duration, 0], ends, [2, 1], 2) :: checker_constraint("cumulative_flex", [1, 2]);
constraint :: "Optional explicit cumulative schedule"
gecode_cumulative_flex_optional(starts, [task_duration, 0], ends, [2, 1], 2, [true, second_present])
    :: checker_constraint("optional_cumulative_flex", [1, 2]);

% Signed multi-machine usage ignores the zero-length second task.
constraint :: "Explicit machine schedule"
gecode_cumulatives_flex(starts, [task_duration, 0], ends, [2, -1], [0, 0], [2], true) :: checker_constraint(
    "machine_flex",
    [1, 2],
);
solve satisfy;
