include "globals.mzn";
include "checker_annotations.mzn";

% These direct declarations expose fixed row-major data in both profiles.
predicate gecode_table_int(array[int] of var int: values, array[int] of int: rows);
predicate gecode_table_int_reif(array[int] of var int: values, array[int] of int: rows, var bool: result);
predicate gecode_table_bool_reif(array[int] of var bool: values, array[int] of bool: rows, var bool: result);
predicate gecode_table_int_imp(array[int] of var int: values, array[int] of int: rows, var bool: enabled);

array[1 .. 2] of var 1 .. 3: entries :: checker_value("entries", [1, 2]);
array[1 .. 2] of var bool: flags :: checker_value("flags", [3, 4]);
var bool: member :: is_defined_var;
var bool: bool_member :: is_defined_var;
var bool: guard :: checker_value("guard", [5]);

% Each predicate checks the supplied entries without selecting an allowed row.
constraint :: "Allowed integer entries"
gecode_table_int(entries, [1, 2, 2, 1]) :: checker_constraint("allowed", [1, 2]);
constraint :: "Integer membership"
gecode_table_int_reif(entries, [1, 2, 2, 1], member) :: defines_var(member) :: checker_constraint(
    "integer_table",
    [1, 2],
);
constraint :: "Boolean membership"
gecode_table_bool_reif(flags, [true, false, false, true], bool_member) :: defines_var(bool_member)
    :: checker_constraint("boolean_table", [3, 4]);
constraint :: "Guarded entries"
gecode_table_int_imp(entries, [1, 2], guard) :: checker_constraint("guarded", [1, 2]);
solve satisfy;
