include "globals.mzn";
include "checker_annotations.mzn";
var 0.0 .. 3.0: x :: checker_value("primary", [1]);
var bool: guard;
% The last argument guards the numerical relation.
constraint :: "Guarded float ordering"
gecode_float_rel_imp_fzn(x, 4, 2.0, guard);
solve :: float_search([x], 0.0, input_order, indomain_split, complete) satisfy;
