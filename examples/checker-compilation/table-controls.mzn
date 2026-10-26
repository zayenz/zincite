include "globals.mzn";
include "checker_annotations.mzn";

% Decisions identify the supplied entries and their two membership results.
array[1 .. 2] of var 1 .. 3: entries :: checker_value("entries", [1, 2]);
array[1 .. 2] of var bool: flags :: checker_value("flags", [3, 4]);
var bool: member :: is_defined_var;
var bool: bool_member :: is_defined_var;
var bool: guard :: checker_value("guard", [5]);

% The same allowed rows define membership and constrain the editable entries.
constraint :: "Allowed integer entries"
table(entries, [|
      1, 2
    | 2, 1
|]) :: checker_constraint("allowed", [1, 2]);
constraint :: "Integer membership"
(member <-> table(entries, [|
      1, 2
    | 2, 1
|])) :: defines_var(member) :: checker_constraint("integer_table", [1, 2]);
constraint :: "Boolean membership"
(bool_member <-> table(flags, [|
      true,  false
    | false, true
|])) :: defines_var(bool_member) :: checker_constraint("boolean_table", [3, 4]);
constraint :: "Guarded entries"
(guard -> table(entries, [|
      1, 2
|])) :: checker_constraint("guarded", [1, 2]);
solve satisfy;
