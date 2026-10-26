include "globals.mzn";
include "checker_annotations.mzn";

var set of 1 .. 4: left;
var set of 1 .. 4: right;
var set of 1 .. 4: difference :: is_defined_var;
var set of 1 .. 4: changed :: is_defined_var;
var 0 .. 4: size :: is_defined_var;
var bool: member :: is_defined_var;

constraint :: "Elements only on the left"
set_diff(left, right, difference) :: defines_var(difference) :: checker_constraint("difference", [1]);
constraint :: "Elements on exactly one side"
set_symdiff(left, right, changed) :: defines_var(changed) :: checker_constraint("changed", [1, 2]);
constraint :: "Count changed elements"
set_card(changed, size) :: defines_var(size);
constraint :: "Track membership"
set_in_reif(1, difference, member) :: defines_var(member) :: checker_constraint("member", [1]);
constraint :: "Ordered source sets"
set_lt(left, right) :: checker_constraint("order", [1, 2]);

solve satisfy;
