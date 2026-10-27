include "globals.mzn";
include "checker_annotations.mzn";

% Editable values drive integer, set and floating-point definitions.
var -3 .. 3: x :: checker_value("x", [1]);
var 0 .. 4: y :: checker_value("y", [1]) = (x + 2) :: checker_constraint("offset", [2]);
var set of 1 .. 3: chosen :: checker_value("chosen", [1]);
var 0 .. 3: size = card(chosen);
var -2.0 .. 2.0: amount :: checker_value("amount", [1]);
var int: rounded = round(amount);
var bool: p;
var bool: q;
var bool: clause = p \/ not q;
var bool: order = x <= y;
var opt 1 .. 3: maybe;

% Distinct mathematical families expose their standard decompositions.
constraint :: "Bound set size"
size <= y :: checker_constraint("capacity", [2]);
constraint :: "Round amount"
rounded = 1 :: checker_constraint("rounded", [2]);
constraint :: "Check clause"
clause :: checker_constraint("clause", [2]);
constraint :: "Check order"
order :: checker_constraint("order", [2]);
constraint :: "Order editable values"
increasing([x, y]) :: checker_constraint("increasing", [2]);
constraint :: "Bound present optional value"
(occurs(maybe) -> deopt(maybe) <= y) :: checker_constraint("optional", [2]);

solve minimize y;
