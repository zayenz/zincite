include "globals.mzn";
include "checker_annotations.mzn";

% Fixed data
set of int: Job = 1 .. 2;
array[Job] of int: duration = [3, 2];

% Editable decisions and arithmetic results
array[Job] of var 0 .. 8: start :: checker_value("start", [1, 2]);
array[Job] of var 0 .. 9: finish :: is_defined_var :: checker_value("finish", [1, 2]);
var bool: starts_early :: checker_value("starts_early", [1]);

% Explicit directions keep completion separate from the finish domain checks.
constraint :: "Finish follows duration"
forall (job in Job) (
    (finish[job] = start[job] + duration[job]) :: defines_var(finish[job]) :: checker_constraint("finish", [job])
);

% Expose a full reified result without requiring either truth value.
constraint :: "Early start indicator"
(starts_early <-> start[1] <= 2) :: checker_constraint("early_start", [1]);

% Job 1 must finish before job 2 starts.
constraint :: "Job 1 precedes job 2"
(finish[1] <= start[2]) :: checker_constraint("precedence", [1, 2]);

% The two start times share one aggregate budget.
constraint :: "Start budget"
(start[1] + start[2] <= 10) :: checker_constraint("start_budget", [1, 2]);

solve satisfy;
