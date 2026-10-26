include "globals.mzn";
include "checker_annotations.mzn";

% Fixed data: job 1 cuts panels in three hours; job 2 assembles them in two.
set of int: Job = 1 .. 2;
array[Job] of int: duration = [3, 2];

% Editable decisions use hours from opening; both jobs must finish by hour 9.
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

% Assembly can start after the panels have been cut.
constraint :: "Job 1 precedes job 2"
(finish[1] <= start[2]) :: checker_constraint("precedence", [1, 2]);

% The two start times sum to at most 10 hours to limit waiting before work starts.
constraint :: "Start budget"
(start[1] + start[2] <= 10) :: checker_constraint("start_budget", [1, 2]);

solve satisfy;
