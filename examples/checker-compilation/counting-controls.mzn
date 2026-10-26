include "globals.mzn";
include "checker_annotations.mzn";

% Editable entries feed scalar and array count outputs.
array[1 .. 3] of var 1 .. 3: entries :: checker_value("entries", [19]);
var 0 .. 3: ones :: is_defined_var;
var 0 .. 3: distinct_values :: is_defined_var;
array[1 .. 2] of var 0 .. 3: counts = global_cardinality(entries, [1, 2]);

% Count outputs expose the distribution of the same editable entries.
constraint :: "Count ones"
count_eq(entries, 1, ones) :: defines_var(ones) :: checker_constraint("ones", [19]);
constraint :: "Count distinct values"
nvalue(distinct_values, entries) :: defines_var(distinct_values) :: checker_constraint("distinct_values", [19]);
constraint :: "Check membership count"
among(ones, entries, {1}) :: checker_constraint("membership", [19]);
constraint :: "Bound adjacent ones"
sliding_sum(0, 2, 2, [bool2int(entry = 1) | entry in entries]) :: checker_constraint("windows", [19]);

solve minimize counts[1];
