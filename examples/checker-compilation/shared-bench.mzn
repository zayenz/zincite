include "globals.mzn";
include "checker_annotations.mzn";

% Data: two independent jobs share one workshop bench.
set of int: Job = 1 .. 2;
array[Job] of int: duration = [3, 2];
constraint :: "Durations are nonnegative"
assert(forall (job in Job) (
    duration[job] >= 0
), "Durations must be nonnegative");

% Decisions: starts and selected tools come from the application's edit.
array[Job] of var 0 .. 8: start :: checker_value("start", [1, 2]);
array[Job] of var set of 1 .. 4: tools :: checker_value("tools", [1, 2]);
var 1 .. 4: lead_tool :: checker_value("lead_tool", [1]);

% Derived values use explicit primitive directions preserved by this compiler.
var set of 1 .. 4: all_tools :: is_defined_var :: checker_value("all_tools", [1, 2]);
var set of 1 .. 4: shared_tools :: is_defined_var :: checker_value("shared_tools", [1, 2]);
var 0 .. 4: n_tools :: is_defined_var;

% Both jobs need exclusive use of the bench, independently of their tools.
constraint :: "One job at a time at the shared bench"
disjunctive_strict(start, duration) :: checker_constraint("shared_bench", [1, 2]);

% The application can display the combined and shared tool selections and count.
constraint :: "Total tools"
set_union(tools[1], tools[2], all_tools) :: defines_var(all_tools) :: checker_constraint("all_tools", [1, 2]);
constraint :: "Shared tools"
set_intersect(tools[1], tools[2], shared_tools) :: defines_var(shared_tools) :: checker_constraint(
    "shared_tools",
    [1, 2],
);
constraint :: "Tool count"
set_card(all_tools, n_tools) :: defines_var(n_tools) :: checker_constraint("n_tools", [1, 2]);

% The cutting job must select its lead tool and use only permitted tools.
constraint :: "Lead tool is available"
(lead_tool in tools[1]) :: checker_constraint("lead_tool", [1]);
constraint :: "Cutting uses permitted tools"
(tools[1] subset {1, 2, 3}) :: checker_constraint("permitted_tools", [1]);

solve satisfy;
